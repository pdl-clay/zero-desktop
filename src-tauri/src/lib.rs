pub mod acp;
pub mod bridge;
pub mod locator;
pub mod mcp_cache;
pub mod terminal;

use bridge::{history_path_for, LiveSessionInfo, StartedSession, ZeroBridge};
use bridge::{get_session_title, remove_session_title, set_session_title};
use bridge::{get_session_model, remove_session_model};
use base64::Engine;
use locator::locate_zero;
use serde::Deserialize;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Manager;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct McpBackendInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub backend_type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(rename = "argCount", default)]
    pub arg_count: i64,
    #[serde(rename = "envKeyCount", default)]
    pub env_key_count: i64,
    #[serde(rename = "headerCount", default)]
    pub header_count: i64,
    #[serde(rename = "toolCount", default)]
    pub tool_count: i64,
    #[serde(rename = "allowGranted", default)]
    pub allow_granted: i64,
    #[serde(rename = "denyGranted", default)]
    pub deny_granted: i64,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct McpCheckResult {
    #[serde(rename = "serverName")]
    pub server_name: String,
    pub status: String,
    #[serde(rename = "toolCount", default)]
    pub tool_count: i64,
    #[serde(default)]
    pub tools: Vec<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "fromCache", default)]
    pub from_cache: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct McpToolsOutput {
    #[serde(default)]
    tools: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct BackendsOutput {
    #[serde(rename = "mcpServers", default)]
    mcp_servers: Vec<McpBackendInfo>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct SessionInfo {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    pub title: String,
    #[serde(alias = "createdAt")]
    pub created_at: String,
    pub cwd: String,
    #[serde(alias = "modelId")]
    pub model_id: String,
    #[serde(alias = "eventCount")]
    pub event_count: Option<i64>,
    pub kind: String,
    pub provider: String,
}

/// A raw persisted session event, as stored in `events.jsonl`. Kept
/// deliberately generic (payload is untyped JSON) because the persisted
/// envelope uses different field names per event type (e.g. `arguments` as a
/// JSON string vs. the live stream's `args` object, `toolCallId` vs. `id`) -
/// the frontend normalizes these the same way it normalizes live stream
/// events, so both paths stay in one place instead of duplicating parsing
/// logic on the Rust side too.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// A single file attached to a user message. Images are sent to the agent as
/// ACP image content blocks (`{"type":"image","mimeType":...,"data":...}`);
/// text/code files are sent as text content blocks so the agent can read them.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    pub mime_type: String,
    /// Base64-encoded file bytes, no `data:` prefix. Text files are also
    /// included as base64 for a consistent wire shape, but decoded to UTF-8
    /// when building ACP prompt blocks.
    pub data: String,
    pub name: String,
}

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachmentKind {
    Image,
    Text,
}

fn attachment_kind_from_extension(path: &Path) -> Result<(AttachmentKind, String), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        // Images
        "png" => Ok((AttachmentKind::Image, "image/png".to_string())),
        "jpg" | "jpeg" => Ok((AttachmentKind::Image, "image/jpeg".to_string())),
        "gif" => Ok((AttachmentKind::Image, "image/gif".to_string())),
        "webp" => Ok((AttachmentKind::Image, "image/webp".to_string())),
        // Plain text / documents
        "txt" => Ok((AttachmentKind::Text, "text/plain".to_string())),
        "md" => Ok((AttachmentKind::Text, "text/markdown".to_string())),
        "csv" => Ok((AttachmentKind::Text, "text/csv".to_string())),
        "json" => Ok((AttachmentKind::Text, "application/json".to_string())),
        "yaml" | "yml" => Ok((AttachmentKind::Text, "application/yaml".to_string())),
        "xml" => Ok((AttachmentKind::Text, "application/xml".to_string())),
        "html" | "htm" => Ok((AttachmentKind::Text, "text/html".to_string())),
        "css" => Ok((AttachmentKind::Text, "text/css".to_string())),
        "js" => Ok((AttachmentKind::Text, "text/javascript".to_string())),
        "ts" => Ok((AttachmentKind::Text, "text/typescript".to_string())),
        "jsx" => Ok((AttachmentKind::Text, "text/jsx".to_string())),
        "tsx" => Ok((AttachmentKind::Text, "text/tsx".to_string())),
        "py" => Ok((AttachmentKind::Text, "text/x-python".to_string())),
        "go" => Ok((AttachmentKind::Text, "text/x-go".to_string())),
        "rs" => Ok((AttachmentKind::Text, "text/x-rust".to_string())),
        "java" => Ok((AttachmentKind::Text, "text/x-java".to_string())),
        "kt" => Ok((AttachmentKind::Text, "text/x-kotlin".to_string())),
        "swift" => Ok((AttachmentKind::Text, "text/x-swift".to_string())),
        "c" => Ok((AttachmentKind::Text, "text/x-c".to_string())),
        "cpp" | "cc" | "cxx" | "h" | "hpp" => Ok((AttachmentKind::Text, "text/x-c++".to_string())),
        "rb" => Ok((AttachmentKind::Text, "text/x-ruby".to_string())),
        "php" => Ok((AttachmentKind::Text, "text/x-php".to_string())),
        "sh" => Ok((AttachmentKind::Text, "text/x-shellscript".to_string())),
        "sql" => Ok((AttachmentKind::Text, "text/x-sql".to_string())),
        "dockerfile" => Ok((AttachmentKind::Text, "text/x-dockerfile".to_string())),
        other => Err(format!("Unsupported file type: .{other}")),
    }
}

/// Decides how to attach a file given its path and already-read bytes.
/// Tries the curated extension map first (keeps exact mime types like
/// `application/json` for recognized types); falls back to treating any
/// extensionless or unrecognized-extension file as plain text as long as it
/// actually looks like text (valid UTF-8, no null byte) - this is what makes
/// the file explorer usable for the many real project files a fixed
/// extension list will never cover (`Dockerfile`, `Makefile`, `.gitignore`,
/// `.env`, lockfiles: `Path::extension()` returns `None` for any of these
/// since they have no `.ext` suffix, so the extension map alone always
/// rejected them). Only genuinely binary-looking content is still rejected.
fn attachment_kind_for_file(path: &Path, bytes: &[u8]) -> Result<(AttachmentKind, String), String> {
    if let Ok(result) = attachment_kind_from_extension(path) {
        if result.0 == AttachmentKind::Text && bytes.contains(&0) {
            return Err("File contains binary data and cannot be attached as text.".to_string());
        }
        return Ok(result);
    }

    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        return Err(format!("Unsupported file type: .{ext}"));
    }
    Ok((AttachmentKind::Text, "text/plain".to_string()))
}

/// Reads a file picked from the native file dialog (or the file explorer
/// tree) and returns it base64-encoded, ready to preview or attach to a
/// message. The dialog/tree only ever give the frontend a path, not bytes -
/// no `fs` plugin/capability is installed, so this plain command reads the
/// file with the same unrestricted `tokio::fs` access
/// `list_zero_sessions`/`delete_session` already use, rather than adding a
/// new plugin dependency.
#[tauri::command]
async fn read_file_attachment(path: String) -> Result<FileAttachment, String> {
    let path_buf = PathBuf::from(&path);

    let metadata = tokio::fs::metadata(&path_buf)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "File is too large ({:.1} MB). Max size is 10 MB.",
            metadata.len() as f64 / (1024.0 * 1024.0)
        ));
    }

    let bytes = tokio::fs::read(&path_buf)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let (_kind, mime_type) = attachment_kind_for_file(&path_buf, &bytes)?;
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let name = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    Ok(FileAttachment { mime_type, data, name })
}

/// One entry in a directory listing, for the file explorer tree.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// Lists one directory level (not recursive - the file explorer tree loads
/// children lazily as folders are expanded, via Quasar QTree's own
/// `lazy`/`@lazy-load`). Folders first, then files, each group sorted
/// alphabetically case-insensitively - matches VS Code's default explorer
/// sort. No `.gitignore`/dotfile filtering in this first pass.
#[tauri::command]
async fn list_directory_entries(path: String) -> Result<Vec<DirEntryInfo>, String> {
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut entries = Vec::new();
    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e| format!("Failed to read directory entry: {e}"))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| format!("Failed to read entry type: {e}"))?;
        entries.push(DirEntryInfo {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: file_type.is_dir(),
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

const RELEVANT_HISTORY_EVENT_TYPES: &[&str] = &[
    "message",
    "reasoning",
    "tool_call",
    "tool_result",
    "permission_request",
    "permission_decision",
    "error",
];

fn parse_events_jsonl(path: &PathBuf) -> Result<Vec<SessionEvent>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open session events: {e}"))?;
    let reader = std::io::BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {e}"))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
            let event_type = value["type"].as_str().unwrap_or("").to_string();
            if !RELEVANT_HISTORY_EVENT_TYPES.contains(&event_type.as_str()) {
                continue;
            }
            events.push(SessionEvent {
                event_type,
                payload: value["payload"].clone(),
                created_at: value["createdAt"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(events)
}

/// zero-desktop's own rich per-session log (see `bridge::history_path_for`)
/// captures tool calls/reasoning/permission decisions that zero itself
/// doesn't persist in ACP mode (verified live - `events.jsonl` there only
/// has `message` entries). Prefer it when present; fall back to zero's own
/// `events.jsonl` for sessions created before this existed, or created
/// outside zero-desktop entirely.
#[tauri::command]
fn load_session_history(session_id: String) -> Result<Vec<SessionEvent>, String> {
    if let Ok(local_path) = history_path_for(&session_id) {
        if local_path.exists() {
            return parse_events_jsonl(&local_path);
        }
    }

    let zero_events_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zero")
        .join("sessions")
        .join(&session_id)
        .join("events.jsonl");

    parse_events_jsonl(&zero_events_path)
}

#[tauri::command]
fn rename_session(session_id: String, title: String) -> Result<(), String> {
    set_session_title(&session_id, &title)
}

#[tauri::command]
async fn delete_session(session_id: String) -> Result<(), String> {
    if let Ok(local_path) = history_path_for(&session_id) {
        let _ = tokio::fs::remove_file(&local_path).await;
    }
    let _ = remove_session_title(&session_id);
    let _ = remove_session_model(&session_id);

    let session_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zero")
        .join("sessions")
        .join(&session_id);

    if !session_dir.exists() {
        return Ok(());
    }

    tokio::fs::remove_dir_all(&session_dir)
        .await
        .map_err(|e| format!("Failed to delete session {}: {e}", session_id))
}

#[tauri::command]
async fn list_zero_sessions(cwd: PathBuf) -> Result<Vec<SessionInfo>, String> {
    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;

    let output = tokio::process::Command::new(&zero_path)
        .arg("sessions")
        .arg("list")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero sessions list: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero sessions list failed: {stderr}"));
    }

    let all_sessions: Vec<SessionInfo> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse sessions JSON: {e}"))?;

    let cwd_str = cwd.to_string_lossy().to_string();
    let mut filtered: Vec<SessionInfo> = all_sessions
        .into_iter()
        .filter(|s| s.cwd == cwd_str)
        .collect();

    // Overlay zero-desktop's own titles (auto-derived from the first
    // message, or set by the user) - zero's own title for ACP-created
    // sessions is just the generic "ACP session".
    for session in &mut filtered {
        if let Some(title) = get_session_title(&session.session_id) {
            session.title = title;
        }
        if session.model_id.is_empty() {
            if let Some(model_id) = get_session_model(&session.session_id) {
                session.model_id = model_id;
            }
        }
    }

    Ok(filtered)
}

#[tauri::command]
fn locate_zero_cli() -> Result<locator::ZeroLocation, String> {
    locator::locate_zero().map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_mcp_backends() -> Result<Vec<McpBackendInfo>, String> {
    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;

    let output = tokio::process::Command::new(&zero_path)
        .arg("backends")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero backends: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero backends failed: {stderr}"));
    }

    let mut backends: BackendsOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse backends JSON: {e}"))?;

    // Overlay cached statuses so the first drawer open already shows data.
    let cached = mcp_cache::all();
    for backend in &mut backends.mcp_servers {
        if let Some(entry) = cached.get(&backend.name) {
            backend.status = Some(entry.status.clone());
            if entry.error.is_some() {
                backend.error = entry.error.clone();
            }
            if entry.tool_count > 0 && backend.tool_count == 0 {
                backend.tool_count = entry.tool_count;
            }
        }
    }

    Ok(backends.mcp_servers)
}

#[tauri::command]
async fn check_mcp_backend(name: String) -> Result<McpCheckResult, String> {
    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;

    let output = tokio::process::Command::new(&zero_path)
        .arg("mcp")
        .arg("check")
        .arg(&name)
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero mcp check: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero mcp check failed: {stderr}"));
    }

    let result: McpCheckResult = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse mcp check JSON: {e}"))?;

    mcp_cache::set_status(
        &result.server_name,
        &result.status,
        result.tool_count,
        result.error.clone(),
    );

    Ok(result)
}

/// Return the current MCP status cache contents for fast initial rendering.
#[tauri::command]
fn load_mcp_status_cache() -> Result<mcp_cache::McpStatusCache, String> {
    Ok(mcp_cache::load())
}

#[tauri::command]
async fn check_mcp_backend_cached(name: String) -> Result<McpCheckResult, String> {
    if let Some(entry) = mcp_cache::get(&name) {
        Ok(McpCheckResult {
            server_name: name,
            status: entry.status,
            tool_count: entry.tool_count,
            tools: Vec::new(),
            error: entry.error,
            from_cache: true,
        })
    } else {
        check_mcp_backend(name).await
    }
}

#[tauri::command]
async fn list_mcp_tools() -> Result<Vec<McpToolInfo>, String> {
    let zero_path = locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path;

    let output = tokio::process::Command::new(&zero_path)
        .arg("mcp")
        .arg("tools")
        .arg("list")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero mcp tools list: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero mcp tools list failed: {stderr}"));
    }

    let parsed: McpToolsOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse mcp tools list JSON: {e}"))?;

    let tools = parsed
        .tools
        .into_iter()
        .map(|value| McpToolInfo {
            name: value["name"].as_str().unwrap_or("").to_string(),
            description: value["description"].as_str().map(String::from),
        })
        .filter(|tool| !tool.name.is_empty())
        .collect();

    Ok(tools)
}

#[tauri::command]
async fn start_zero_session(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    key: String,
    cwd: PathBuf,
    session_id: Option<String>,
) -> Result<StartedSession, String> {
    state.start(key, cwd, session_id).await
}

#[tauri::command]
async fn send_zero_message(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    key: String,
    content: String,
    file: Option<FileAttachment>,
) -> Result<(), String> {
    state.send(key, content, file).await
}

#[tauri::command]
async fn stop_zero_session(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    key: String,
) -> Result<(), String> {
    state.stop(key).await
}

#[tauri::command]
async fn cancel_zero_run(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    key: String,
) -> Result<(), String> {
    state.cancel(key).await
}

#[tauri::command]
async fn list_zero_models() -> Result<bridge::AvailableModels, String> {
    bridge::fetch_available_models().await
}

#[tauri::command]
async fn switch_zero_model(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    key: String,
    model: String,
) -> Result<(), String> {
    bridge::switch_active_model(&model).await?;
    state.cancel(key).await
}

#[tauri::command]
async fn list_live_sessions(
    state: tauri::State<'_, Arc<ZeroBridge>>,
) -> Result<Vec<LiveSessionInfo>, String> {
    Ok(state.list_live_sessions().await)
}

#[tauri::command]
async fn respond_to_permission(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    request_id: String,
    option_id: String,
) -> Result<(), String> {
    state.respond_to_permission(request_id, option_id).await
}

#[tauri::command]
async fn spawn_terminal(
    state: tauri::State<'_, Arc<terminal::TerminalManager>>,
    key: String,
    cwd: String,
    cols: u16,
    rows: u16,
) -> Result<terminal::TerminalSpawnInfo, String> {
    state.spawn(key, cwd, cols, rows).await
}

#[tauri::command]
async fn write_terminal(
    state: tauri::State<'_, Arc<terminal::TerminalManager>>,
    key: String,
    data: String,
) -> Result<(), String> {
    state.write(key, data).await
}

#[tauri::command]
async fn resize_terminal(
    state: tauri::State<'_, Arc<terminal::TerminalManager>>,
    key: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize(key, cols, rows).await
}

#[tauri::command]
async fn kill_terminal(
    state: tauri::State<'_, Arc<terminal::TerminalManager>>,
    key: String,
) -> Result<(), String> {
    state.kill(key).await
}

#[tauri::command]
async fn list_terminals(
    state: tauri::State<'_, Arc<terminal::TerminalManager>>,
) -> Result<Vec<terminal::LiveTerminalInfo>, String> {
    Ok(state.list().await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let bridge = Arc::new(ZeroBridge::new(app.handle().clone()));
            app.manage(bridge);

            let terminal_manager = Arc::new(terminal::TerminalManager::new(app.handle().clone()));
            app.manage(terminal_manager);

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            locate_zero_cli,
            start_zero_session,
            send_zero_message,
            respond_to_permission,
            stop_zero_session,
            cancel_zero_run,
            list_zero_sessions,
            load_session_history,
            delete_session,
            rename_session,
            read_file_attachment,
            list_directory_entries,
            list_zero_models,
            switch_zero_model,
            list_live_sessions,
            list_mcp_backends,
            check_mcp_backend,
            check_mcp_backend_cached,
            load_mcp_status_cache,
            list_mcp_tools,
            spawn_terminal,
            write_terminal,
            resize_terminal,
            kill_terminal,
            list_terminals
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = _app_handle.state::<Arc<ZeroBridge>>();
                tauri::async_runtime::block_on(state.kill_all());
                let terminal_state = _app_handle.state::<Arc<terminal::TerminalManager>>();
                tauri::async_runtime::block_on(terminal_state.kill_all());
            }
        });
}

#[cfg(test)]
mod attachment_tests {
    use super::*;

    #[test]
    fn known_extension_keeps_exact_mime_type() {
        let (kind, mime) =
            attachment_kind_for_file(Path::new("data.json"), b"{}").expect("should be accepted");
        assert_eq!(kind, AttachmentKind::Text);
        assert_eq!(mime, "application/json");
    }

    #[test]
    fn unknown_extension_falls_back_to_text_plain() {
        let (kind, mime) = attachment_kind_for_file(Path::new("Dockerfile"), b"FROM rust:1\n")
            .expect("text-looking extensionless file should be accepted");
        assert_eq!(kind, AttachmentKind::Text);
        assert_eq!(mime, "text/plain");
    }

    #[test]
    fn unknown_extension_with_binary_bytes_is_rejected() {
        let result = attachment_kind_for_file(Path::new("data.bin"), &[0xFF, 0xFE, 0x00, 0x01]);
        assert!(result.is_err());
    }

    #[test]
    fn known_text_extension_with_null_byte_is_rejected() {
        let result = attachment_kind_for_file(Path::new("notes.txt"), &[0x00, b'a']);
        assert!(result.is_err());
    }

    #[test]
    fn known_image_extension_is_not_sniffed_as_text() {
        let (kind, mime) = attachment_kind_for_file(Path::new("photo.png"), &[0x89, b'P', b'N', b'G'])
            .expect("known image extension should be accepted regardless of content");
        assert_eq!(kind, AttachmentKind::Image);
        assert_eq!(mime, "image/png");
    }
}
