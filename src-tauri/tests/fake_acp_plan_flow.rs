//! Drives a fake `zero acp` backend (a small script speaking the same
//! newline-delimited JSON-RPC protocol as the real `zero acp`, but scripted
//! instead of calling an LLM) through the real `app_lib::acp` transport, to
//! reproduce - without spending real agent turns - the exact sequence of
//! `session/update` notifications a live plan run produces. Used to
//! investigate why the plan checklist wasn't showing up in the frontend.
//!
//! Mirrors the shape of `bridge.rs`'s own `spawn_stdout_reader`: a single
//! background task owns stdout and feeds `Response`s back into the `AcpPeer`
//! while forwarding `Notification`s to the test through a channel. Waiting
//! on a `peer.request()` future directly against a `next_line()` future in
//! the same `select!`/`join!` deadlocks, since the response can only arrive
//! after a notification (or the response line itself) has been read off the
//! same stream - which is exactly the bug the first draft of this test hit.

use app_lib::acp::{parse_line, AcpMessage, AcpPeer};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;

fn fake_backend_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_zero_acp.py")
}

#[tokio::test]
async fn test_fake_backend_plan_sequence_matches_bridge_contract() {
    let script = fake_backend_path();
    assert!(script.is_file(), "fixture missing at {script:?}");

    let mut child = Command::new("python3")
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn fake acp backend");

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let peer = AcpPeer::new(stdin);

    let (tx, mut notifications) = mpsc::unbounded_channel::<(String, serde_json::Value)>();
    let reader_peer = peer.clone();
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match parse_line(&line) {
                Some(AcpMessage::Response { id, result }) => {
                    reader_peer.resolve_response(id, result).await;
                }
                Some(AcpMessage::Notification { method, params }) => {
                    let _ = tx.send((method, params));
                }
                _ => {}
            }
        }
    });

    let init = timeout(Duration::from_secs(5), peer.request("initialize", serde_json::json!({})))
        .await
        .expect("initialize timed out")
        .expect("initialize should succeed");
    assert_eq!(init["protocolVersion"], 1);

    let session_result = timeout(Duration::from_secs(5), peer.request("session/new", serde_json::json!({})))
        .await
        .expect("session/new timed out")
        .expect("session/new should succeed");
    let session_id = session_result["sessionId"].as_str().unwrap().to_string();
    assert_eq!(session_id, "fake-session-1");

    // Drive the turn and drain notifications concurrently - the fake
    // backend emits several `session/update`s before it answers
    // `session/prompt`, exactly like the real `zero acp` does for a turn
    // that updates its plan mid-flight.
    let prompt = peer.request("session/prompt", serde_json::json!({ "sessionId": session_id }));
    tokio::pin!(prompt);

    let mut plan_snapshots: Vec<serde_json::Value> = Vec::new();
    let mut saw_tool_call = false;
    let mut saw_tool_call_update = false;

    let mut record_update = |method: &str, params: &serde_json::Value| {
        if method != "session/update" {
            return;
        }
        let kind = params["update"]["sessionUpdate"].as_str().unwrap_or("");
        match kind {
            "plan" => plan_snapshots.push(params["update"]["entries"].clone()),
            "tool_call" => saw_tool_call = true,
            "tool_call_update" => saw_tool_call_update = true,
            _ => {}
        }
    };

    let prompt_result = loop {
        tokio::select! {
            result = &mut prompt => break result,
            Some((method, params)) = notifications.recv() => record_update(&method, &params),
        }
    };

    // The reader task pushes notifications to the channel strictly in the
    // order it reads them off stdout, then resolves the `session/prompt`
    // response last - but `select!` polls both branches every iteration and
    // may pick the now-ready response branch on some iteration even while
    // earlier notifications are still sitting unread in the channel buffer.
    // Drain whatever arrived alongside the final response before asserting.
    while let Ok((method, params)) = notifications.try_recv() {
        record_update(&method, &params);
    }

    let final_result = timeout(Duration::from_secs(5), async { prompt_result })
        .await
        .expect("session/prompt timed out")
        .expect("session/prompt should succeed");
    assert_eq!(final_result["stopReason"], "end_turn");

    // The exact contract bridge.rs's `translate_session_update` relies on:
    // a `plan` update carries a full-replacement `entries` array of
    // {content, status, priority} objects (see bridge.rs's
    // `test_translate_plan_update`), and a real turn interleaves plan
    // updates with tool calls rather than sending the plan only once.
    assert_eq!(plan_snapshots.len(), 3, "expected pending -> in_progress -> completed snapshots");
    assert_eq!(plan_snapshots[0][0]["status"], "pending");
    assert_eq!(plan_snapshots[1][0]["status"], "in_progress");
    assert_eq!(plan_snapshots[2][0]["status"], "completed");
    assert_eq!(plan_snapshots[2][1]["status"], "completed");
    for entry in plan_snapshots[0].as_array().unwrap() {
        assert!(entry.get("content").is_some(), "entry must have `content` for PlanPanel.vue");
        assert!(entry.get("status").is_some(), "entry must have `status` for planIcon/planColor");
    }
    assert!(saw_tool_call, "expected a tool_call between plan updates");
    assert!(saw_tool_call_update, "expected the tool_call to resolve");

    reader.abort();
    let _ = child.kill().await;
}

/// `switch_session_model` (`bridge.rs`) relies on `_zero/set_model` being a
/// real request/response method - fast regression check for the request
/// shape against the fake backend, complementing the live confirmation in
/// `acp::tests::test_live_cancel_and_set_model_avoid_process_kill`.
#[tokio::test]
async fn test_fake_backend_zero_set_model_roundtrip() {
    let script = fake_backend_path();
    let mut child = Command::new("python3")
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn fake acp backend");

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let peer = AcpPeer::new(stdin);
    let reader_peer = peer.clone();

    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(AcpMessage::Response { id, result }) = parse_line(&line) {
                reader_peer.resolve_response(id, result).await;
            }
        }
    });

    let result = timeout(
        Duration::from_secs(5),
        peer.request(
            "_zero/set_model",
            serde_json::json!({"sessionId": "fake-session-1", "model": "big-model-x"}),
        ),
    )
    .await
    .expect("_zero/set_model timed out")
    .expect("_zero/set_model should succeed");
    assert_eq!(result["model"], "big-model-x");

    reader.abort();
    let _ = child.kill().await;
}
