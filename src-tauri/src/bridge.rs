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
}

/// ACP's tool_call/tool_call_update don't carry a clean `name` field like
/// zero exec's tool_call event did (verified live) - `toolCallId` looks like
/// `"read_file_0"`. Best-effort recovery of the tool name by stripping the
/// trailing `_<digits>` counter.
fn tool_name_from_call_id(tool_call_id: &str) -> String {
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
        if let Some(kind) = attachment_kind_from_mime(&file.mime_type) {
            match kind {
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
            }
        }
    }
    blocks
}

fn attachment_kind_from_mime(mime_type: &str) -> Option<AttachmentKind> {
    if mime_type.starts_with("image/") {
        Some(AttachmentKind::Image)
    } else if mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/yaml"
        || mime_type == "application/xml"
    {
        Some(AttachmentKind::Text)
    } else {
        None
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
            let name = tool_name_from_call_id(tool_call_id);
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
    let fallback_name = tool_name_from_call_id(tool_call_id);
    let tool_name = tool_call["title"].as_str().unwrap_or(&fallback_name);
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
    let mut map = load_title_map();
    map.insert(session_id.to_string(), title.to_string());
    save_title_map(&map)
}

pub fn remove_session_title(session_id: &str) -> Result<(), String> {
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
    let mut map = load_model_map();
    map.insert(session_id.to_string(), model_id.to_string());
    save_model_map(&map)
}

pub fn remove_session_model(session_id: &str) -> Result<(), String> {
    let mut map = load_model_map();
    if map.remove(session_id).is_some() {
        save_model_map(&map)?;
    }
    Ok(())
}

/// The active provider's live model list, for the model picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableModels {
    pub models: Vec<String>,
    pub active: String,
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
    let models = value["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(AvailableModels { models, active: active_model })
}

/// Switches the active provider's model. This is a global, persisted zero
/// CLI/config change, not a per-session ACP call - verified live that ACP
/// has no such method (`session/set_model`/`session/models` both return
/// "method not found"), no env var influences it, and `zero acp` rejects an
/// unknown `--model` flag outright. Affects every `zero` process on this
/// machine, not just this app. Caller is responsible for restarting the
/// live process (see `ZeroBridge::cancel`) so the next turn picks up the
/// change - this function only performs the CLI mutation.
pub async fn switch_active_model(model: &str) -> Result<(), String> {
    let (provider_name, _) = active_provider_entry()
        .await
        .ok_or_else(|| "Failed to resolve the active zero provider".to_string())?;

    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;
    let output = Command::new(&zero_path)
        .arg("providers")
        .arg("add")
        .arg(&provider_name)
        .arg("--name")
        .arg(&provider_name)
        .arg("--model")
        .arg(model)
        .arg("--set-active")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers add: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to switch model: {stderr}"));
    }

    Ok(())
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
/// when we answer it).
struct PendingPermission {
    reply_id: serde_json::Value,
}

/// A live `zero acp` child process plus the peer used to talk to it.
struct LiveProcess {
    child: Child,
    peer: AcpPeer,
}

/// State for the app's one active zero session. `live` is `None` when the
/// process has been killed (cancelled, or crashed) but the session is still
/// logically "current" - the next `send()` respawns it and reattaches via
/// `session/load`.
struct AcpSession {
    cwd: PathBuf,
    session_id: String,
    history_path: PathBuf,
    live: Option<LiveProcess>,
}

/// Bridge that manages a `zero acp` child process per active session and
/// forwards translated events to the frontend.
///
/// One process per session (not shared across sessions/workspaces): `zero`
/// has no `session/cancel` method (verified live - "method not found"), so
/// interrupting a turn means killing the process. A single process shared
/// across sessions would take every other open conversation down with it,
/// so each session gets its own.
pub struct ZeroBridge {
    app: tauri::AppHandle,
    session: Arc<Mutex<Option<AcpSession>>>,
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
}

impl ZeroBridge {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            session: Arc::new(Mutex::new(None)),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn `zero acp`, complete the `initialize` handshake, and open a
    /// session (`session/load` when `resume_id` is given, falling back to
    /// `session/new` if that fails; plain `session/new` otherwise). Spawns
    /// the stdout reader loop and the stderr forwarder. Does not touch
    /// `self.session` - callers install the result.
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
    async fn spawn_and_handshake(
        &self,
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
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app_stderr.emit("zero:stderr", line);
            }
        });

        let history_cell: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(known_history_path));
        self.spawn_stdout_reader(peer.clone(), stdout, history_cell.clone());

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

        // Snapshotted here (after every successful handshake - session/new,
        // session/load, or session/load-failed-so-fell-back-to-session/new)
        // rather than only inside session_new(), so a respawn after a model
        // switch (see ZeroBridge::cancel/switch_active_model) re-snapshots
        // too - otherwise the session list would keep showing the model that
        // was active when the session was first created.
        if let Some(model_id) = active_model_id().await {
            let _ = set_session_model(&session_id, &model_id);
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
        peer: AcpPeer,
        stdout: ChildStdout,
        history_path: Arc<Mutex<Option<PathBuf>>>,
    ) {
        let app = self.app.clone();
        let pending_permissions = self.pending_permissions.clone();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut pending_thinking = String::new();
            let mut pending_text = String::new();

            while let Ok(Some(line)) = lines.next_line().await {
                let Some(msg) = parse_line(&line) else {
                    log::error!("[bridge] failed to parse acp line: {line}");
                    let _ = app.emit("zero:stderr", format!("[unparsed] {line}"));
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
                            "tool_call" | "tool_call_update" => {
                                if let Some(path) = history_path.lock().await.clone() {
                                    flush_pending_reasoning(&path, &mut pending_thinking).await;
                                    if let Some(ref e) = event {
                                        append_history(&path, &e.event_type, &e.payload).await;
                                    }
                                } else {
                                    pending_thinking.clear();
                                }
                            }
                            _ => {}
                        }

                        if let Some(event) = event {
                            let _ = app.emit("zero:event", event);
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
                        pending_permissions
                            .lock()
                            .await
                            .insert(correlation_id.clone(), PendingPermission { reply_id: id });
                        let payload = translate_permission_request(&correlation_id, &params);
                        if let Some(path) = history_path.lock().await.clone() {
                            flush_pending_reasoning(&path, &mut pending_thinking).await;
                            append_history(&path, "permission_request", &payload).await;
                        } else {
                            pending_thinking.clear();
                        }
                        let _ = app.emit("zero:permission-request", payload);
                    }
                }
            }
            let _ = app.emit("zero:process-exited", ());
        });
    }

    async fn kill_live(live: &mut LiveProcess) {
        live.child.kill().await.ok();
        let _ = live.child.wait().await;
    }

    pub async fn start(&self, cwd: PathBuf, resume_id: Option<String>) -> Result<(), String> {
        {
            let mut session = self.session.lock().await;
            if let Some(mut old) = session.take() {
                if let Some(mut live) = old.live.take() {
                    Self::kill_live(&mut live).await;
                }
            }
        }

        let known_history_path = match resume_id.as_deref() {
            Some(id) => Some(history_path_for(id)?),
            None => None,
        };
        let (child, peer, session_id) = self
            .spawn_and_handshake(&cwd, resume_id.as_deref(), known_history_path)
            .await?;
        let history_path = history_path_for(&session_id)?;

        let mut session = self.session.lock().await;
        *session = Some(AcpSession {
            cwd,
            session_id,
            history_path,
            live: Some(LiveProcess { child, peer }),
        });

        Ok(())
    }

    /// Ensure the current session has a live process, respawning (and
    /// `session/load`-ing) if it was killed by `cancel()` or died on its
    /// own. Returns the peer, session id, and history path to use for the
    /// next request.
    async fn ensure_live(&self) -> Result<(AcpPeer, String, PathBuf), String> {
        let (cwd, session_id, history_path, needs_respawn) = {
            let session = self.session.lock().await;
            let s = session
                .as_ref()
                .ok_or_else(|| "No active zero session".to_string())?;
            (
                s.cwd.clone(),
                s.session_id.clone(),
                s.history_path.clone(),
                s.live.is_none(),
            )
        };

        if needs_respawn {
            let (child, peer, resumed_id) = self
                .spawn_and_handshake(&cwd, Some(&session_id), Some(history_path))
                .await?;
            let mut session = self.session.lock().await;
            if let Some(ref mut s) = *session {
                s.session_id = resumed_id;
                s.live = Some(LiveProcess { child, peer });
            }
        }

        let session = self.session.lock().await;
        let s = session
            .as_ref()
            .ok_or_else(|| "No active zero session".to_string())?;
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
    pub async fn send(&self, content: String, file: Option<FileAttachment>) -> Result<(), String> {
        let (peer, session_id, history_path) = self.ensure_live().await?;

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

        let app = self.app.clone();
        tokio::spawn(async move {
            let result = peer
                .request(
                    "session/prompt",
                    serde_json::json!({
                        "sessionId": session_id,
                        "prompt": build_prompt_blocks(&content, file.as_ref()),
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
            let _ = app.emit("zero:event", event);
        });

        Ok(())
    }

    /// Answer a pending `session/request_permission` request from the
    /// agent. This is the actual payoff of the ACP migration: unlike the old
    /// `zero exec` transport, this reply really reaches the agent.
    pub async fn respond_to_permission(&self, request_id: String, option_id: String) -> Result<(), String> {
        let pending = self
            .pending_permissions
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| format!("No pending permission request with id {request_id}"))?;

        let (peer, _, _) = self.ensure_live().await?;
        peer.respond(
            pending.reply_id,
            serde_json::json!({ "outcome": { "outcome": "selected", "optionId": option_id } }),
        )
        .await
    }

    /// Cancel the in-flight turn. `zero` has no `session/cancel` method, so
    /// this kills the process outright; the session record (cwd/session_id)
    /// is kept so the next `send()` respawns and `session/load`s back in.
    pub async fn cancel(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(ref mut s) = *session {
            if let Some(mut live) = s.live.take() {
                Self::kill_live(&mut live).await;
            }
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(mut s) = session.take() {
            if let Some(mut live) = s.live.take() {
                Self::kill_live(&mut live).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_from_call_id_strips_counter() {
        assert_eq!(tool_name_from_call_id("read_file_0"), "read_file");
        assert_eq!(tool_name_from_call_id("list_directory_12"), "list_directory");
        assert_eq!(tool_name_from_call_id("mcp_firecrawl_scrape_3"), "mcp_firecrawl_scrape");
    }

    #[test]
    fn test_tool_name_from_call_id_no_counter_suffix() {
        assert_eq!(tool_name_from_call_id("request_permissions"), "request_permissions");
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
    fn test_translate_unknown_update_kind_ignored() {
        let params = serde_json::json!({ "update": { "sessionUpdate": "plan" } });
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
}
