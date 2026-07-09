use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::locator::locate_zero;

/// Input events accepted by zero's stream-json protocol.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum InputEvent {
    Message {
        #[serde(rename = "schemaVersion")]
        schema_version: i32,
        role: String,
        content: String,
    },
}

impl InputEvent {
    pub fn user_message(content: String) -> Self {
        Self::Message {
            schema_version: 2,
            role: "user".to_string(),
            content,
        }
    }
}

/// Output events emitted by zero's stream-json protocol.
/// We only deserialize the fields we care about; unknown fields are ignored.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputEvent {
    #[serde(rename = "schemaVersion")]
    pub schema_version: i32,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// Persistent state for a zero session across turns.
struct SessionState {
    cwd: PathBuf,
    session_id: Arc<Mutex<Option<String>>>,
    child: Option<Child>,
}

/// Bridge that manages the zero CLI child process and forwards events.
///
/// Each call to `send()` spawns a fresh `zero exec` process. The first turn
/// spawns a plain `zero exec`; subsequent turns use `--resume <sessionId>` so
/// that zero picks up the persisted session and retains conversation context.
pub struct ZeroBridge {
    app: tauri::AppHandle,
    session: Arc<Mutex<Option<SessionState>>>,
}

impl ZeroBridge {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// Record the workspace directory for this session.
    ///
    /// If `session_id` is provided, the bridge will use `--resume` on the
    /// first `send()` to continue that session instead of starting a new one.
    ///
    /// The first zero process is spawned lazily on the first `send()`.
    pub async fn start(&self, cwd: PathBuf, resume_id: Option<String>) -> Result<(), String> {
        let mut session = self.session.lock().await;
        let sid = Arc::new(Mutex::new(resume_id));
        *session = Some(SessionState {
            cwd,
            session_id: sid,
            child: None,
        });
        Ok(())
    }

    /// Send an input event to zero.
    ///
    /// Spawns a fresh `zero exec` process for each turn. If a previous turn
    /// captured a `sessionId`, the new process is launched with
    /// `--resume <sessionId>` so zero continues the same conversation.
    pub async fn send(&self, event: InputEvent) -> Result<(), String> {
        let (cwd, session_id_arc, old_child) = {
            let mut session = self.session.lock().await;
            let s = session
                .as_mut()
                .ok_or_else(|| "No active zero session".to_string())?;
            let old = s.child.take();
            (s.cwd.clone(), s.session_id.clone(), old)
        };

        // Kill the previous process outside the lock so we don't hold it
        // across async operations.
        if let Some(mut child) = old_child {
            child.kill().await.ok();
            let _ = child.wait().await;
        }

        let zero_path = locate_zero()
            .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
            .path;

        let resume_id = {
            let id = session_id_arc.lock().await;
            id.clone()
        };

        let mut cmd = Command::new(&zero_path);
        cmd.arg("exec")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--cwd")
            .arg(&cwd);
        if let Some(ref id) = resume_id {
            cmd.arg("--resume").arg(id);
        }

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn zero exec: {e}"))?;

        let mut stdin = child
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

        // Write the message and close stdin so zero reads exactly one turn.
        let line = serde_json::to_string(&event).map_err(|e| e.to_string())?;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;
        drop(stdin);

        // Spawn stdout reader — captures sessionId from run_start events
        // and forwards everything to the frontend.
        let app = self.app.clone();
        let sid = session_id_arc.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(event) = serde_json::from_str::<OutputEvent>(&line) {
                    if event.event_type == "run_start" {
                        if let Some(id) = event.payload["sessionId"].as_str() {
                            let mut lock = sid.lock().await;
                            *lock = Some(id.to_string());
                        }
                    }
                    let _ = app.emit("zero:event", event);
                }
            }
        });

        // Spawn stderr reader — forwards raw lines to the frontend.
        let app = self.app.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit("zero:stderr", line);
            }
        });

        // Store the child so we can kill it on the next send() or on stop().
        {
            let mut session = self.session.lock().await;
            if let Some(ref mut s) = *session {
                s.child = Some(child);
            }
        }

        Ok(())
    }

    /// Stop the current session, killing any running child process.
    pub async fn stop(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(mut s) = session.take() {
            if let Some(ref mut child) = s.child {
                child.kill().await.ok();
                let _ = child.wait().await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_user_message_serialization() {
        let event = InputEvent::user_message("Hello, zero!".to_string());
        let json = serde_json::to_string(&event).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schemaVersion"], 2);
        assert_eq!(parsed["type"], "message");
        assert_eq!(parsed["role"], "user");
        assert_eq!(parsed["content"], "Hello, zero!");
    }

    #[test]
    fn test_output_event_run_start_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"run_start","sessionId":"abc-123","cwd":"/tmp/test","model":"test-model"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.schema_version, 2);
        assert_eq!(event.event_type, "run_start");

        let payload = event.payload.as_object().unwrap();
        assert_eq!(payload["sessionId"], "abc-123");
        assert_eq!(payload["cwd"], "/tmp/test");
        assert_eq!(payload["model"], "test-model");
    }

    #[test]
    fn test_output_event_text_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"text","delta":"Hello"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.schema_version, 2);
        assert_eq!(event.event_type, "text");
        assert_eq!(event.payload["delta"], "Hello");
    }

    #[test]
    fn test_output_event_final_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"final","content":"Hello, world!"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "final");
        assert_eq!(event.payload["content"], "Hello, world!");
    }

    #[test]
    fn test_output_event_run_end_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"run_end","status":"completed"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "run_end");
        assert_eq!(event.payload["status"], "completed");
    }

    #[test]
    fn test_output_event_error_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"error","message":"something went wrong"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "error");
        assert_eq!(event.payload["message"], "something went wrong");
    }

    #[test]
    fn test_output_event_tool_call_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"tool_call","toolName":"read","toolUseId":"tu-1","input":{"filePath":"/tmp/test.txt"}}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "tool_call");
        assert_eq!(event.payload["toolName"], "read");
        assert_eq!(event.payload["toolUseId"], "tu-1");
        assert_eq!(event.payload["input"]["filePath"], "/tmp/test.txt");
    }

    #[test]
    fn test_output_event_tool_result_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"tool_result","toolUseId":"tu-1","content":"file contents here"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "tool_result");
        assert_eq!(event.payload["toolUseId"], "tu-1");
        assert_eq!(event.payload["content"], "file contents here");
    }

    #[test]
    fn test_output_event_permission_request_deserialization() {
        let json = r#"{"schemaVersion":2,"type":"permission_request","permissionId":"p-1","toolName":"bash","proposedCommand":"rm -rf /"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "permission_request");
        assert_eq!(event.payload["permissionId"], "p-1");
        assert_eq!(event.payload["toolName"], "bash");
        assert_eq!(event.payload["proposedCommand"], "rm -rf /");
    }

    #[test]
    fn test_output_event_unknown_fields_ignored() {
        let json = r#"{"schemaVersion":2,"type":"text","delta":"test","extra_field":42,"another_extra":"ignored"}"#;

        let event: OutputEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "text");
        assert_eq!(event.payload["delta"], "test");
    }

    #[test]
    fn test_output_event_serialize_roundtrip() {
        let original = OutputEvent {
            schema_version: 2,
            event_type: "text".to_string(),
            payload: serde_json::json!({"delta": "hello"}),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: OutputEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.schema_version, original.schema_version);
        assert_eq!(parsed.event_type, original.event_type);
        assert_eq!(parsed.payload["delta"], original.payload["delta"]);
    }
}
