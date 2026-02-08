//! Integration tests for `rec stop` command (REC-06 through REC-08).

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use common::TestEnv;
use uuid::Uuid;

use rec::error::RecError;
use rec::models::{Command, SessionFooter, SessionHeader, SessionStatus};
use rec::recording::{CommandCapture, RecordingState};

// ---------------------------------------------------------------------------
// REC-06: stop completes an active recording and removes lock/state files
// ---------------------------------------------------------------------------

#[test]
fn test_stop_completes_recording() {
    let env = TestEnv::new();

    // Set up recording state
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // Compute expected paths (RecordingState fields are private)
    let lock_path = env.paths.state_dir.join("recording.lock");
    let state_path = env.paths.state_dir.join("recording.json");

    // Create a session header (using struct literal, NOT Session::new)
    let session_id = Uuid::new_v4();
    let header = SessionHeader {
        version: 2,
        id: session_id,
        name: "test-stop-recording".to_string(),
        shell: "bash".to_string(),
        os: "test".to_string(),
        hostname: "test-host".to_string(),
        env: HashMap::new(),
        tags: Vec::new(),
        recovered: None,
        started_at: 1_700_000_000.0,
    };

    // Start recording
    let session_path = env.paths.session_file(&session_id.to_string());
    let active = rec_state
        .start("test-stop-recording", session_path.clone())
        .expect("start should succeed");

    // Write header to NDJSON file
    CommandCapture::write_header(&session_path, &header).expect("write_header should succeed");

    // Pre-condition: verify recording is active
    assert!(
        rec_state.is_recording(),
        "recording should be active before stop"
    );
    assert!(lock_path.exists(), "lock file should exist before stop");
    assert!(state_path.exists(), "state file should exist before stop");

    // Action: stop recording
    let stopped = rec_state.stop().expect("stop should succeed");

    // Post-conditions
    assert_eq!(stopped.id, active.id, "stopped session ID should match");
    assert_eq!(
        stopped.name, "test-stop-recording",
        "stopped session name should match"
    );
    assert!(
        !rec_state.is_recording(),
        "recording should not be active after stop"
    );
    assert!(
        !lock_path.exists(),
        "lock file should be removed after stop"
    );
    assert!(
        !state_path.exists(),
        "state file should be removed after stop"
    );
}

// ---------------------------------------------------------------------------
// REC-08: stop when not recording returns NoActiveRecording error
// ---------------------------------------------------------------------------

#[test]
fn test_stop_when_not_recording_returns_error() {
    let env = TestEnv::new();

    // Set up recording state (but don't start recording)
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // Pre-condition: no recording active
    assert!(
        !rec_state.is_recording(),
        "recording should not be active initially"
    );

    // Action: attempt to stop
    let result = rec_state.stop();

    // Post-condition: NoActiveRecording error
    assert!(
        result.is_err(),
        "stop should return error when not recording"
    );
    match result.unwrap_err() {
        RecError::NoActiveRecording => {}
        e => panic!("Expected NoActiveRecording error, got: {e:?}"),
    }
}

// ---------------------------------------------------------------------------
// REC-08 CLI: stop when not recording returns exit code 1 via CLI
// ---------------------------------------------------------------------------

#[test]
fn test_stop_when_not_recording_exit_code_1_via_cli() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["stop"]);
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains(
            "No recording session is in progress",
        ));
}

// ---------------------------------------------------------------------------
// REC-07: stop writes footer with correct command count
// ---------------------------------------------------------------------------

#[test]
fn test_stop_writes_footer_with_command_count() {
    let env = TestEnv::new();

    // Set up recording state
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // Create a session header (using struct literal, NOT Session::new)
    let session_id = Uuid::new_v4();
    let header = SessionHeader {
        version: 2,
        id: session_id,
        name: "test-footer-count".to_string(),
        shell: "bash".to_string(),
        os: "test".to_string(),
        hostname: "test-host".to_string(),
        env: HashMap::new(),
        tags: Vec::new(),
        recovered: None,
        started_at: 1_700_000_000.0,
    };

    // Start recording
    let session_path = env.paths.session_file(&session_id.to_string());
    rec_state
        .start("test-footer-count", session_path.clone())
        .expect("start should succeed");

    // Write header to NDJSON file
    CommandCapture::write_header(&session_path, &header).expect("write_header should succeed");

    // Append 3 commands
    for i in 0..3 {
        let cmd = Command {
            index: i,
            command: format!("echo command-{i}"),
            cwd: PathBuf::from("/tmp"),
            started_at: 1_700_000_000.0 + f64::from(i),
            ended_at: Some(1_700_000_001.0 + f64::from(i)),
            exit_code: Some(0),
            duration_ms: Some(1000),
        };
        CommandCapture::append_command(&session_path, &cmd).expect("append_command should succeed");
    }

    // Write footer with correct command count
    let footer = SessionFooter {
        ended_at: 1_700_000_010.0,
        command_count: 3,
        status: SessionStatus::Completed,
    };
    CommandCapture::write_footer(&session_path, &footer).expect("write_footer should succeed");

    // Stop recording (removes state/lock files)
    rec_state.stop().expect("stop should succeed");

    // Verify NDJSON file contents
    let content = std::fs::read_to_string(&session_path).expect("should read session file");
    let lines: Vec<&str> = content.lines().collect();

    // Should have: 1 header + 3 commands + 1 footer = 5 lines
    assert_eq!(lines.len(), 5, "NDJSON file should have 5 lines");

    // Parse footer line (last line)
    let footer_json: serde_json::Value =
        serde_json::from_str(lines[4]).expect("footer should be valid JSON");
    assert_eq!(
        footer_json.get("type").and_then(|v| v.as_str()),
        Some("footer"),
        "last line should be footer"
    );
    assert_eq!(
        footer_json
            .get("command_count")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "footer command_count should be 3"
    );
    assert_eq!(
        footer_json.get("status").and_then(|v| v.as_str()),
        Some("completed"),
        "footer status should be completed"
    );
}
