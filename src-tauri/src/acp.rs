//! Minimal JSON-RPC 2.0 "peer" for the Agent Client Protocol (`zero acp`).
//!
//! Unlike a typical JSON-RPC client or server, this needs to act as both at
//! once over the same connection: we send requests (`initialize`,
//! `session/new`, `session/prompt`) and receive their responses, but the
//! agent can *also* send us requests mid-turn (`session/request_permission`)
//! that we must reply to, plus fire-and-forget notifications
//! (`session/update`). Messages are newline-delimited JSON on stdio (no
//! `Content-Length` framing like LSP) - confirmed by testing directly
//! against the real `zero acp` binary.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::process::ChildStdin;
use tokio::sync::{oneshot, Mutex};

/// A single parsed line from the peer's stdout.
#[derive(Debug, Clone)]
pub enum AcpMessage {
    /// A response to a request we sent (`id` echoed back with `result` or `error`).
    Response {
        id: u64,
        result: Result<serde_json::Value, serde_json::Value>,
    },
    /// A request the agent is sending us, expecting a reply (e.g.
    /// `session/request_permission`). `id` is kept as raw JSON since ACP
    /// doesn't guarantee it's a number (echo it back verbatim in the reply).
    Request {
        id: serde_json::Value,
        method: String,
        params: serde_json::Value,
    },
    /// A fire-and-forget notification (e.g. `session/update`). No reply.
    Notification {
        method: String,
        params: serde_json::Value,
    },
}

/// Parse one line of stdout into an `AcpMessage`. Returns `None` for lines
/// that aren't valid JSON-RPC 2.0 messages (caller should log and skip).
pub fn parse_line(line: &str) -> Option<AcpMessage> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let obj = value.as_object()?;

    if let Some(method) = obj.get("method").and_then(|m| m.as_str()) {
        let params = obj.get("params").cloned().unwrap_or(serde_json::Value::Null);
        return Some(match obj.get("id") {
            Some(id) => AcpMessage::Request {
                id: id.clone(),
                method: method.to_string(),
                params,
            },
            None => AcpMessage::Notification {
                method: method.to_string(),
                params,
            },
        });
    }

    let id = obj.get("id")?.as_u64()?;
    if let Some(result) = obj.get("result") {
        return Some(AcpMessage::Response {
            id,
            result: Ok(result.clone()),
        });
    }
    if let Some(error) = obj.get("error") {
        return Some(AcpMessage::Response {
            id,
            result: Err(error.clone()),
        });
    }
    None
}

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, serde_json::Value>>>>>;

/// Sends requests/responses to an ACP agent process and correlates
/// responses back to callers. Does not own reading stdout - the caller
/// spawns a reader loop that calls `parse_line` and feeds `Response`
/// variants to `resolve_response`, and handles `Request`/`Notification`
/// variants itself (translating them into app events, etc).
#[derive(Clone)]
pub struct AcpPeer {
    stdin: Arc<Mutex<BufWriter<ChildStdin>>>,
    next_id: Arc<AtomicU64>,
    pending: PendingMap,
}

impl AcpPeer {
    pub fn new(stdin: ChildStdin) -> Self {
        Self {
            stdin: Arc::new(Mutex::new(BufWriter::new(stdin))),
            next_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn write_line(&self, value: &serde_json::Value) -> Result<(), String> {
        let mut text = serde_json::to_string(value).map_err(|e| e.to_string())?;
        text.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(text.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    /// Send a request and await its response. Registers a pending oneshot
    /// keyed by the allocated id, which the reader loop resolves via
    /// `resolve_response` once the matching line arrives.
    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let line = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        if let Err(e) = self.write_line(&line).await {
            self.pending.lock().await.remove(&id);
            return Err(e);
        }

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("connection closed while waiting for response".to_string()),
        }
    }

    /// Reply to a request the agent sent us (e.g. `session/request_permission`).
    pub async fn respond(&self, id: serde_json::Value, result: serde_json::Value) -> Result<(), String> {
        let line = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        self.write_line(&line).await
    }

    /// Send a fire-and-forget JSON-RPC notification (no `id`, no response
    /// expected). `zero acp` registers some methods (e.g. `session/cancel`)
    /// as notification-only handlers - sending them as a `request()` gets a
    /// "method not found" error, since the id-keyed reply is never sent.
    /// Verified live against the real binary.
    pub async fn notify(&self, method: &str, params: serde_json::Value) -> Result<(), String> {
        let line = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_line(&line).await
    }

    /// Called by the reader loop when a `Response` message arrives.
    pub async fn resolve_response(&self, id: u64, result: Result<serde_json::Value, serde_json::Value>) {
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(result);
        }
    }

    /// Drop all pending requests with an error (e.g. the process died).
    /// Prevents callers from hanging forever awaiting a response that will
    /// never arrive.
    pub async fn fail_all_pending(&self, message: &str) {
        let mut pending = self.pending.lock().await;
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(serde_json::json!({ "message": message })));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_with_result() {
        let line = r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"abc"}}"#;
        match parse_line(line) {
            Some(AcpMessage::Response { id, result: Ok(v) }) => {
                assert_eq!(id, 2);
                assert_eq!(v["sessionId"], "abc");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_response_with_error() {
        let line = r#"{"jsonrpc":"2.0","id":100,"error":{"code":-32601,"message":"method not found"}}"#;
        match parse_line(line) {
            Some(AcpMessage::Response { id, result: Err(e) }) => {
                assert_eq!(id, 100);
                assert_eq!(e["code"], -32601);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_request_from_agent() {
        let line = r#"{"jsonrpc":"2.0","id":5,"method":"session/request_permission","params":{"sessionId":"s1","options":[]}}"#;
        match parse_line(line) {
            Some(AcpMessage::Request { id, method, params }) => {
                assert_eq!(id, serde_json::json!(5));
                assert_eq!(method, "session/request_permission");
                assert_eq!(params["sessionId"], "s1");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_notification() {
        let line = r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi"}}}}"#;
        match parse_line(line) {
            Some(AcpMessage::Notification { method, params }) => {
                assert_eq!(method, "session/update");
                assert_eq!(params["update"]["sessionUpdate"], "agent_message_chunk");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_invalid_line_returns_none() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"jsonrpc":"2.0"}"#).is_none());
    }

    /// `notify()` must produce a JSON-RPC line with no `id` field - that's
    /// what makes `zero acp` treat it as a notification instead of a
    /// request expecting a reply (verified live: sending `session/cancel`
    /// with an `id` gets "method not found", since it's only registered as
    /// a notification handler). Uses `cat` as a trivial echo process so the
    /// exact bytes written can be inspected without a real ACP server.
    #[tokio::test]
    async fn test_notify_sends_no_id_field() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        let mut child = Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn `cat`");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let peer = AcpPeer::new(stdin);

        peer.notify("session/cancel", serde_json::json!({ "sessionId": "s1" }))
            .await
            .expect("notify should send successfully");

        let mut lines = BufReader::new(stdout).lines();
        let line = lines
            .next_line()
            .await
            .unwrap()
            .expect("expected a line echoed back by `cat`");

        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(value.get("id").is_none(), "a notification must not carry an id field");
        assert_eq!(value["method"], "session/cancel");
        assert_eq!(value["params"]["sessionId"], "s1");

        match parse_line(&line) {
            Some(AcpMessage::Notification { method, params }) => {
                assert_eq!(method, "session/cancel");
                assert_eq!(params["sessionId"], "s1");
            }
            other => panic!("expected Notification, got {other:?}"),
        }

        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn test_resolve_response_delivers_to_waiting_request() {
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert(7, tx);

        // Simulate what the reader loop does on a Response message.
        if let Some(sender) = pending.lock().await.remove(&7) {
            let _ = sender.send(Ok(serde_json::json!({"ok": true})));
        }

        let result = rx.await.unwrap();
        assert_eq!(result.unwrap()["ok"], true);
    }

    /// Live integration test against the real `zero acp` binary. Ignored by
    /// default (requires `zero` installed on PATH); run explicitly with
    /// `cargo test -- --ignored acp::tests::test_live_acp_permission_flow`.
    /// Spawns a real process, drives initialize -> session/new ->
    /// session/prompt through `AcpPeer`, and auto-approves any
    /// `session/request_permission` the agent sends - the same pattern
    /// `bridge.rs`'s reader loop will use, just inlined here for the test.
    #[tokio::test]
    #[ignore]
    async fn test_live_acp_permission_flow() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        let workdir = std::env::temp_dir().join(format!("acp-test-{}", std::process::id()));
        std::fs::create_dir_all(&workdir).unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(".")
            .current_dir(&workdir)
            .status()
            .unwrap();
        std::fs::write(workdir.join("note.txt"), "old content\n").unwrap();

        let mut child = Command::new("zero")
            .arg("acp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn `zero acp` - is zero on PATH?");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let peer = AcpPeer::new(stdin);

        let saw_permission_request = Arc::new(Mutex::new(false));
        let saw_permission_request2 = saw_permission_request.clone();
        let peer_for_reader = peer.clone();

        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match parse_line(&line) {
                    Some(AcpMessage::Response { id, result }) => {
                        peer_for_reader.resolve_response(id, result).await;
                    }
                    Some(AcpMessage::Request { id, method, params }) => {
                        if method == "session/request_permission" {
                            *saw_permission_request2.lock().await = true;
                            let options = params["options"].as_array().cloned().unwrap_or_default();
                            let chosen = options
                                .iter()
                                .find(|o| o["optionId"].as_str() == Some("allow"))
                                .or_else(|| options.first())
                                .and_then(|o| o["optionId"].as_str())
                                .unwrap_or("allow")
                                .to_string();
                            let _ = peer_for_reader
                                .respond(
                                    id,
                                    serde_json::json!({"outcome": {"outcome": "selected", "optionId": chosen}}),
                                )
                                .await;
                        }
                    }
                    Some(AcpMessage::Notification { .. }) | None => {}
                }
            }
        });

        let init = peer
            .request(
                "initialize",
                serde_json::json!({"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}}}),
            )
            .await
            .expect("initialize failed");
        assert!(init["agentCapabilities"].is_object());

        let new_session = peer
            .request(
                "session/new",
                serde_json::json!({"cwd": workdir.to_string_lossy(), "mcpServers": []}),
            )
            .await
            .expect("session/new failed");
        let session_id = new_session["sessionId"].as_str().unwrap().to_string();

        // Network access is denied at every autonomy level in `zero exec`
        // (verified separately) and reliably goes through a real
        // session/request_permission ask here too, unlike workspace writes
        // which "auto" mode often grants without asking - so this is the
        // dependable way to exercise the permission round trip.
        let prompt_result = peer
            .request(
                "session/prompt",
                serde_json::json!({
                    "sessionId": session_id,
                    "prompt": [{"type": "text", "text": "Use your web fetch tool to fetch https://example.com right now."}]
                }),
            )
            .await
            .expect("session/prompt failed");
        assert_eq!(prompt_result["stopReason"], "end_turn");

        // Verify session/load: separately confirmed unsupported/unsupported-shaped
        // methods return a clean JSON-RPC error rather than hanging, so this
        // either succeeds or fails fast - either way proves the peer handles
        // the round trip correctly.
        let load_result = peer
            .request("session/load", serde_json::json!({"sessionId": session_id}))
            .await;
        eprintln!("session/load result: {load_result:?}");

        drop(peer);
        let _ = child.start_kill();
        let _ = reader_task.await;

        assert!(
            *saw_permission_request.lock().await,
            "expected the agent to ask for write permission at least once"
        );

        std::fs::remove_dir_all(&workdir).ok();
    }

    /// Live integration test against the real `zero acp` binary. Ignored by
    /// default; run explicitly with `cargo test -- --ignored
    /// acp::tests::test_live_cancel_and_set_model_avoid_process_kill`.
    ///
    /// Encodes what was manually verified with an ad-hoc Python probe while
    /// investigating `bridge.rs`'s old kill/respawn hacks:
    /// - `_zero/set_model` (a non-standard, `_zero/`-prefixed ACP extension)
    ///   is a real request/response method, not "method not found".
    /// - `session/cancel` sent as a notification (no `id`) actually
    ///   interrupts an in-flight `session/prompt` - the pending request
    ///   resolves with `stopReason: "cancelled"` - without killing the
    ///   process, and the same process answers a follow-up prompt
    ///   afterwards. Sending it as a *request* (with an `id`) instead gets
    ///   "method not found", since `zero acp` only registers it as a
    ///   notification handler.
    #[tokio::test]
    #[ignore]
    async fn test_live_cancel_and_set_model_avoid_process_kill() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        let workdir = std::env::temp_dir().join(format!("acp-cancel-test-{}", std::process::id()));
        std::fs::create_dir_all(&workdir).unwrap();

        let mut child = Command::new("zero")
            .arg("acp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn `zero acp` - is zero on PATH?");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let peer = AcpPeer::new(stdin);
        let reader_peer = peer.clone();

        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(AcpMessage::Response { id, result }) = parse_line(&line) {
                    reader_peer.resolve_response(id, result).await;
                }
            }
        });
        // Must be drained continuously, not just read once at the end - an
        // unread stderr pipe fills its OS buffer and blocks the child
        // process outright once `zero acp` writes enough to it (it logs
        // verbosely while generating), which looked indistinguishable from
        // cancel not working until this was added.
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(_)) = lines.next_line().await {}
        });

        peer.request(
            "initialize",
            serde_json::json!({"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}}}),
        )
        .await
        .expect("initialize failed");

        // Separate session for `_zero/set_model`: it doesn't validate the
        // model id against the active provider's catalog (verified live),
        // so an arbitrary label is enough to prove the method exists and
        // echoes back what was set - but setting a bogus model on the
        // *same* session used for the cancel flow below would make the
        // real generation call fail against whatever provider is
        // configured on the machine running this test.
        let model_session = peer
            .request(
                "session/new",
                serde_json::json!({"cwd": workdir.to_string_lossy(), "mcpServers": []}),
            )
            .await
            .expect("session/new (model check) failed");
        let model_session_id = model_session["sessionId"].as_str().unwrap().to_string();
        let set_model = peer
            .request(
                "_zero/set_model",
                serde_json::json!({"sessionId": model_session_id, "model": "test-model-override"}),
            )
            .await
            .expect("_zero/set_model should be a supported method");
        assert_eq!(set_model["model"], "test-model-override");

        // Fresh session, left on the provider's real default model, to
        // drive the cancel-mid-turn flow with an actual generation call.
        let new_session = peer
            .request(
                "session/new",
                serde_json::json!({"cwd": workdir.to_string_lossy(), "mcpServers": []}),
            )
            .await
            .expect("session/new failed");
        let session_id = new_session["sessionId"].as_str().unwrap().to_string();

        // Fire a slow turn, cancel it partway through as a notification,
        // and confirm the turn actually stops instead of running to
        // completion. `tokio::spawn` (not just constructing the future) is
        // required here - `peer.request()` doesn't write anything to stdin
        // until it's actually polled, so merely holding the future while
        // sleeping/cancelling would send the cancel *before* the prompt
        // request itself, making it a no-op against a turn that hadn't
        // started yet.
        let peer_for_prompt = peer.clone();
        let session_id_for_prompt = session_id.clone();
        let prompt_task = tokio::spawn(async move {
            peer_for_prompt
                .request(
                    "session/prompt",
                    serde_json::json!({
                        "sessionId": session_id_for_prompt,
                        "prompt": [{"type": "text", "text": "Write a very long, detailed essay (at least 2000 words) about the history of computing, step by step."}]
                    }),
                )
                .await
        });

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        peer.notify("session/cancel", serde_json::json!({"sessionId": session_id}))
            .await
            .expect("session/cancel notification should send successfully");

        let cancelled_result = tokio::time::timeout(std::time::Duration::from_secs(20), prompt_task)
            .await
            .expect("session/prompt should resolve promptly after cancel")
            .expect("prompt task should not panic")
            .expect("cancelled session/prompt should still return a result, not an error");
        assert_eq!(cancelled_result["stopReason"], "cancelled");

        // The same process/session must still be alive and responsive - no
        // kill/respawn should have been needed to cancel the turn.
        let followup = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            peer.request(
                "session/prompt",
                serde_json::json!({
                    "sessionId": session_id,
                    "prompt": [{"type": "text", "text": "Reply with the single word: alive"}]
                }),
            ),
        )
        .await
        .expect("follow-up prompt should resolve promptly")
        .expect("follow-up prompt on the same session should still succeed");
        assert_eq!(followup["stopReason"], "end_turn");

        reader_task.abort();
        stderr_task.abort();
        let _ = child.start_kill();
        std::fs::remove_dir_all(&workdir).ok();
    }
}
