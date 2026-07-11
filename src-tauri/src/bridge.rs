use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

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
    PermissionDecision {
        #[serde(rename = "schemaVersion")]
        schema_version: i32,
        #[serde(rename = "permissionId")]
        permission_id: String,
        decision: String,
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
    /// Sender for forwarding permission decisions to the stdin writer task.
    /// Dropping this closes the channel, which causes the writer to finish
    /// and drop stdin, signalling end-of-input to zero.
    permission_tx: Option<mpsc::Sender<InputEvent>>,
}

/// Bridge that manages the zero CLI child process and forwards events.
///
/// Each call to `send()` spawns a fresh `zero exec` process with a persistent
/// stdin writer task. This task writes the initial user message and then
/// listens on an mpsc channel for permission decisions, forwarding them to
/// zero's stdin. When the channel sender is dropped (on next `send()` or
/// `stop()`), the writer task finishes and drops stdin.
///
/// The first turn spawns a plain `zero exec`; subsequent turns use
/// `--resume <sessionId>` so that zero picks up the persisted session and
/// retains conversation context.
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

    pub async fn start(&self, cwd: PathBuf, resume_id: Option<String>) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(ref mut old) = *session {
            old.permission_tx.take();
            if let Some(mut child) = old.child.take() {
                child.kill().await.ok();
                let _ = child.wait().await;
            }
        }
        let sid = Arc::new(Mutex::new(resume_id));
        *session = Some(SessionState {
            cwd,
            session_id: sid,
            child: None,
            permission_tx: None,
        });
        Ok(())
    }

    /// Send a user message to zero.
    ///
    /// Spawns a fresh `zero exec` process. Keeps stdin open via a background
    /// writer task so that permission decisions can be forwarded mid-turn.
    pub async fn send(&self, event: InputEvent) -> Result<(), String> {
        let (cwd, session_id_arc, old_child, old_tx) = {
            let mut session = self.session.lock().await;
            let s = session
                .as_mut()
                .ok_or_else(|| "No active zero session".to_string())?;
            let old = s.child.take();
            let tx = s.permission_tx.take();
            (s.cwd.clone(), s.session_id.clone(), old, tx)
        };

        drop(old_tx);

        if let Some(mut child) = old_child {
            child.kill().await.ok();
            let _ = child.wait().await;
        }

        let zero_path = locate_zero()
            .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
            .path;
        log::info!("[bridge] spawning zero exec at {zero_path:?}, cwd={cwd:?}");

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
            // zero exec is non-interactive: it can never deliver an actual
            // permission_request back to the user (stdin is closed before
            // the turn starts - see the comment below), so at the default
            // "low" autonomy it silently auto-denies shell/write actions,
            // failing tool calls with "Sandbox approval required" instead of
            // asking. Verified directly against the CLI: "medium" is the
            // level where sandboxed shell commands get auto-allowed instead
            // of denied ("auto-allowed: sandbox is active for this shell
            // command"), without going as far as "high", which the CLI's
            // own help text says "enables unsafe tools". Network access
            // (web_fetch/curl) still gets denied even at "high" - that one
            // is a hard limit of this transport, not something a flag fixes.
            .arg("--auto")
            .arg("medium")
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

        let initial_line = serde_json::to_string(&event).map_err(|e| e.to_string())?;
        stdin
            .write_all(initial_line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;
        // `zero exec --input-format stream-json` reads stdin to EOF before it starts
        // acting on any of it - confirmed by testing directly: with stdin held open,
        // it produces no stdout/stderr/network activity at all, no matter how long you
        // wait. It only begins the turn once stdin is closed. So we must close it here
        // rather than keep it open for later writes (e.g. permission decisions - see
        // send_permission_decision, which is consequently a no-op for now).
        stdin.shutdown().await.map_err(|e| e.to_string())?;
        drop(stdin);

        let app = self.app.clone();
        let sid = session_id_arc.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<OutputEvent>(&line) {
                    Ok(event) => {
                        if event.event_type == "run_start" {
                            if let Some(id) = event.payload["sessionId"].as_str() {
                                let mut lock = sid.lock().await;
                                *lock = Some(id.to_string());
                            }
                        }
                        let _ = app.emit("zero:event", event);
                    }
                    Err(e) => {
                        log::error!("[bridge] failed to parse stdout line: {e}: {line}");
                        let _ = app.emit("zero:stderr", format!("[unparsed] {line}"));
                    }
                }
            }
            let _ = app.emit("zero:process-exited", ());
        });

        let app = self.app.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit("zero:stderr", line);
            }
        });

        {
            let mut session = self.session.lock().await;
            if let Some(ref mut s) = *session {
                s.permission_tx = None;
                s.child = Some(child);
            }
        }

        Ok(())
    }

    /// Forward a permission decision back to zero.
    ///
    /// Not currently supported: `zero exec` reads stdin to EOF before acting on
    /// anything, and we close stdin right after the initial message so the turn
    /// actually runs (see `send()`). There is no channel left to deliver a
    /// mid-run decision through. Approving/denying in the UI updates local
    /// state but zero never sees the decision.
    pub async fn send_permission_decision(
        &self,
        _permission_id: String,
        _decision: String,
    ) -> Result<(), String> {
        Err("Permission decisions are not supported: zero exec closes stdin after \
             the initial message, so there is no way to deliver a decision mid-run."
            .to_string())
    }

    /// Cancel the in-flight turn without tearing down the session: kills the
    /// child process (if any) but keeps `cwd`/`session_id` intact so the next
    /// `send()` can still `--resume` the same zero session. Unlike `stop()`,
    /// which is used when switching workspaces/sessions entirely.
    pub async fn cancel(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(ref mut s) = *session {
            drop(s.permission_tx.take());
            if let Some(mut child) = s.child.take() {
                child.kill().await.ok();
                let _ = child.wait().await;
            }
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if let Some(mut s) = session.take() {
            drop(s.permission_tx.take());
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
    fn test_input_event_permission_decision_serialization() {
        let event = InputEvent::PermissionDecision {
            schema_version: 2,
            permission_id: "p-1".to_string(),
            decision: "approved".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schemaVersion"], 2);
        assert_eq!(parsed["type"], "permission_decision");
        assert_eq!(parsed["permissionId"], "p-1");
        assert_eq!(parsed["decision"], "approved");
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
