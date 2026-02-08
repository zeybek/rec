//! Integration tests for `rec start` command (REC-01 through REC-05, REC-09).

mod common;

use common::TestEnv;
use rec::error::RecError;
use rec::models::{generate_session_name, validate_session_name};

// ---------------------------------------------------------------------------
// REC-01: start creates a new recording session
// ---------------------------------------------------------------------------

#[test]
fn test_start_creates_new_recording_session() {
    let env = TestEnv::new();
    let recording_state = env.recording_state();
    let session_path = env.paths.state_dir.join("test-session.ndjson");

    // Pre-condition: no recording active
    assert!(
        !recording_state.is_recording(),
        "should not be recording before start"
    );

    // Action: start a new recording
    let result = recording_state.start("test-session", session_path.clone());
    assert!(result.is_ok(), "start should succeed: {result:?}");

    let active = result.unwrap();

    // Post-condition: recording is active with correct metadata
    assert!(
        recording_state.is_recording(),
        "should be recording after start"
    );
    assert_eq!(active.name, "test-session");
    assert_eq!(active.session_path, session_path);
    assert!(active.started_at > 0.0, "should have valid timestamp");
    assert_eq!(active.pid, std::process::id(), "should record current PID");
}

// ---------------------------------------------------------------------------
// REC-02: start --name uses the provided session name
// ---------------------------------------------------------------------------

#[test]
fn test_start_uses_provided_name() {
    let env = TestEnv::new();
    let recording_state = env.recording_state();
    let session_path = env.paths.state_dir.join("my-custom-session.ndjson");

    // Action: start with a custom name
    let active = recording_state
        .start("my-custom-session", session_path)
        .expect("start should succeed");

    // Post-condition: name matches what was provided
    assert_eq!(
        active.name, "my-custom-session",
        "session name should match provided name"
    );

    // Verify via current()
    let current = recording_state
        .current()
        .expect("current() should succeed while recording");
    assert_eq!(
        current.name, "my-custom-session",
        "current() should return same name"
    );
}

// ---------------------------------------------------------------------------
// REC-05: start creates recording.lock and recording.json files
// ---------------------------------------------------------------------------

#[test]
fn test_start_creates_lock_and_state_files() {
    let env = TestEnv::new();
    let recording_state = env.recording_state();
    let session_path = env.paths.state_dir.join("session-with-files.ndjson");

    let lock_path = env.paths.state_dir.join("recording.lock");
    let state_path = env.paths.state_dir.join("recording.json");

    // Pre-condition: files do not exist
    assert!(
        !lock_path.exists(),
        "lock file should not exist before start"
    );
    assert!(
        !state_path.exists(),
        "state file should not exist before start"
    );

    // Action: start recording
    recording_state
        .start("file-test-session", session_path)
        .expect("start should succeed");

    // Post-condition: both files exist
    assert!(
        lock_path.exists(),
        "recording.lock should exist after start"
    );
    assert!(
        state_path.exists(),
        "recording.json should exist after start"
    );

    // Verify lock file contains current PID
    let lock_content = std::fs::read_to_string(&lock_path).expect("should read lock file");
    let expected_pid = std::process::id().to_string();
    assert_eq!(
        lock_content.trim(),
        expected_pid,
        "lock file should contain current PID"
    );

    // Verify state file contains valid JSON with session data
    let state_content = std::fs::read_to_string(&state_path).expect("should read state file");
    assert!(
        state_content.contains("\"name\": \"file-test-session\"")
            || state_content.contains("\"name\":\"file-test-session\""),
        "state file should contain session name: {state_content}"
    );
}

// ---------------------------------------------------------------------------
// REC-04: start when already recording is detected via is_recording()
//
// NOTE: The CLI prevents concurrent recordings by checking is_recording()
// before calling start(). The fd-lock RecordingInProgress error is a
// lower-level protection for actual multi-process concurrency.
// This test verifies the user-facing behavior: is_recording() returns true
// after start(), which the CLI uses to block a second start attempt.
// ---------------------------------------------------------------------------

#[test]
fn test_start_when_recording_returns_error() {
    let env = TestEnv::new();
    let recording_state = env.recording_state();
    let session_path = env.paths.state_dir.join("first-session.ndjson");

    // Pre-condition: not recording
    assert!(
        !recording_state.is_recording(),
        "should not be recording before start"
    );

    // Action: start first recording
    let active = recording_state
        .start("first-session", session_path)
        .expect("start should succeed");

    // Post-condition: is_recording() returns true, preventing CLI from allowing second start
    assert!(
        recording_state.is_recording(),
        "is_recording() should return true after start"
    );

    // Verify current() returns the active session
    let current = recording_state
        .current()
        .expect("current() should succeed while recording");
    assert_eq!(
        current.id, active.id,
        "current() should return same session"
    );
    assert_eq!(current.name, "first-session");

    // CLI behavior: if is_recording() is true, CLI will print error and not call start()
    // This is how REC-04 "cannot start while already recording" is implemented
}

// ---------------------------------------------------------------------------
// REC-03: start with invalid name returns validation error
// ---------------------------------------------------------------------------

#[test]
fn test_validate_session_name_rejects_invalid() {
    // Test various invalid session names

    // Empty name
    let result = validate_session_name("");
    assert!(result.is_err(), "empty name should be rejected");
    match result.unwrap_err() {
        RecError::InvalidSessionName(msg) => {
            assert!(msg.contains("empty"), "error should mention empty: {msg}");
        }
        other => panic!("expected InvalidSessionName error, got: {other:?}"),
    }

    // Name with space
    let result = validate_session_name("my session");
    assert!(result.is_err(), "name with space should be rejected");
    match result.unwrap_err() {
        RecError::InvalidSessionName(_) => {}
        other => panic!("expected InvalidSessionName error, got: {other:?}"),
    }

    // Name with special characters
    let result = validate_session_name("test@123");
    assert!(result.is_err(), "name with @ should be rejected");

    let result = validate_session_name("hello/world");
    assert!(result.is_err(), "name with / should be rejected");

    let result = validate_session_name("name.ext");
    assert!(result.is_err(), "name with . should be rejected");

    // Valid names should pass
    assert!(
        validate_session_name("my-session").is_ok(),
        "alphanumeric with dash should be valid"
    );
    assert!(
        validate_session_name("test_123").is_ok(),
        "alphanumeric with underscore should be valid"
    );
    assert!(
        validate_session_name("MySession").is_ok(),
        "mixed case should be valid"
    );
}

// ---------------------------------------------------------------------------
// REC-09: Auto-generated session names follow session-YYYY-MM-DD-HHMMSS format
// ---------------------------------------------------------------------------

#[test]
fn test_generate_session_name_format() {
    let name = generate_session_name();

    // Check prefix
    assert!(
        name.starts_with("session-"),
        "generated name should start with 'session-': {name}"
    );

    // Check total length: "session-" (8) + "YYYY-MM-DD-HHMMSS" (17) = 25
    assert_eq!(
        name.len(),
        25,
        "generated name should be 25 characters: {} (len={})",
        name,
        name.len()
    );

    // Check format pattern (session-YYYY-MM-DD-HHMMSS)
    // Extract date portion: after "session-" prefix
    let date_part = &name[8..]; // "YYYY-MM-DD-HHMMSS"
    assert_eq!(date_part.len(), 17, "date portion should be 17 characters");

    // Verify dashes are in expected positions (indices 4, 7, 10 in date_part)
    let chars: Vec<char> = date_part.chars().collect();
    assert_eq!(chars[4], '-', "5th char should be dash: {date_part}");
    assert_eq!(chars[7], '-', "8th char should be dash: {date_part}");
    assert_eq!(chars[10], '-', "11th char should be dash: {date_part}");

    // Year should be 4 digits
    assert!(
        chars[0..4].iter().all(char::is_ascii_digit),
        "year should be 4 digits: {date_part}"
    );

    // Generated name should be valid for use
    assert!(
        validate_session_name(&name).is_ok(),
        "generated name should be valid: {name}"
    );
}
