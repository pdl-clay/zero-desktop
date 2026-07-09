use std::time::Duration;

use app_lib::bridge::{InputEvent, OutputEvent};
use app_lib::locator;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

const TURN_TIMEOUT: Duration = Duration::from_secs(120);

fn temp_workspace() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("zero-desktop-test");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn zero_data_dir() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".local").join("share").join("zero");
    }
    dirs_next().unwrap_or_else(|| std::path::PathBuf::from(".")).join("zero")
}

fn dirs_next() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| std::path::PathBuf::from(h).join(".local").join("share"))
            })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn session_dir(session_id: &str) -> std::path::PathBuf {
    zero_data_dir().join("sessions").join(session_id)
}

fn spawn_zero(cwd: &std::path::Path)
    -> (tokio::process::Child, tokio::process::ChildStdin, tokio::process::ChildStdout, tokio::process::ChildStderr)
{
    let zero_path = locator::locate_zero()
        .expect("zero CLI must be installed to run integration tests")
        .path;

    let mut child = tokio::process::Command::new(&zero_path)
        .arg("exec")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--cwd")
        .arg(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn zero exec");

    let stdin = child.stdin.take().expect("stdin must be available");
    let stdout = child.stdout.take().expect("stdout must be available");
    let stderr = child.stderr.take().expect("stderr must be available");

    (child, stdin, stdout, stderr)
}

fn spawn_zero_resume(cwd: &std::path::Path, session_id: &str)
    -> (tokio::process::Child, tokio::process::ChildStdin, tokio::process::ChildStdout, tokio::process::ChildStderr)
{
    let zero_path = locator::locate_zero()
        .expect("zero CLI must be installed to run integration tests")
        .path;

    let mut child = tokio::process::Command::new(&zero_path)
        .arg("exec")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--cwd")
        .arg(cwd)
        .arg("--resume")
        .arg(session_id)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn zero exec with --resume");

    let stdin = child.stdin.take().expect("stdin must be available");
    let stdout = child.stdout.take().expect("stdout must be available");
    let stderr = child.stderr.take().expect("stderr must be available");

    (child, stdin, stdout, stderr)
}

async fn send_and_receive(
    stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
    content: &str,
    timeout_duration: Duration,
) -> Result<Vec<OutputEvent>, String> {
    let mut lines = BufReader::new(stdout).lines();
    let mut writer = tokio::io::BufWriter::new(stdin);

    let event = InputEvent::user_message(content.to_string());
    let line = serde_json::to_string(&event).map_err(|e| e.to_string())?;
    writer.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.write_all(b"\n").await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;
    drop(writer);

    let mut events = Vec::new();
    loop {
        let line = timeout(timeout_duration, lines.next_line())
            .await
            .map_err(|_| "timeout waiting for stdout event".to_string())?
            .map_err(|e| format!("read error: {e}"))?;

        let line = line.ok_or_else(|| "stdout stream ended unexpectedly".to_string())?;

        let event: OutputEvent = serde_json::from_str(&line)
            .map_err(|e| format!("parse error: {e} in line: {line}"))?;

        let is_run_end = event.event_type == "run_end";
        events.push(event);

        if is_run_end {
            break;
        }
    }

    Ok(events)
}

fn has_type(events: &[OutputEvent], event_type: &str) -> bool {
    events.iter().any(|e| e.event_type == event_type)
}

fn event_types(events: &[OutputEvent]) -> Vec<&str> {
    events.iter().map(|e| e.event_type.as_str()).collect()
}

#[tokio::test]
async fn test_locator_finds_zero() {
    let location = locator::locate_zero().expect("zero CLI must be found");
    assert!(location.path.is_file(), "zero path must be a file");
    assert!(
        location.version.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
        "version must be present"
    );
}

#[tokio::test]
async fn test_session_emits_run_start() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with: OK", TURN_TIMEOUT).await.unwrap();

    assert!(has_type(&events, "run_start"), "must have run_start. types: {:?}", event_types(&events));
    assert_eq!(events.iter().find(|e| e.event_type == "run_start").unwrap().schema_version, 2);

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_complete_response_flow() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with exactly: OK", TURN_TIMEOUT).await.unwrap();

    let types = event_types(&events);
    assert!(types.contains(&"run_start"), "must have run_start. types: {:?}", types);
    assert!(types.contains(&"text"), "must have text streaming. types: {:?}", types);
    assert!(types.contains(&"final"), "must have final. types: {:?}", types);
    assert!(types.contains(&"run_end"), "must have run_end. types: {:?}", types);

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_response_contains_expected_text() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with exactly: HELLO_WORLD", TURN_TIMEOUT).await.unwrap();

    let final_event = events.iter().find(|e| e.event_type == "final")
        .unwrap_or_else(|| panic!("no final event. types: {:?}", event_types(&events)));

    let text = final_event.payload["text"].as_str()
        .unwrap_or_else(|| panic!("no 'text' in final payload: {:?}", final_event.payload));

    assert!(
        text.to_lowercase().contains("hello"),
        "final response should contain 'HELLO', got: {text}"
    );

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_run_start_metadata() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with: OK", TURN_TIMEOUT).await.unwrap();

    let run_start = events.iter().find(|e| e.event_type == "run_start")
        .expect("must have run_start event");

    assert!(run_start.payload["sessionId"].is_string(), "must have sessionId");
    assert!(run_start.payload["cwd"].is_string(), "must have cwd");
    assert!(run_start.payload["model"].is_string(), "must have model");

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_run_end_success_status() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with: SUCCESS", TURN_TIMEOUT).await.unwrap();

    let run_end = events.iter().find(|e| e.event_type == "run_end")
        .expect("must have run_end event");

    assert_eq!(run_end.payload["status"], "success", "run_end status should be success");
    assert_eq!(run_end.payload["exitCode"], 0, "run_end exitCode should be 0");

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_json_serialization_roundtrip_all_events() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with: TEST", TURN_TIMEOUT).await.unwrap();

    for event in &events {
        let serialized = serde_json::to_string(event)
            .unwrap_or_else(|e| panic!("failed to serialize {:?}: {e}", event.event_type));
        let deserialized: OutputEvent = serde_json::from_str(&serialized)
            .unwrap_or_else(|e| panic!("roundtrip failed for '{}': {e}\n{serialized}", event.event_type));

        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.schema_version, event.schema_version);
    }

    child.kill().await.ok();
    let _ = child.wait().await;
}

#[tokio::test]
async fn test_stderr_no_errors() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, stderr) = spawn_zero(&workspace);

    let mut stderr_lines = BufReader::new(stderr).lines();
    let stderr_task = tokio::spawn(async move {
        let mut collected = Vec::new();
        while let Ok(Some(line)) =
            timeout(Duration::from_secs(5), stderr_lines.next_line()).await.unwrap_or(Ok(None))
        {
            collected.push(line);
        }
        collected
    });

    let events = send_and_receive(stdin, stdout, "Respond with: OK", TURN_TIMEOUT).await.unwrap();
    assert!(has_type(&events, "run_end"));

    child.kill().await.ok();
    let _ = child.wait().await;

    let stderr_output = timeout(Duration::from_secs(5), stderr_task)
        .await.expect("stderr task timed out").expect("stderr task panicked");

    let error_lines: Vec<_> = stderr_output.iter()
        .filter(|l| l.to_lowercase().contains("error"))
        .collect();
    if !error_lines.is_empty() {
        eprintln!("stderr error-like lines: {:?}", error_lines);
    }
}

#[tokio::test]
async fn test_multiple_turns_new_sessions() {
    let workspace = temp_workspace();

    for msg in &["Respond with: OK", "Respond with: YES"] {
        let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

        let events = send_and_receive(stdin, stdout, msg, TURN_TIMEOUT).await.unwrap();

        let has_text = has_type(&events, "text") || has_type(&events, "final");
        assert!(has_text, "must get response for '{msg}'. types: {:?}", event_types(&events));

        let has_error = has_type(&events, "error");
        assert!(!has_error, "must not have error for '{msg}'. types: {:?}", event_types(&events));

        child.kill().await.ok();
        let _ = child.wait().await;
    }
}

#[tokio::test]
async fn test_input_event_serialization() {
    let event = InputEvent::user_message("test message".to_string());
    let json = serde_json::to_value(&event).unwrap();

    assert_eq!(json["schemaVersion"], 2);
    assert_eq!(json["type"], "message");
    assert_eq!(json["role"], "user");
    assert_eq!(json["content"], "test message");
}

#[tokio::test]
async fn test_output_event_deserialization_all_types() {
    let test_cases = vec![
        (r#"{"schemaVersion":2,"type":"run_start","sessionId":"abc"}"#, "run_start"),
        (r#"{"schemaVersion":2,"type":"text","delta":"hello"}"#, "text"),
        (r#"{"schemaVersion":2,"type":"final","text":"done"}"#, "final"),
        (r#"{"schemaVersion":2,"type":"run_end","status":"success"}"#, "run_end"),
        (r#"{"schemaVersion":2,"type":"error","message":"oops"}"#, "error"),
        (r#"{"schemaVersion":2,"type":"usage","totalTokens":100}"#, "usage"),
    ];

    for (json, expected_type) in &test_cases {
        let event: OutputEvent = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", expected_type));
        assert_eq!(event.event_type, *expected_type,
            "expected '{}' got '{}'", expected_type, event.event_type);
    }
}

#[tokio::test]
async fn test_output_event_handles_extra_fields() {
    let json = r#"{"schemaVersion":2,"type":"text","delta":"x","extra":42,"unknown":"y"}"#;
    let event: OutputEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type, "text");
    assert_eq!(event.payload["delta"], "x");
}

#[tokio::test]
async fn test_no_sessions_cleans_up() {
    let workspace = temp_workspace();
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);

    let events = send_and_receive(stdin, stdout, "Respond with: QUICK", TURN_TIMEOUT).await.unwrap();
    assert!(has_type(&events, "run_end"));

    child.kill().await.ok();
    let status = child.wait().await;
    assert!(status.is_ok(), "process should exit cleanly: {:?}", status);
}

#[tokio::test]
async fn test_multi_turn_context_preserved_with_resume() {
    let workspace = temp_workspace();

    // Turn 1: provide a fact that the model should remember.
    let (mut child1, stdin1, stdout1, _stderr1) = spawn_zero(&workspace);
    let events1 = send_and_receive(
        stdin1,
        stdout1,
        "My name is Alice. Remember it and answer with just: OK",
        TURN_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(has_type(&events1, "run_end"), "turn 1 must complete. types: {:?}", event_types(&events1));

    let session_id = events1
        .iter()
        .find(|e| e.event_type == "run_start")
        .expect("turn 1 must have run_start")
        .payload["sessionId"]
        .as_str()
        .expect("sessionId must be a string");

    child1.kill().await.ok();
    let _ = child1.wait().await;

    // Turn 2: resume the same session and ask a follow-up question.
    let (mut child2, stdin2, stdout2, _stderr2) = spawn_zero_resume(&workspace, session_id);
    let events2 = send_and_receive(
        stdin2,
        stdout2,
        "What is my name? Answer with just the name.",
        TURN_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(has_type(&events2, "run_end"), "turn 2 must complete. types: {:?}", event_types(&events2));

    let final_event = events2
        .iter()
        .find(|e| e.event_type == "final")
        .unwrap_or_else(|| panic!("turn 2 must have final. types: {:?}", event_types(&events2)));
    let text = final_event.payload["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text in final: {:?}", final_event.payload));

    assert!(
        text.to_lowercase().contains("alice"),
        "--resume must preserve conversation context. expected 'Alice' in response, got: {text} (session: {session_id})"
    );

    child2.kill().await.ok();
    let _ = child2.wait().await;
}

#[tokio::test]
async fn test_sessions_list_filters_by_cwd() {
    let workspace = temp_workspace();

    // Send a message to create a session in this workspace
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);
    let events = send_and_receive(stdin, stdout, "Respond with: OK", TURN_TIMEOUT).await.unwrap();

    let session_id = events
        .iter()
        .find(|e| e.event_type == "run_start")
        .unwrap()
        .payload["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    child.kill().await.ok();
    let _ = child.wait().await;

    // Run zero sessions list --json and parse manually
    let zero_path = locator::locate_zero().unwrap().path;
    let output = std::process::Command::new(&zero_path)
        .arg("sessions")
        .arg("list")
        .arg("--json")
        .output()
        .expect("failed to run zero sessions list");

    let all_sessions: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("failed to parse sessions JSON");

    let cwd_str = workspace.to_string_lossy().to_string();
    let filtered: Vec<_> = all_sessions
        .iter()
        .filter(|s| s["cwd"].as_str().unwrap_or("") == cwd_str)
        .collect();

    assert!(
        !filtered.is_empty(),
        "sessions list must find sessions in workspace {}",
        cwd_str
    );

    let found = filtered
        .iter()
        .any(|s| s["sessionId"].as_str().unwrap_or("") == session_id);

    assert!(
        found,
        "newly created session {} must appear in sessions list for {}",
        session_id, cwd_str
    );
}

#[tokio::test]
async fn test_session_info_fields() {
    let workspace = temp_workspace();

    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);
    let _events = send_and_receive(stdin, stdout, "Respond with: OK", TURN_TIMEOUT).await.unwrap();
    child.kill().await.ok();
    let _ = child.wait().await;

    let zero_path = locator::locate_zero().unwrap().path;
    let output = std::process::Command::new(&zero_path)
        .arg("sessions")
        .arg("list")
        .arg("--json")
        .output()
        .unwrap();

    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let cwd_str = workspace.to_string_lossy().to_string();
    let ours: Vec<_> = sessions
        .iter()
        .filter(|s| s["cwd"].as_str().unwrap_or("") == cwd_str)
        .collect();

    if let Some(session) = ours.first() {
        assert!(session["sessionId"].is_string(), "must have sessionId");
        assert!(session["createdAt"].is_string(), "must have createdAt");
        assert!(session["modelId"].is_string(), "must have modelId");
        assert_eq!(session["cwd"], cwd_str, "cwd must match workspace");
    }
}

#[tokio::test]
async fn test_delete_session_removes_from_list() {
    let workspace = temp_workspace();

    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);
    let events = send_and_receive(stdin, stdout, "Respond with: DELETE_TEST", TURN_TIMEOUT).await.unwrap();
    child.kill().await.ok();
    let _ = child.wait().await;

    let session_id = events
        .iter()
        .find(|e| e.event_type == "run_start")
        .unwrap()
        .payload["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // Verify session exists on disk
    let dir = session_dir(&session_id);
    assert!(dir.exists(), "session dir must exist at {:?}", dir);

    // Verify session appears in list
    let zero_path = locator::locate_zero().unwrap().path;
    let output = std::process::Command::new(&zero_path)
        .arg("sessions")
        .arg("list")
        .arg("--json")
        .output()
        .unwrap();
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let cwd_str = workspace.to_string_lossy().to_string();
    let found_before = sessions.iter().any(|s| {
        s["sessionId"].as_str().unwrap_or("") == session_id
            && s["cwd"].as_str().unwrap_or("") == cwd_str
    });
    assert!(found_before, "session must appear in list before deletion");

    // Delete the session
    std::fs::remove_dir_all(&dir).expect("failed to delete session dir");
    assert!(!dir.exists(), "session dir must be gone after deletion");

    // Verify session no longer appears in list
    let output = std::process::Command::new(&zero_path)
        .arg("sessions")
        .arg("list")
        .arg("--json")
        .output()
        .unwrap();
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let found_after = sessions.iter().any(|s| {
        s["sessionId"].as_str().unwrap_or("") == session_id
            && s["cwd"].as_str().unwrap_or("") == cwd_str
    });
    assert!(!found_after, "session must NOT appear in list after deletion");
}

#[tokio::test]
async fn test_message_history_recovery_from_events_jsonl() {
    let workspace = temp_workspace();

    let user_msg = "My name is Bob and I like Rust";
    let (mut child, stdin, stdout, _stderr) = spawn_zero(&workspace);
    let events = send_and_receive(stdin, stdout, user_msg, TURN_TIMEOUT).await.unwrap();
    child.kill().await.ok();
    let _ = child.wait().await;

    let session_id = events
        .iter()
        .find(|e| e.event_type == "run_start")
        .unwrap()
        .payload["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // Read events.jsonl
    let events_path = session_dir(&session_id).join("events.jsonl");
    assert!(events_path.exists(), "events.jsonl must exist");

    let content = std::fs::read_to_string(&events_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(!lines.is_empty(), "events.jsonl must have at least one event");

    // Parse all events
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect();

    // Find message events
    let messages: Vec<&serde_json::Value> = parsed
        .iter()
        .filter(|e| e["type"].as_str().unwrap_or("") == "message")
        .collect();

    // At minimum: user message + assistant response
    assert!(
        messages.len() >= 2,
        "must have at least 2 messages (user + assistant), got {}",
        messages.len()
    );

    // First message should be the user message
    let first_msg = &messages[0];
    assert_eq!(
        first_msg["payload"]["role"].as_str().unwrap_or(""),
        "user",
        "first message role must be 'user'"
    );
    assert!(
        first_msg["payload"]["content"]
            .as_str()
            .unwrap_or("")
            .contains("Bob"),
        "first message content must contain what we sent"
    );

    // Second message should be assistant response
    let second_msg = &messages[1];
    assert_eq!(
        second_msg["payload"]["role"].as_str().unwrap_or(""),
        "assistant",
        "second message role must be 'assistant'"
    );
    assert!(
        !second_msg["payload"]["content"]
            .as_str()
            .unwrap_or("")
            .is_empty(),
        "assistant response must not be empty"
    );

    // All message events must have required fields
    for msg in &messages {
        assert!(msg["id"].is_string(), "message event must have id");
        assert!(msg["sessionId"].is_string(), "message event must have sessionId");
        assert!(msg["createdAt"].is_string(), "message event must have createdAt");
        assert!(msg["sequence"].is_number(), "message event must have sequence");
    }
}
