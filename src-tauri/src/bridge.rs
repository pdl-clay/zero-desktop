use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use base64::Engine;
use tauri::Emitter;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::acp::{parse_line, AcpMessage, AcpPeer};
use crate::locator::locate_zero;
use crate::{AttachmentKind, FileAttachment};

/// The backend imposes no hard limit on the number of `zero acp` child
/// processes that may be live at the same time - the user manages them freely.
/// The frontend enforces a per-workspace panel cap (see `MAX_OPEN_PANELS` in
/// `session-runtime-store.js`), but there is no global process cap here.
///
/// `live_count_sync` is kept for `list_live_sessions` / diagnostics even though
/// it is no longer used as a gate.

/// Events emitted to the frontend on `zero:event`. Kept in the same shape
/// zero's old stream-json protocol used ({schemaVersion, type, ...payload})
/// so the frontend (zero-store.js) barely needs to change across the
/// exec -> acp migration - we translate ACP's notifications into this shape
/// here instead of teaching the frontend ACP's native format.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputEvent {
    #[serde(rename = "schemaVersion")]
    pub schema_version: i32,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

impl OutputEvent {
    fn new(event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            schema_version: 2,
            event_type: event_type.to_string(),
            payload,
        }
    }

    /// Tag the payload with the session key so the frontend can route global
    /// events to the correct panel/store. Called right before emission.
    fn with_session_key(self, session_key: &str) -> Self {
        let mut payload = self.payload;
        payload["sessionKey"] = serde_json::Value::String(session_key.to_string());
        Self {
            schema_version: self.schema_version,
            event_type: self.event_type,
            payload,
        }
    }
}

/// Result returned by `ZeroBridge::start` so the frontend learns the real
/// `session_id` assigned by the CLI and whether a new process was spawned or
/// an existing live session was reattached.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StartedSession {
    pub key: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub reattached: bool,
}

/// Snapshot of one live session, returned by `list_live_sessions`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveSessionInfo {
    pub key: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub cwd: PathBuf,
    pub live: bool,
}

/// ACP's tool_call/tool_call_update identify calls with a `toolCallId` like
/// `"call_00_..."` and put the human-readable tool name in `title` (e.g.
/// `"edit_file /path/to/file"`). Prefer the title; fall back to stripping a
/// trailing `_<digits>` counter from the id when the title is missing.
fn tool_name_from_call(tool_call_id: &str, title: Option<&str>) -> String {
    if let Some(title) = title {
        let trimmed = title.trim();
        if !trimmed.is_empty() {
            // Titles often include the first argument (e.g. "edit_file note.txt");
            // keep only the first whitespace-delimited token as the tool name.
            return trimmed.split_whitespace().next().unwrap_or(trimmed).to_string();
        }
    }
    match tool_call_id.rsplit_once('_') {
        Some((prefix, suffix)) if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) => {
            prefix.to_string()
        }
        _ => tool_call_id.to_string(),
    }
}

fn extract_tool_result_text(content: Option<&serde_json::Value>) -> String {
    content
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("content"))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

/// Builds the `prompt` content-block array for `session/prompt` (ACP:
/// `[{"type":"text",...}, {"type":"image",...}]`). The text block is
/// omitted for an image-only message instead of sending an empty string.
fn build_prompt_blocks(content: &str, file: Option<&FileAttachment>) -> Vec<serde_json::Value> {
    let mut blocks = Vec::new();
    if !content.is_empty() {
        blocks.push(serde_json::json!({ "type": "text", "text": content }));
    }
    if let Some(file) = file {
        match attachment_kind_from_mime(&file.mime_type) {
            AttachmentKind::Image => {
                blocks.push(serde_json::json!({
                    "type": "image",
                    "mimeType": file.mime_type,
                    "data": file.data,
                }));
            }
            AttachmentKind::Text => {
                if let Ok(text) = base64_decode_to_string(&file.data) {
                    blocks.push(serde_json::json!({
                        "type": "text",
                        "text": format!("<attached file name=\"{}\" type=\"{}\">\n{}\n</attached file>",
                            file.name, file.mime_type, text),
                    }));
                }
            }
            AttachmentKind::Binary => {
                // No content block type lets us hand raw binary bytes to the
                // model - the file is still attached (the user picked it and
                // sees it in the composer), but the agent only learns it
                // exists, not what's inside. Better than silently vanishing
                // from the prompt, which is what happened before this kind
                // existed (attachment_kind_from_mime used to return None for
                // anything non-image/non-text, and this whole `if let` was
                // skipped).
                blocks.push(serde_json::json!({
                    "type": "text",
                    "text": format!(
                        "<attached file name=\"{}\" type=\"{}\">\n[binary file - content not included]\n</attached file>",
                        file.name, file.mime_type,
                    ),
                }));
            }
        }
    }
    blocks
}

fn attachment_kind_from_mime(mime_type: &str) -> AttachmentKind {
    if mime_type.starts_with("image/") {
        AttachmentKind::Image
    } else if mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/yaml"
        || mime_type == "application/xml"
    {
        AttachmentKind::Text
    } else {
        AttachmentKind::Binary
    }
}

fn base64_decode_to_string(data: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Failed to decode attachment: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("Attachment is not valid UTF-8: {e}"))
}

/// Translate an ACP `session/update` notification's `params` into our
/// internal event shape. Returns `None` for update kinds we don't render.
fn translate_session_update(params: &serde_json::Value) -> Option<OutputEvent> {
    let update = params.get("update")?;
    let kind = update.get("sessionUpdate")?.as_str()?;

    match kind {
        "agent_message_chunk" => {
            let text = update["content"]["text"].as_str().unwrap_or("");
            Some(OutputEvent::new("text", serde_json::json!({ "delta": text })))
        }
        "agent_thought_chunk" => {
            let text = update["content"]["text"].as_str().unwrap_or("");
            Some(OutputEvent::new("reasoning", serde_json::json!({ "delta": text })))
        }
        "tool_call" => {
            let tool_call_id = update["toolCallId"].as_str().unwrap_or("");
            let title = update["title"].as_str();
            let name = tool_name_from_call(tool_call_id, title);
            Some(OutputEvent::new(
                "tool_call",
                serde_json::json!({
                    "id": tool_call_id,
                    "name": name,
                    "args": update.get("rawInput").cloned().unwrap_or(serde_json::json!({})),
                }),
            ))
        }
        "tool_call_update" => {
            let tool_call_id = update["toolCallId"].as_str().unwrap_or("");
            let status = update["status"].as_str().unwrap_or("completed");
            let is_error = status == "failed" || status == "error";
            let output = extract_tool_result_text(update.get("content"));
            Some(OutputEvent::new(
                "tool_result",
                serde_json::json!({
                    "id": tool_call_id,
                    "status": if is_error { "error" } else { "ok" },
                    "output": output,
                }),
            ))
        }
        "plan" => {
            let entries = update.get("entries").cloned().unwrap_or(serde_json::json!([]));
            Some(OutputEvent::new("plan_update", serde_json::json!({ "entries": entries })))
        }
        "_zero/spec_review_required" => Some(OutputEvent::new(
            "spec_review_required",
            serde_json::json!({
                "specId": update["specId"].as_str().unwrap_or(""),
                "title": update["title"].as_str().unwrap_or(""),
                "filePath": update["filePath"].as_str().unwrap_or(""),
                "relativePath": update["relativePath"].as_str().unwrap_or(""),
            }),
        )),
        _ => None,
    }
}

/// Translate a `session/request_permission` request's params into the
/// payload emitted on `zero:permission-request`. `correlation_id` is ours -
/// generated so we can look the pending request back up when the frontend
/// answers, independent of whatever id shape the JSON-RPC request used.
fn translate_permission_request(correlation_id: &str, params: &serde_json::Value) -> serde_json::Value {
    let tool_call = &params["toolCall"];
    let tool_call_id = tool_call["toolCallId"].as_str().unwrap_or("");
    let title = tool_call["title"].as_str();
    let fallback_name = tool_name_from_call(tool_call_id, title);
    let tool_name = title.map(|t| {
        let trimmed = t.trim();
        trimmed.split_whitespace().next().unwrap_or(trimmed).to_string()
    }).unwrap_or(fallback_name);
    let reason = tool_call["rawInput"]["reason"].as_str().unwrap_or("");
    let options = params.get("options").cloned().unwrap_or(serde_json::json!([]));

    serde_json::json!({
        "requestId": correlation_id,
        "toolName": tool_name,
        "reason": reason,
        "options": options,
        "answerable": true,
    })
}

fn now_ms_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_default()
}

/// Append one entry to the local history log for a session. Best-effort:
/// failures are logged, not propagated, since a lost history line shouldn't
/// interrupt a live conversation.
async fn append_history(path: &Path, event_type: &str, payload: &serde_json::Value) {
    let entry = serde_json::json!({
        "type": event_type,
        "payload": payload,
        "createdAt": now_ms_string(),
    });
    let Ok(mut line) = serde_json::to_string(&entry) else {
        return;
    };
    line.push('\n');

    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            log::error!("[bridge] failed to create history dir {parent:?}: {e}");
            return;
        }
    }

    match OpenOptions::new().create(true).append(true).open(path).await {
        Ok(mut file) => {
            if let Err(e) = file.write_all(line.as_bytes()).await {
                log::error!("[bridge] failed to append history line: {e}");
            }
        }
        Err(e) => log::error!("[bridge] failed to open history file {path:?}: {e}"),
    }
}

/// Flush accumulated `agent_thought_chunk` deltas as one `reasoning` history
/// entry. Chunks arrive one word (sometimes one token) at a time - persisting
/// each individually turned history replay into dozens of one-word "thinking"
/// bubbles instead of the single continuous thought the live UI shows.
async fn flush_pending_reasoning(path: &Path, pending: &mut String) {
    if pending.is_empty() {
        return;
    }
    append_history(path, "reasoning", &serde_json::json!({ "content": pending })).await;
    pending.clear();
}

/// Flush an accumulated message (user prompt or coalesced assistant reply)
/// as one `message` history entry - the type `buildMessagesFromHistory` on
/// the frontend actually renders as a chat bubble. Nothing used to write
/// this type at all: the assistant's `agent_message_chunk` deltas were
/// persisted verbatim as `text` events, which `load_session_history`'s type
/// allowlist doesn't even let through, so replayed sessions showed every
/// tool call and thought but never the actual reply.
async fn flush_pending_message(path: &Path, role: &str, pending: &mut String) {
    if pending.is_empty() {
        return;
    }
    append_history(
        path,
        "message",
        &serde_json::json!({ "role": role, "content": pending }),
    )
    .await;
    pending.clear();
}

/// Where zero-desktop keeps its own rich session history, separate from
/// whatever zero's own `~/.local/share/zero/sessions/<id>/events.jsonl`
/// contains (which, in ACP mode, only records `message` entries - verified
/// live - not tool calls/reasoning/permission decisions).
pub fn history_path_for(session_id: &str) -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base
        .join("zero-desktop")
        .join("session-history")
        .join(format!("{session_id}.jsonl")))
}

/// zero-desktop's own record of session titles, keyed by session id. Needed
/// because ACP-created sessions get a generic "ACP session" title from zero
/// itself with no discovered protocol method to set a better one - so we
/// track titles ourselves and overlay them onto `zero sessions list` output.
fn title_map_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("session-titles.json"))
}

static TITLE_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load_title_map() -> HashMap<String, String> {
    let Ok(path) = title_map_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_title_map(map: &HashMap<String, String>) -> Result<(), String> {
    let path = title_map_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn get_session_title(session_id: &str) -> Option<String> {
    load_title_map().get(session_id).cloned()
}

/// Set (or overwrite) a session's title - used both for the auto-derived
/// title on a session's first message and for an explicit user rename.
pub fn set_session_title(session_id: &str, title: &str) -> Result<(), String> {
    let _lock = TITLE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_title_map();
    map.insert(session_id.to_string(), title.to_string());
    save_title_map(&map)
}

pub fn remove_session_title(session_id: &str) -> Result<(), String> {
    let _lock = TITLE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_title_map();
    if map.remove(session_id).is_some() {
        save_title_map(&map)?;
    }
    Ok(())
}

/// zero-desktop's own record of which model answered each session, keyed by
/// session id. Needed because `zero sessions list --json` reports an empty
/// `modelId` in ACP mode and the ACP protocol itself never surfaces the
/// model (verified live - neither `session/new`'s result nor any
/// `session/update` notification carries it) - so we snapshot the active
/// model from `zero config --json` when the session is created and overlay
/// it the same way session titles are overlaid.
fn model_map_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("session-models.json"))
}

static MODEL_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load_model_map() -> HashMap<String, String> {
    let Ok(path) = model_map_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_model_map(map: &HashMap<String, String>) -> Result<(), String> {
    let path = model_map_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn get_session_model(session_id: &str) -> Option<String> {
    load_model_map().get(session_id).cloned()
}

pub fn set_session_model(session_id: &str, model_id: &str) -> Result<(), String> {
    let _lock = MODEL_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_model_map();
    map.insert(session_id.to_string(), model_id.to_string());
    save_model_map(&map)
}

pub fn remove_session_model(session_id: &str) -> Result<(), String> {
    let _lock = MODEL_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_model_map();
    if map.remove(session_id).is_some() {
        save_model_map(&map)?;
    }
    Ok(())
}

/// zero-desktop's own record of each session's reasoning-effort preference,
/// keyed by session id - same rationale as session-models.json: nothing in
/// the ACP protocol reports the current effort back, so this is the only way
/// to reapply the user's choice after a respawn (see the reapply block next
/// to the model one in spawn_and_handshake). Absent from the map means
/// "auto" - no entry is written for that case.
fn effort_map_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("session-reasoning-effort.json"))
}

static EFFORT_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load_effort_map() -> HashMap<String, String> {
    let Ok(path) = effort_map_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_effort_map(map: &HashMap<String, String>) -> Result<(), String> {
    let path = effort_map_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn get_session_effort(session_id: &str) -> Option<String> {
    load_effort_map().get(session_id).cloned()
}

pub fn set_session_effort(session_id: &str, effort: &str) -> Result<(), String> {
    let _lock = EFFORT_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_effort_map();
    if effort.is_empty() {
        map.remove(session_id);
    } else {
        map.insert(session_id.to_string(), effort.to_string());
    }
    save_effort_map(&map)
}

pub fn remove_session_effort(session_id: &str) -> Result<(), String> {
    let _lock = EFFORT_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_effort_map();
    if map.remove(session_id).is_some() {
        save_effort_map(&map)?;
    }
    Ok(())
}

/// zero-desktop's own record of each session's ACP permission mode ("auto" |
/// "ask" | "spec-draft" - Plan Mode) and, while one is awaiting a decision,
/// the spec it drafted. Needed for the same reason the model map is: every
/// freshly (re)spawned `zero acp` process registers its session pinned to
/// `PermissionModeAuto` (see `registerSession` in my-zero's
/// internal/acp/agent.go) - the engine itself has no memory of a session
/// having been in spec-draft, so a mode switch here would otherwise be lost
/// across any crash/respawn, and a plan awaiting review would vanish if the
/// app is closed before the user decides. Persisted (not just held in the
/// live `AcpSession`) so both survive a full app restart, not only a process
/// respawn.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionPlanState {
    #[serde(default = "default_plan_mode")]
    pub mode: String,
    #[serde(rename = "pendingSpec", skip_serializing_if = "Option::is_none", default)]
    pub pending_spec: Option<PendingSpec>,
}

impl Default for SessionPlanState {
    fn default() -> Self {
        Self { mode: default_plan_mode(), pending_spec: None }
    }
}

fn default_plan_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSpec {
    pub spec_id: String,
    pub title: String,
    pub file_path: String,
    pub relative_path: String,
}

fn plan_state_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("session-plan-state.json"))
}

static PLAN_STATE_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load_plan_state_map() -> HashMap<String, SessionPlanState> {
    let Ok(path) = plan_state_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_plan_state_map(map: &HashMap<String, SessionPlanState>) -> Result<(), String> {
    let path = plan_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn get_session_plan_state(session_id: &str) -> Option<SessionPlanState> {
    load_plan_state_map().get(session_id).cloned()
}

pub fn set_session_mode(session_id: &str, mode: &str) -> Result<(), String> {
    let _lock = PLAN_STATE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_plan_state_map();
    map.entry(session_id.to_string()).or_default().mode = mode.to_string();
    save_plan_state_map(&map)
}

pub fn set_pending_spec(session_id: &str, spec: PendingSpec) -> Result<(), String> {
    let _lock = PLAN_STATE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_plan_state_map();
    map.entry(session_id.to_string()).or_default().pending_spec = Some(spec);
    save_plan_state_map(&map)
}

pub fn clear_pending_spec(session_id: &str) -> Result<(), String> {
    let _lock = PLAN_STATE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_plan_state_map();
    if let Some(entry) = map.get_mut(session_id) {
        entry.pending_spec = None;
        return save_plan_state_map(&map);
    }
    Ok(())
}

pub fn remove_session_plan_state(session_id: &str) -> Result<(), String> {
    let _lock = PLAN_STATE_FILE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut map = load_plan_state_map();
    if map.remove(session_id).is_some() {
        save_plan_state_map(&map)?;
    }
    Ok(())
}

/// A model's reasoning-effort capability, sourced from `zero providers models
/// --json`'s `reasoning`/`reasoningEfforts` fields (see internal/cli/
/// provider_models.go in the zero CLI). `reasoning_efforts` is empty when the
/// model doesn't support discrete effort tiers - the frontend hides the
/// effort selector in that case, mirroring how the TUI's /effort picker only
/// offers "auto" for such models.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelCapabilityInfo {
    pub reasoning: bool,
    #[serde(rename = "reasoningEfforts")]
    pub reasoning_efforts: Vec<String>,
}

/// The active provider's live model list, for the model picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableModels {
    pub models: Vec<String>,
    pub active: String,
    /// Additive, keyed by model id - `models` itself stays a plain string
    /// list so existing consumers (model picker filtering/recents) don't
    /// need to change shape.
    #[serde(default)]
    pub capabilities: HashMap<String, ModelCapabilityInfo>,
}

/// Active provider's `name` and `model` via `zero config --json`. `name`
/// doubles as the `<catalog-id>` argument `zero providers add` expects when
/// updating an existing profile - verified live (`zero providers add
/// opencode-go --name opencode-go --model <x> --set-active` updates the
/// profile in place, no duplicate created) for a profile that was created
/// without an explicit `--name`, which is zero's default. Neither `zero
/// config --json`, `zero providers current --json`, nor `zero providers
/// list --json` expose the real `catalogID` field at all (checked all
/// three) - it only exists in `~/.config/zero/config.json` itself - so if a
/// renamed profile ever breaks this assumption, that file is the fallback
/// source, not another CLI flag.
async fn active_provider_entry() -> Option<(String, String)> {
    let zero_path = locate_zero().ok()?.path;
    let output = Command::new(&zero_path)
        .arg("config")
        .arg("--json")
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let active_provider = value["activeProvider"].as_str()?.to_string();
    let model = value["providers"]
        .as_array()?
        .iter()
        .find(|p| p["name"].as_str() == Some(active_provider.as_str()))
        .and_then(|p| p["model"].as_str())?
        .to_string();
    Some((active_provider, model))
}

/// Best-effort read of the currently active provider's model. Used to
/// snapshot which model is answering a session, since nothing in `zero
/// sessions list` or the ACP protocol reports it after the fact.
async fn active_model_id() -> Option<String> {
    active_provider_entry().await.map(|(_, model)| model)
}

/// The active provider's live model list (a real network probe against the
/// provider's own `/v1/models`-style endpoint - not instant, not cached, per
/// `zero providers models`'s own help text) plus which one is active.
pub async fn fetch_available_models() -> Result<AvailableModels, String> {
    let (provider_name, active_model) = active_provider_entry()
        .await
        .ok_or_else(|| "Failed to resolve the active zero provider".to_string())?;

    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;
    let output = Command::new(&zero_path)
        .arg("providers")
        .arg("models")
        .arg(&provider_name)
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers models: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers models failed: {stderr}"));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse providers models JSON: {e}"))?;
    let empty = Vec::new();
    let entries = value["models"].as_array().unwrap_or(&empty);
    let models: Vec<String> = entries
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .collect();
    let mut capabilities = HashMap::new();
    for entry in entries {
        let Some(id) = entry["id"].as_str() else { continue };
        let reasoning = entry["reasoning"].as_bool().unwrap_or(false);
        if !reasoning {
            continue;
        }
        let reasoning_efforts = entry["reasoningEfforts"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        capabilities.insert(id.to_string(), ModelCapabilityInfo { reasoning, reasoning_efforts });
    }

    Ok(AvailableModels { models, active: active_model, capabilities })
}

/// Appends the advisor-mode instruction (if enabled) to `content`, for use
/// only in the text sent to `zero` via `session/prompt` - never in the
/// persisted history or the title derived from the user's raw message.
fn apply_advisor_instruction(content: &str, config: &crate::advisor::AdvisorConfig) -> String {
    match crate::advisor::executor_instruction_prompt(config) {
        Some(instruction) => format!("{content}{instruction}"),
        None => content.to_string(),
    }
}

/// Resolves the advisor config to use when (re)inserting a session into the
/// map: the existing session's own config when one is already tracked under
/// this key (preserves it across a respawn - a dead process reconnecting
/// under the same key, see AcpSession's doc comment: "the next send()
/// respawns it"), otherwise the global default for a key that has
/// genuinely never been seen before.
///
/// Before this, `ZeroBridge::start` unconditionally called
/// `advisor::load_global_config()` on every (re)insert, which silently
/// reverted `enabled` and the chosen advisor model back to global defaults
/// on every respawn, with no user action - while the on-disk specialist
/// file (last synced before the respawn, see `advisor::sync_specialist_model`)
/// stayed pointed at whatever model the user had actually chosen. Nothing
/// looked broken (the file on disk was still correct, and the UI's model
/// picker just re-mirrored whatever `get_advisor_config` returned) until
/// the next consultation silently used the wrong model, because the
/// in-memory config actually driving `apply_advisor_instruction`'s
/// `enabled` check - and `get_advisor_config`'s response, which the
/// frontend trusts over its own local state on every reconnect - had reset.
fn advisor_config_for_restart(existing: Option<&crate::advisor::AdvisorConfig>) -> crate::advisor::AdvisorConfig {
    match existing {
        Some(config) => config.clone(),
        None => crate::advisor::load_global_config(),
    }
}

fn derive_title_from_message(content: &str) -> String {
    const MAX_CHARS: usize = 60;
    let cleaned = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return "New session".to_string();
    }
    if cleaned.chars().count() > MAX_CHARS {
        let truncated: String = cleaned.chars().take(MAX_CHARS).collect();
        format!("{truncated}…")
    } else {
        cleaned.to_string()
    }
}

/// A permission request from the agent, awaiting the user's decision.
/// `reply_id` is the original JSON-RPC request id (echoed back verbatim
/// when we answer it). `payload` is the translated request (toolName/reason)
/// kept around so `respond_to_permission` can persist a matching decision
/// record - without it, history replay had no way to tell an answered
/// request apart from one abandoned mid-session, and showed both as expired.
struct PendingPermission {
    reply_id: serde_json::Value,
    payload: serde_json::Value,
    session_key: String,
}

/// A live `zero acp` child process plus the peer used to talk to it.
struct LiveProcess {
    child: Child,
    peer: AcpPeer,
}

/// State for one active zero session. `live` is `None` when the process has
/// been killed (cancelled, or crashed) but the session is still logically
/// tracked - the next `send()` respawns it and reattaches via `session/load`.
struct AcpSession {
    cwd: PathBuf,
    session_id: String,
    history_path: PathBuf,
    live: Option<LiveProcess>,
    /// Configuração do advisor para esta sessão.
    advisor_config: crate::advisor::AdvisorConfig,
}

/// Bridge that manages a `zero acp` child process per active session and
/// forwards translated events to the frontend keyed by session.
///
/// One process per session (not shared across sessions/workspaces): even
/// though cancelling a turn no longer requires killing the process (see
/// `ZeroBridge::cancel`), a crashed or explicitly stopped process still
/// takes its session down - a single process shared across sessions would
/// take every other open conversation down with it, so each session still
/// gets its own.
pub struct ZeroBridge {
    app: tauri::AppHandle,
    sessions: Arc<Mutex<HashMap<String, AcpSession>>>,
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
}

impl ZeroBridge {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn `zero acp`, complete the `initialize` handshake, and open a
    /// session (`session/load` when `resume_id` is given, falling back to
    /// `session/new` if that fails; plain `session/new` otherwise). Spawns
    /// the stdout reader loop and the stderr forwarder. Does not touch
    /// `self.sessions` - callers install the result.
    ///
    /// `known_history_path` is `Some` when the caller already knows which
    /// session this will be (resuming, or respawning after a cancel) and
    /// `None` for a genuinely new session, where the real session id (and
    /// thus the history path) is only known after `session/new` responds.
    /// The reader task is spawned before that response can possibly arrive
    /// (it's the only thing that can resolve it), so the path is threaded
    /// through a shared cell it re-checks per event rather than being
    /// captured once - a brand new session's early events used to be
    /// silently written to a shared placeholder file instead of
    /// `<real-session-id>.jsonl`, which is why history replay is the thing
    /// to check first if this regresses again.
    ///
    /// `session_key` is the frontend-owned routing key; all emitted events
    /// carry it so listeners can filter by panel.
    async fn spawn_and_handshake(
        &self,
        session_key: &str,
        cwd: &Path,
        resume_id: Option<&str>,
        known_history_path: Option<PathBuf>,
    ) -> Result<(Child, AcpPeer, String), String> {
        let zero_path = locate_zero()
            .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
            .path;

        let mut child = Command::new(&zero_path)
            .arg("acp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn zero acp: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to open stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to open stderr".to_string())?;

        let peer = AcpPeer::new(stdin);

        let app_stderr = self.app.clone();
        let stderr_key = session_key.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let payload = serde_json::json!({
                    "sessionKey": stderr_key.clone(),
                    "line": line,
                });
                let _ = app_stderr.emit("zero:stderr", payload);
            }
        });

        let history_cell: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(known_history_path));
        self.spawn_stdout_reader(session_key.to_string(), peer.clone(), stdout, history_cell.clone());

        peer.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": 1,
                "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } },
            }),
        )
        .await
        .map_err(|e| format!("ACP initialize failed: {e}"))?;

        let session_id = match resume_id {
            Some(id) => {
                match peer
                    .request("session/load", serde_json::json!({ "sessionId": id }))
                    .await
                {
                    Ok(_) => id.to_string(),
                    Err(e) => {
                        log::warn!("[bridge] session/load({id}) failed, starting a new session instead: {e}");
                        let new_id = self.session_new(&peer, cwd).await?;
                        *history_cell.lock().await = Some(history_path_for(&new_id)?);
                        new_id
                    }
                }
            }
            None => {
                let new_id = self.session_new(&peer, cwd).await?;
                *history_cell.lock().await = Some(history_path_for(&new_id)?);
                new_id
            }
        };

        // If this session ever had its model switched via `_zero/set_model`
        // (see `ZeroBridge::switch_session_model`), re-apply that choice
        // after every handshake (session/new, session/load, or
        // session/load-failed-so-fell-back-to-session/new) - a fresh
        // `zero acp` process otherwise starts each session on the
        // *provider's* default model, silently discarding a per-session
        // choice across a crash/respawn. A session that never had its own
        // model set falls back to snapshotting the provider default, same
        // as before.
        if let Some(desired_model) = get_session_model(&session_id) {
            if let Err(e) = peer
                .request(
                    "_zero/set_model",
                    serde_json::json!({ "sessionId": session_id, "model": desired_model }),
                )
                .await
            {
                log::warn!("[bridge] failed to reapply session model '{desired_model}' after respawn: {e}");
            }
        } else if let Some(model_id) = active_model_id().await {
            let _ = set_session_model(&session_id, &model_id);
        }

        // Same respawn-reapply need as the model block above: a session whose
        // reasoning-effort was set via `_zero/set_effort` (see
        // `ZeroBridge::switch_session_effort`) must have that choice restored
        // after every handshake, or a fresh `zero acp` process silently falls
        // back to "auto". Absent from the map simply means "auto" - no
        // fallback snapshot needed, unlike the model block above.
        if let Some(desired_effort) = get_session_effort(&session_id) {
            if let Err(e) = peer
                .request(
                    "_zero/set_effort",
                    serde_json::json!({ "sessionId": session_id, "effort": desired_effort }),
                )
                .await
            {
                log::warn!("[bridge] failed to reapply session effort '{desired_effort}' after respawn: {e}");
            }
        }

        // Same respawn-reapply need as the model block above, gated to
        // spec-draft only: "auto"/"ask" don't need reapplying - "auto" is
        // already registerSession's own default for a brand-new Go-side
        // session, and "ask" is never something we must silently restore
        // after a crash (only a still-pending plan review matters enough to
        // survive one, and that lives in spec-draft while awaiting a
        // decision).
        if get_session_plan_state(&session_id).map(|s| s.mode).as_deref() == Some("spec-draft") {
            if let Err(e) = peer
                .request(
                    "session/set_mode",
                    serde_json::json!({ "sessionId": session_id, "modeId": "spec-draft" }),
                )
                .await
            {
                log::warn!("[bridge] failed to reapply spec-draft mode after respawn: {e}");
            }
        }

        Ok((child, peer, session_id))
    }

    async fn session_new(&self, peer: &AcpPeer, cwd: &Path) -> Result<String, String> {
        let result = peer
            .request(
                "session/new",
                serde_json::json!({ "cwd": cwd.to_string_lossy(), "mcpServers": [] }),
            )
            .await
            .map_err(|e| format!("session/new failed: {e}"))?;
        result["sessionId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "session/new response missing sessionId".to_string())
    }

    /// The only task reading a process's stdout. Resolves responses to our
    /// own requests, translates `session/update` notifications into
    /// `zero:event` (also appending them to the local history file), and
    /// surfaces `session/request_permission` as `zero:permission-request` -
    /// the part that's actually new: a permission request the frontend can
    /// answer for real, unlike the old exec transport.
    ///
    /// `agent_thought_chunk`/`agent_message_chunk` arrive as many small
    /// deltas per turn; they're buffered here (`pending_thinking`/
    /// `pending_text`) and only written to history as one coalesced
    /// `reasoning`/`message` entry - at whichever of tool-call, permission
    /// request, or turn end (the `session/prompt` response, identified by a
    /// `stopReason` field) comes first. Live streaming to the frontend is
    /// untouched; only what lands in the on-disk history changes.
    fn spawn_stdout_reader(
        &self,
        session_key: String,
        peer: AcpPeer,
        stdout: ChildStdout,
        history_path: Arc<Mutex<Option<PathBuf>>>,
    ) {
        let app = self.app.clone();
        let pending_permissions = self.pending_permissions.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut pending_thinking = String::new();
            let mut pending_text = String::new();

            while let Ok(Some(line)) = lines.next_line().await {
                let Some(msg) = parse_line(&line) else {
                    log::error!("[bridge] failed to parse acp line: {line}");
                    let payload = serde_json::json!({
                        "sessionKey": session_key.clone(),
                        "line": format!("[unparsed] {line}"),
                    });
                    let _ = app.emit("zero:stderr", payload);
                    continue;
                };

                match msg {
                    AcpMessage::Response { id, result } => {
                        let is_turn_end = result
                            .as_ref()
                            .ok()
                            .and_then(|v| v.get("stopReason"))
                            .is_some();
                        if is_turn_end {
                            if let Some(path) = history_path.lock().await.clone() {
                                flush_pending_reasoning(&path, &mut pending_thinking).await;
                                flush_pending_message(&path, "assistant", &mut pending_text).await;
                            } else {
                                pending_thinking.clear();
                                pending_text.clear();
                            }
                        }
                        peer.resolve_response(id, result).await;
                    }
                    AcpMessage::Notification { method, params } => {
                        if method != "session/update" {
                            continue;
                        }
                        let kind = params
                            .get("update")
                            .and_then(|u| u.get("sessionUpdate"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let event = translate_session_update(&params);

                        match kind.as_str() {
                            "agent_thought_chunk" => {
                                if let Some(ref e) = event {
                                    pending_thinking.push_str(e.payload["delta"].as_str().unwrap_or(""));
                                }
                            }
                            "agent_message_chunk" => {
                                if let Some(path) = history_path.lock().await.clone() {
                                    flush_pending_reasoning(&path, &mut pending_thinking).await;
                                } else {
                                    pending_thinking.clear();
                                }
                                if let Some(ref e) = event {
                                    pending_text.push_str(e.payload["delta"].as_str().unwrap_or(""));
                                }
                            }
                            "tool_call" | "tool_call_update" | "plan" => {
                                if let Some(path) = history_path.lock().await.clone() {
                                    flush_pending_reasoning(&path, &mut pending_thinking).await;
                                    if let Some(ref e) = event {
                                        append_history(&path, &e.event_type, &e.payload).await;
                                    }
                                } else {
                                    pending_thinking.clear();
                                }
                            }
                            // Not appended to chat history (unlike the arm above) -
                            // the durable record is the .zero/specs/*.md file plus
                            // this session-plan-state.json entry; submit_spec's own
                            // tool_call/tool_call_update (emitted separately by the
                            // engine regardless) already leaves a normal trace in
                            // the replayed transcript. Persisted here, keyed by the
                            // stable session_id (only session_key is in scope in
                            // this reader, resolved via the shared sessions map) so
                            // it survives a respawn or a full app restart, not just
                            // this in-memory turn - see SessionPlanState.
                            "_zero/spec_review_required" => {
                                if let Some(ref e) = event {
                                    let session_id = sessions
                                        .lock()
                                        .await
                                        .get(&session_key)
                                        .map(|s| s.session_id.clone());
                                    if let Some(session_id) = session_id {
                                        let spec = PendingSpec {
                                            spec_id: e.payload["specId"].as_str().unwrap_or("").to_string(),
                                            title: e.payload["title"].as_str().unwrap_or("").to_string(),
                                            file_path: e.payload["filePath"].as_str().unwrap_or("").to_string(),
                                            relative_path: e.payload["relativePath"].as_str().unwrap_or("").to_string(),
                                        };
                                        let _ = set_pending_spec(&session_id, spec);
                                    }
                                }
                            }
                            _ => {}
                        }

                        if let Some(event) = event {
                            let _ = app.emit("zero:event", event.with_session_key(&session_key));
                        }
                    }
                    AcpMessage::Request { id, method, params } => {
                        if method != "session/request_permission" {
                            continue;
                        }
                        let correlation_id = format!(
                            "perm-{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_nanos())
                                .unwrap_or_default()
                        );
                        log::info!("[bridge] permission request received: correlation_id={correlation_id} reply_id={id} session_key={session_key}");
                        let payload = translate_permission_request(&correlation_id, &params);
                        pending_permissions.lock().await.insert(
                            correlation_id.clone(),
                            PendingPermission {
                                reply_id: id.clone(),
                                payload: payload.clone(),
                                session_key: session_key.clone(),
                            },
                        );
                        if let Some(path) = history_path.lock().await.clone() {
                            flush_pending_reasoning(&path, &mut pending_thinking).await;
                            append_history(&path, "permission_request", &payload).await;
                        } else {
                            pending_thinking.clear();
                        }
                        let payload_with_key = {
                            let mut v = payload;
                            v["sessionKey"] = serde_json::Value::String(session_key.clone());
                            v
                        };
                        let _ = app.emit("zero:permission-request", payload_with_key);
                    }
                }
            }
            let payload = serde_json::json!({ "sessionKey": session_key });
            let _ = app.emit("zero:process-exited", payload);
        });
    }

    async fn kill_live(live: &mut LiveProcess) {
        live.child.kill().await.ok();
        let _ = live.child.wait().await;
    }

    pub async fn start(&self, key: String, cwd: PathBuf, resume_id: Option<String>) -> Result<StartedSession, String> {
        {
            let sessions = self.sessions.lock().await;
            if let Some(existing) = sessions.get(&key) {
                if existing.live.is_some() {
                    // Already running under this key - reconnect without spawning a second process.
                    return Ok(StartedSession {
                        key,
                        session_id: existing.session_id.clone(),
                        reattached: true,
                    });
                }
            }
            // No global process cap - the user may run as many concurrent
            // `zero acp` sessions as their machine can handle. The frontend
            // enforces a per-workspace panel limit instead.
        }

        let known_history_path = match resume_id.as_deref() {
            Some(id) => Some(history_path_for(id)?),
            None => None,
        };
        let (child, peer, session_id) = self
            .spawn_and_handshake(&key, &cwd, resume_id.as_deref(), known_history_path)
            .await?;
        let history_path = history_path_for(&session_id)?;

        let mut sessions = self.sessions.lock().await;
        let advisor_config = advisor_config_for_restart(sessions.get(&key).map(|s| &s.advisor_config));

        sessions.insert(
            key.clone(),
            AcpSession {
                cwd,
                session_id: session_id.clone(),
                history_path,
                live: Some(LiveProcess { child, peer }),
                advisor_config,
            },
        );

        Ok(StartedSession {
            key,
            session_id,
            reattached: false,
        })
    }

    /// Ensure the session identified by `key` has a live process, respawning
    /// (and `session/load`-ing) if it was killed by `cancel()` or died on its
    /// own. Returns the peer, session id, and history path to use for the
    /// next request.
    async fn ensure_live(&self, key: &str) -> Result<(AcpPeer, String, PathBuf), String> {
        let (cwd, session_id, history_path, needs_respawn) = {
            let sessions = self.sessions.lock().await;
            let s = sessions
                .get(key)
                .ok_or_else(|| "No active zero session for this key".to_string())?;
            (
                s.cwd.clone(),
                s.session_id.clone(),
                s.history_path.clone(),
                s.live.is_none(),
            )
        };

        if needs_respawn {
            let (child, peer, resumed_id) = self
                .spawn_and_handshake(key, &cwd, Some(&session_id), Some(history_path))
                .await?;
            let mut sessions = self.sessions.lock().await;
            if let Some(s) = sessions.get_mut(key) {
                s.session_id = resumed_id;
                s.live = Some(LiveProcess { child, peer });
            }
        }

        let sessions = self.sessions.lock().await;
        let s = sessions
            .get(key)
            .ok_or_else(|| "No active zero session for this key".to_string())?;
        let live = s
            .live
            .as_ref()
            .ok_or_else(|| "Failed to reconnect to zero session".to_string())?;
        Ok((live.peer.clone(), s.session_id.clone(), s.history_path.clone()))
    }

    /// Send a user message, with an optional image attachment. Fires
    /// `session/prompt` in the background (it only resolves once the whole
    /// turn ends) and returns immediately - progress is tracked via
    /// `zero:event`/`zero:permission-request`, same as the app already
    /// expects.
    pub async fn send(&self, key: String, content: String, file: Option<FileAttachment>) -> Result<(), String> {
        let (peer, session_id, history_path) = self.ensure_live(&key).await?;

        // First message of a session (no title recorded yet) gives it a
        // real name instead of zero's generic "ACP session" default - there
        // is no discovered ACP method to set the title on zero's side, so
        // zero-desktop tracks it locally and overlays it in list_zero_sessions.
        // A file-only message (empty content) falls back to the file's
        // filename instead of deriving a title from an empty string.
        if get_session_title(&session_id).is_none() {
            let title_source = if content.trim().is_empty() {
                file.as_ref().map(|f| f.name.clone()).unwrap_or_default()
            } else {
                content.clone()
            };
            let _ = set_session_title(&session_id, &derive_title_from_message(&title_source));
        }

        // The user's own turn was never persisted before - only the agent's
        // side went through `spawn_stdout_reader`. Without this, replaying a
        // session's history showed the agent's tool calls/reasoning with no
        // record of what was actually asked. The `file` key is only added
        // when present, so text-only history entries stay identical to
        // before this field existed.
        let mut message_payload = serde_json::json!({ "role": "user", "content": content.clone() });
        if let Some(ref f) = file {
            message_payload["file"] = serde_json::json!({
                "mimeType": f.mime_type,
                "data": f.data,
                "name": f.name,
            });
        }
        append_history(&history_path, "message", &message_payload).await;

        // Advisor mode: append the instruction telling the executor to
        // delegate to the `advisor` specialist (via the `Task` tool) for
        // architectural/critical decisions - only in the text actually sent
        // to `zero`, never in the persisted history or the title derived
        // above, so the user's own message stays exactly what they typed.
        let advisor_config = self.get_advisor_config(&key).await.unwrap_or_default();
        let effective_content = apply_advisor_instruction(&content, &advisor_config);

        let app = self.app.clone();
        let key_for_event = key.clone();
        tokio::spawn(async move {
            let result = peer
                .request(
                    "session/prompt",
                    serde_json::json!({
                        "sessionId": session_id,
                        "prompt": build_prompt_blocks(&effective_content, file.as_ref()),
                    }),
                )
                .await;

            let event = match result {
                Ok(value) => OutputEvent::new(
                    "run_end",
                    serde_json::json!({
                        "status": "success",
                        "stopReason": value.get("stopReason").cloned().unwrap_or(serde_json::Value::Null),
                    }),
                ),
                Err(e) => OutputEvent::new("error", serde_json::json!({ "message": e })),
            };
            let _ = app.emit("zero:event", event.with_session_key(&key_for_event));
        });

        Ok(())
    }

    /// Answer a pending `session/request_permission` request from the
    /// agent. This is the actual payoff of the ACP migration: unlike the old
    /// exec transport, this reply really reaches the agent.
    pub async fn respond_to_permission(&self, request_id: String, option_id: String) -> Result<(), String> {
        log::info!("[bridge] respond_to_permission called: request_id={request_id} option_id={option_id}");
        let pending = self
            .pending_permissions
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| {
                log::warn!("[bridge] no pending permission request with id {request_id}");
                format!("No pending permission request with id {request_id}")
            })?;

        // Persist the decision itself, not just the request: without this,
        // history replay (buildMessagesFromHistory on the frontend) can't
        // distinguish "the user answered in time" from "the session ended
        // with no answer", and rendered both as an expired permission once
        // the periodic history resync overwrote the live in-memory state.
        let history_path = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&pending.session_key)
                .map(|s| s.history_path.clone())
        };
        if let Some(history_path) = history_path {
            let decision = serde_json::json!({
                "requestId": request_id,
                "toolName": pending.payload.get("toolName").cloned().unwrap_or_default(),
                "reason": pending.payload.get("reason").cloned().unwrap_or_default(),
                "action": if option_id.starts_with("allow") { "allow" } else { "deny" },
            });
            append_history(&history_path, "permission_decision", &decision).await;
        }

        log::info!("[bridge] sending permission reply: reply_id={} option_id={option_id}", pending.reply_id);
        let (peer, _, _) = self.ensure_live(&pending.session_key).await?;
        peer.respond(
            pending.reply_id,
            serde_json::json!({ "outcome": { "outcome": "selected", "optionId": option_id } }),
        )
        .await
    }

    /// Cancel the in-flight turn for the session identified by `key`. Sends
    /// `session/cancel` as a fire-and-forget JSON-RPC *notification* -
    /// `zero` registers it as a notification-only handler, not a request
    /// (`peer.request("session/cancel", ...)` gets "method not found").
    /// Verified live: the pending `session/prompt` call resolves with
    /// `stopReason: "cancelled"` and the process/session stays alive and
    /// responsive to the next prompt - no kill/respawn needed. Falls back to
    /// killing the process only if the notification itself can't be sent
    /// (e.g. the peer is already gone).
    pub async fn cancel(&self, key: String) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        let Some(s) = sessions.get_mut(&key) else {
            return Ok(());
        };
        let Some(live) = s.live.as_ref() else {
            return Ok(());
        };
        let peer = live.peer.clone();
        let session_id = s.session_id.clone();
        if let Err(e) = peer
            .notify("session/cancel", serde_json::json!({ "sessionId": session_id }))
            .await
        {
            log::warn!("[bridge] session/cancel notify failed ({e}); killing process as fallback");
            if let Some(mut live) = s.live.take() {
                Self::kill_live(&mut live).await;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, key: String) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut s) = sessions.remove(&key) {
            if let Some(mut live) = s.live.take() {
                Self::kill_live(&mut live).await;
            }
        }
        Ok(())
    }

    /// Switch this session's model in place via `_zero/set_model`. Per-
    /// session and takes effect on the next turn - no config mutation, no
    /// kill/respawn (unlike the old `switch_active_model` +
    /// `cancel`-to-force-respawn combo, which mutated the *global* provider
    /// config via `zero providers add --set-active`, affecting every `zero`
    /// process on the machine). If the session has no live process right
    /// now, the choice is only persisted to `session-models.json` and gets
    /// re-applied automatically the next time this session spawns (see the
    /// respawn snapshot logic in `spawn_and_handshake`).
    pub async fn switch_session_model(&self, key: String, model: String) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let Some(s) = sessions.get(&key) else {
            return Err(format!("No active session for key: {key}"));
        };
        let session_id = s.session_id.clone();
        let live_peer = s.live.as_ref().map(|live| live.peer.clone());
        drop(sessions);

        if let Some(peer) = live_peer {
            peer.request(
                "_zero/set_model",
                serde_json::json!({ "sessionId": session_id, "model": model }),
            )
            .await
            .map_err(|e| format!("_zero/set_model failed: {e}"))?;
        }

        set_session_model(&session_id, &model)
    }

    /// Switch this session's reasoning-effort preference in place via
    /// `_zero/set_effort`. Mirrors switch_session_model exactly: per-session,
    /// takes effect on the next turn, no kill/respawn. If the session has no
    /// live process right now, the choice is only persisted to
    /// `session-reasoning-effort.json` and gets re-applied automatically the
    /// next time this session spawns (see the reapply block in
    /// `spawn_and_handshake`). `effort` is `""` for "auto".
    pub async fn switch_session_effort(&self, key: String, effort: String) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let Some(s) = sessions.get(&key) else {
            return Err(format!("No active session for key: {key}"));
        };
        let session_id = s.session_id.clone();
        let live_peer = s.live.as_ref().map(|live| live.peer.clone());
        drop(sessions);

        if let Some(peer) = live_peer {
            peer.request(
                "_zero/set_effort",
                serde_json::json!({ "sessionId": session_id, "effort": effort }),
            )
            .await
            .map_err(|e| format!("_zero/set_effort failed: {e}"))?;
        }

        set_session_effort(&session_id, &effort)
    }

    /// Switch this session's ACP permission mode in place via
    /// `session/set_mode` ("auto" | "ask" | "spec-draft" - Plan Mode, the
    /// ACP-native equivalent of Claude Code's ExitPlanMode). Live - no
    /// kill/respawn needed, unlike model switching: `handleSetMode` on the Go
    /// side mutates the already-running session object directly. If there's
    /// no live process right now, the choice is only persisted to
    /// `session-plan-state.json` and gets re-applied automatically next
    /// spawn (see the reapply block in `spawn_and_handshake`) - though in
    /// practice a disconnected session should go through
    /// `set_zero_session_mode_by_id` instead, since this method requires a
    /// registered `key` entry to resolve the session id from.
    pub async fn switch_session_mode(&self, key: String, mode: String) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let Some(s) = sessions.get(&key) else {
            return Err(format!("No active session for key: {key}"));
        };
        let session_id = s.session_id.clone();
        let live_peer = s.live.as_ref().map(|live| live.peer.clone());
        drop(sessions);

        if let Some(peer) = live_peer {
            peer.request(
                "session/set_mode",
                serde_json::json!({ "sessionId": session_id, "modeId": mode }),
            )
            .await
            .map_err(|e| format!("session/set_mode failed: {e}"))?;
        }

        set_session_mode(&session_id, &mode)
    }

    /// Retorna o advisor config para uma sessão específica.
    pub async fn get_advisor_config(&self, key: &str) -> Option<crate::advisor::AdvisorConfig> {
        let sessions = self.sessions.lock().await;
        sessions.get(key).map(|s| s.advisor_config.clone())
    }

    /// Atualiza o advisor config para uma sessão específica. Também
    /// resincroniza o `model:` do frontmatter do specialist `advisor`
    /// (`.zero/specialists/advisor.md` no workspace desta sessão) com o
    /// modelo escolhido - é isso que faz o advisor de fato rodar num modelo
    /// diferente do executor (ver `advisor::sync_specialist_model`).
    pub async fn set_advisor_config(&self, key: &str, config: crate::advisor::AdvisorConfig) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(key) else {
            return Err(format!("No active session for key: {key}"));
        };
        let workspace_root = session.cwd.clone();
        let model = config.model.clone();
        session.advisor_config = config;
        drop(sessions);

        let _ = crate::advisor::sync_specialist_model(&workspace_root, model.as_deref());
        Ok(())
    }

    /// Kill every live process and clear the session map. Used when the app
    /// exits so no orphan `zero acp` processes remain.
    pub async fn kill_all(&self) {
        let mut sessions = self.sessions.lock().await;
        for (_, mut s) in sessions.drain() {
            if let Some(mut live) = s.live.take() {
                Self::kill_live(&mut live).await;
            }
        }
    }

    /// Return the list of tracked sessions for frontend reconciliation.
    pub async fn list_live_sessions(&self) -> Vec<LiveSessionInfo> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|(key, s)| LiveSessionInfo {
                key: key.clone(),
                session_id: s.session_id.clone(),
                cwd: s.cwd.clone(),
                live: s.live.is_some(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_from_call_prefers_title() {
        assert_eq!(tool_name_from_call("call_00_abc", Some("edit_file note.txt")), "edit_file");
        assert_eq!(tool_name_from_call("call_00_abc", Some("write_file")), "write_file");
    }

    #[test]
    fn test_tool_name_from_call_strips_counter_when_no_title() {
        assert_eq!(tool_name_from_call("read_file_0", None), "read_file");
        assert_eq!(tool_name_from_call("list_directory_12", None), "list_directory");
        assert_eq!(tool_name_from_call("mcp_firecrawl_scrape_3", None), "mcp_firecrawl_scrape");
    }

    #[test]
    fn test_tool_name_from_call_no_counter_suffix_when_no_title() {
        assert_eq!(tool_name_from_call("request_permissions", None), "request_permissions");
    }

    #[test]
    fn test_translate_agent_message_chunk() {
        let params = serde_json::json!({
            "sessionId": "s1",
            "update": { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "hi" } }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "text");
        assert_eq!(event.payload["delta"], "hi");
    }

    #[test]
    fn test_translate_agent_thought_chunk() {
        let params = serde_json::json!({
            "update": { "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "thinking..." } }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "reasoning");
        assert_eq!(event.payload["delta"], "thinking...");
    }

    #[test]
    fn test_translate_tool_call() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call",
                "toolCallId": "read_file_0",
                "title": "read_file note.txt",
                "kind": "read",
                "status": "in_progress",
                "rawInput": { "path": "note.txt" }
            }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "tool_call");
        assert_eq!(event.payload["id"], "read_file_0");
        assert_eq!(event.payload["name"], "read_file");
        assert_eq!(event.payload["args"]["path"], "note.txt");
    }

    #[test]
    fn test_translate_tool_call_update_success() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "read_file_0",
                "status": "completed",
                "content": [{ "type": "content", "content": { "type": "text", "text": "file contents" } }]
            }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "tool_result");
        assert_eq!(event.payload["status"], "ok");
        assert_eq!(event.payload["output"], "file contents");
    }

    #[test]
    fn test_translate_tool_call_update_failure() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "web_fetch_0",
                "status": "failed",
                "content": [{ "type": "content", "content": { "type": "text", "text": "network denied" } }]
            }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.payload["status"], "error");
        assert_eq!(event.payload["output"], "network denied");
    }

    #[test]
    fn test_translate_plan_update() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "plan",
                "entries": [
                    { "content": "Step one", "status": "in_progress", "priority": 0 },
                    { "content": "Step two", "status": "pending", "priority": 1 }
                ]
            }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "plan_update");
        let entries = event.payload["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["content"], "Step one");
        assert_eq!(entries[0]["status"], "in_progress");
    }

    #[test]
    fn test_translate_spec_review_required() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "_zero/spec_review_required",
                "specId": "2026-07-17-add-foo",
                "title": "Add foo",
                "filePath": "/home/user/project/.zero/specs/2026-07-17-add-foo.md",
                "relativePath": ".zero/specs/2026-07-17-add-foo.md"
            }
        });
        let event = translate_session_update(&params).unwrap();
        assert_eq!(event.event_type, "spec_review_required");
        assert_eq!(event.payload["specId"], "2026-07-17-add-foo");
        assert_eq!(event.payload["title"], "Add foo");
        assert_eq!(
            event.payload["filePath"],
            "/home/user/project/.zero/specs/2026-07-17-add-foo.md"
        );
        assert_eq!(event.payload["relativePath"], ".zero/specs/2026-07-17-add-foo.md");
    }

    #[test]
    fn test_translate_unknown_update_kind_ignored() {
        let params = serde_json::json!({ "update": { "sessionUpdate": "totally_unknown_kind" } });
        assert!(translate_session_update(&params).is_none());
    }

    #[test]
    fn test_translate_permission_request() {
        let params = serde_json::json!({
            "sessionId": "s1",
            "toolCall": {
                "toolCallId": "request_permissions_1",
                "title": "request_permissions",
                "rawInput": { "reason": "Need write permission." }
            },
            "options": [
                { "optionId": "allow", "name": "Allow", "kind": "allow_once" },
                { "optionId": "reject_once", "name": "Reject", "kind": "reject_once" }
            ]
        });
        let payload = translate_permission_request("corr-1", &params);
        assert_eq!(payload["requestId"], "corr-1");
        assert_eq!(payload["toolName"], "request_permissions");
        assert_eq!(payload["reason"], "Need write permission.");
        assert_eq!(payload["options"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_output_event_serialize_roundtrip() {
        let original = OutputEvent::new("text", serde_json::json!({ "delta": "hello" }));
        let json = serde_json::to_string(&original).unwrap();
        let parsed: OutputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, original.schema_version);
        assert_eq!(parsed.event_type, original.event_type);
        assert_eq!(parsed.payload["delta"], original.payload["delta"]);
    }

    #[test]
    fn test_output_event_with_session_key() {
        let event = OutputEvent::new("text", serde_json::json!({ "delta": "hello" }))
            .with_session_key("key-1");
        assert_eq!(event.payload["sessionKey"], "key-1");
    }

    fn test_image() -> FileAttachment {
        FileAttachment {
            mime_type: "image/png".to_string(),
            data: "iVBORw0KGgo=".to_string(),
            name: "screenshot.png".to_string(),
        }
    }

    #[test]
    fn test_build_prompt_blocks_text_only() {
        let blocks = build_prompt_blocks("hello", None);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["text"], "hello");
    }

    #[test]
    fn test_build_prompt_blocks_image_only() {
        let img = test_image();
        let blocks = build_prompt_blocks("", Some(&img));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "image");
        assert_eq!(blocks[0]["mimeType"], "image/png");
        assert_eq!(blocks[0]["data"], "iVBORw0KGgo=");
    }

    #[test]
    fn test_build_prompt_blocks_text_and_image() {
        let img = test_image();
        let blocks = build_prompt_blocks("what is this?", Some(&img));
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
    }

    #[test]
    fn test_build_prompt_blocks_binary_attachment_becomes_named_reference() {
        // A genuinely binary, non-image attachment (e.g. a PDF or zip) can't
        // be inlined as either an image or text content block - it still
        // must produce SOME block referencing it by name, not silently
        // vanish from the prompt.
        let file = FileAttachment {
            mime_type: "application/pdf".to_string(),
            data: "JVBERi0xLjQK".to_string(),
            name: "report.pdf".to_string(),
        };
        let blocks = build_prompt_blocks("check this out", Some(&file));
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "text");
        let attached_text = blocks[1]["text"].as_str().unwrap();
        assert!(attached_text.contains("report.pdf"));
        assert!(attached_text.contains("application/pdf"));
    }

    #[test]
    fn test_apply_advisor_instruction_disabled_leaves_content_unchanged() {
        let config = crate::advisor::AdvisorConfig::default();
        assert_eq!(apply_advisor_instruction("oi, tudo bem?", &config), "oi, tudo bem?");
    }

    #[test]
    fn test_apply_advisor_instruction_enabled_appends_prompt() {
        let config = crate::advisor::AdvisorConfig {
            enabled: true,
            model: None,
            mode: crate::advisor::AdvisorMode::Max,
        };
        let result = apply_advisor_instruction("oi, tudo bem?", &config);
        assert!(result.starts_with("oi, tudo bem?"), "user content must come first, unmodified");
        assert!(result.contains("advisor_mode"));
        assert!(result.contains("Task"));
    }

    #[test]
    fn test_apply_advisor_instruction_enabled_with_model_does_not_repeat_it() {
        // The model lives in the specialist file's own frontmatter (see
        // advisor::sync_specialist_model), not in the injected prompt - the
        // executor never needs it passed here.
        let config = crate::advisor::AdvisorConfig {
            enabled: true,
            model: Some("claude-opus-4-1".to_string()),
            mode: crate::advisor::AdvisorMode::Max,
        };
        let result = apply_advisor_instruction("refatora isso", &config);
        assert!(!result.contains("claude-opus-4-1"));
    }

    #[test]
    fn test_apply_advisor_instruction_low_mode_uses_restrictive_prompt() {
        let config = crate::advisor::AdvisorConfig {
            enabled: true,
            model: None,
            mode: crate::advisor::AdvisorMode::Low,
        };
        let result = apply_advisor_instruction("refatora isso", &config);
        assert!(result.contains("modo Low"));
        assert!(result.contains("Recuperação de falha repetida"));
    }

    #[test]
    fn test_advisor_config_for_restart_preserves_existing_session_config() {
        // The core regression: a respawn (dead process reconnecting under
        // the same key) must not lose the session's own advisor
        // enabled/model choice.
        let existing = crate::advisor::AdvisorConfig {
            enabled: true,
            model: Some("deepseek-v4-pro".to_string()),
            mode: crate::advisor::AdvisorMode::Low,
        };
        let result = advisor_config_for_restart(Some(&existing));
        assert!(result.enabled);
        assert_eq!(result.model, Some("deepseek-v4-pro".to_string()));
        assert_eq!(result.mode, crate::advisor::AdvisorMode::Low);
    }

    #[test]
    fn test_advisor_config_for_restart_falls_back_to_global_default_for_new_session() {
        // Compared against a direct load_global_config() call (not fixed
        // field values), so this doesn't depend on whatever this machine's
        // saved global config file actually contains.
        let result = advisor_config_for_restart(None);
        let expected = crate::advisor::load_global_config();
        assert_eq!(result.enabled, expected.enabled);
        assert_eq!(result.model, expected.model);
    }
}
