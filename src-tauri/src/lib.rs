pub mod bridge;
pub mod locator;

use bridge::{InputEvent, ZeroBridge};
use locator::locate_zero;
use serde::Deserialize;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

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

const RELEVANT_HISTORY_EVENT_TYPES: &[&str] = &[
    "message",
    "reasoning",
    "tool_call",
    "tool_result",
    "permission_request",
    "permission_decision",
    "error",
];

#[tauri::command]
fn load_session_history(session_id: String) -> Result<Vec<SessionEvent>, String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zero")
        .join("sessions")
        .join(&session_id)
        .join("events.jsonl");

    let file = std::fs::File::open(&data_dir)
        .map_err(|e| format!("Failed to open session events: {e}"))?;

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

#[tauri::command]
async fn delete_session(session_id: String) -> Result<(), String> {
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
    let filtered: Vec<SessionInfo> = all_sessions
        .into_iter()
        .filter(|s| s.cwd == cwd_str)
        .collect();

    Ok(filtered)
}

#[tauri::command]
fn locate_zero_cli() -> Result<locator::ZeroLocation, String> {
    locator::locate_zero().map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_zero_session(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    cwd: PathBuf,
    session_id: Option<String>,
) -> Result<(), String> {
    state.start(cwd, session_id).await
}

#[tauri::command]
async fn send_zero_message(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    content: String,
) -> Result<(), String> {
    state.send(InputEvent::user_message(content)).await
}

#[tauri::command]
async fn stop_zero_session(state: tauri::State<'_, Arc<ZeroBridge>>) -> Result<(), String> {
    state.stop().await
}

#[tauri::command]
async fn cancel_zero_run(state: tauri::State<'_, Arc<ZeroBridge>>) -> Result<(), String> {
    state.cancel().await
}

#[tauri::command]
async fn send_permission_decision(
    state: tauri::State<'_, Arc<ZeroBridge>>,
    permission_id: String,
    decision: String,
) -> Result<(), String> {
    state
        .send_permission_decision(permission_id, decision)
        .await
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

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            locate_zero_cli,
            start_zero_session,
            send_zero_message,
            send_permission_decision,
            stop_zero_session,
            cancel_zero_run,
            list_zero_sessions,
            load_session_history,
            delete_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
