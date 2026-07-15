//! Drives a fake `zero acp` backend (a small script speaking the same
//! newline-delimited JSON-RPC protocol as the real `zero acp`, but scripted
//! instead of calling an LLM) through the real `app_lib::acp` transport, to
//! reproduce - without spending real agent turns - the exact sequence of
//! `session/update` notifications a live plan run produces. Used to
//! investigate why the plan checklist wasn't showing up in the frontend.

use app_lib::acp::{parse_line, AcpMessage, AcpPeer};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
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
    let mut lines = BufReader::new(stdout).lines();

    // initialize
    let init = peer.request("initialize", serde_json::json!({}));
    let (init_result, first_line) = tokio::join!(init, async {
        timeout(Duration::from_secs(5), lines.next_line()).await.unwrap().unwrap()
    });
    // Feed the response line back into the peer manually since we're
    // reading stdout ourselves instead of running the real reader loop.
    if let Some(AcpMessage::Response { id, result }) = parse_line(&first_line.unwrap()) {
        peer.resolve_response(id, result).await;
    }
    init_result.expect("initialize should succeed");

    // session/new
    let new_session = peer.request("session/new", serde_json::json!({}));
    let (session_result, line) = tokio::join!(new_session, async {
        timeout(Duration::from_secs(5), lines.next_line()).await.unwrap().unwrap()
    });
    if let Some(AcpMessage::Response { id, result }) = parse_line(&line.unwrap()) {
        peer.resolve_response(id, result).await;
    }
    let session_result = session_result.expect("session/new should succeed");
    let session_id = session_result["sessionId"].as_str().unwrap().to_string();
    assert_eq!(session_id, "fake-session-1");

    // session/prompt - collect every notification until the matching response.
    let prompt = peer.request("session/prompt", serde_json::json!({ "sessionId": session_id }));
    tokio::pin!(prompt);

    let mut plan_snapshots: Vec<serde_json::Value> = Vec::new();
    let mut saw_tool_call = false;
    let mut saw_tool_call_update = false;

    let prompt_result = loop {
        tokio::select! {
            result = &mut prompt => break result,
            line = timeout(Duration::from_secs(5), lines.next_line()) => {
                let Some(line) = line.unwrap().unwrap() else { continue };
                match parse_line(&line) {
                    Some(AcpMessage::Notification { method, params }) if method == "session/update" => {
                        let kind = params["update"]["sessionUpdate"].as_str().unwrap_or("");
                        match kind {
                            "plan" => plan_snapshots.push(params["update"]["entries"].clone()),
                            "tool_call" => saw_tool_call = true,
                            "tool_call_update" => saw_tool_call_update = true,
                            _ => {}
                        }
                    }
                    Some(AcpMessage::Response { id, result }) => {
                        peer.resolve_response(id, result).await;
                    }
                    _ => {}
                }
            }
        }
    };

    let final_result = prompt_result.expect("session/prompt should succeed");
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

    let _ = child.kill().await;
}
