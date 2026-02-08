//! Integration tests for `rec status` command (REC-10 through REC-12).
//!
//! Validates that status shows active recording details when recording,
//! shows idle state when not recording, and produces valid JSON with --json flag.

mod common;

use common::TestEnv;
use rec::error::RecError;
use rec::recording::RecordingState;

// ---------------------------------------------------------------------------
// REC-10: status shows recording details when active
// ---------------------------------------------------------------------------

#[test]
fn test_status_shows_recording_details_via_api() {
    let env = TestEnv::new();
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // Start a recording
    let session_path = env.paths.data_dir.join("status-test.ndjson");
    let active = rec_state
        .start("status-test", session_path.clone())
        .expect("start should succeed");

    // Verify current() returns correct details
    let current = rec_state.current().expect("current should succeed");
    assert_eq!(current.name, "status-test");
    assert_eq!(current.session_path, session_path);
    assert!(current.started_at > 0.0, "started_at should be positive");
    assert_eq!(current.pid, std::process::id());
    assert_eq!(current.id, active.id);

    // Clean up
    rec_state.stop().expect("stop should succeed");
}

// ---------------------------------------------------------------------------
// REC-11: status is_recording returns false when idle
// ---------------------------------------------------------------------------

#[test]
fn test_status_is_recording_returns_false_when_idle() {
    let env = TestEnv::new();
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // No recording started
    assert!(
        !rec_state.is_recording(),
        "is_recording should return false when no recording is active"
    );
}

// ---------------------------------------------------------------------------
// REC-11 additional: current() without recording returns error
// ---------------------------------------------------------------------------

#[test]
fn test_status_current_without_recording_returns_error() {
    let env = TestEnv::new();
    let rec_state = RecordingState::new(&env.paths.state_dir);

    // No recording started
    let result = rec_state.current();
    assert!(
        result.is_err(),
        "current should return Err when not recording"
    );

    match result.unwrap_err() {
        RecError::NoActiveRecording => {} // Expected
        e => panic!("Expected NoActiveRecording, got {e:?}"),
    }
}

// ---------------------------------------------------------------------------
// REC-12: status --json produces valid JSON output
// ---------------------------------------------------------------------------

#[test]
fn test_status_json_when_not_recording() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec").unwrap();

    cmd.args(["status", "--json", "--quiet"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"recording\""));
}

#[test]
fn test_status_json_output_is_valid_json() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec").unwrap();

    let output = cmd
        .args(["status", "--json", "--quiet"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout_str = String::from_utf8(output).expect("stdout should be valid UTF-8");

    // Parse as JSON to verify structure
    let json: serde_json::Value =
        serde_json::from_str(&stdout_str).expect("status --json should output valid JSON");

    // Verify it has the expected field
    assert!(
        json.get("recording").is_some(),
        "JSON should have 'recording' field"
    );
}
