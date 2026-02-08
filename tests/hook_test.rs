//! Integration tests for `rec _hook` commands (REC-13 through REC-15).
//!
//! Validates that shell hooks correctly capture command information,
//! track exit codes, and exit cleanly (code 0, no stdout) when not recording.
//!
//! The _hook commands use `Paths::new()` internally, so we test:
//! - CLI exit behavior when not recording (REC-15) via `assert_cmd`
//! - Library-level `CommandCapture` behavior (REC-13, REC-14) via struct literals

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use common::TestEnv;
use rec::models::{Command, SessionHeader};
use rec::recording::CommandCapture;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helper: Create a minimal SessionHeader for library tests
// ---------------------------------------------------------------------------

fn sample_header() -> SessionHeader {
    SessionHeader {
        version: 2,
        id: Uuid::new_v4(),
        name: "hook-test-session".to_string(),
        shell: "bash".to_string(),
        os: "linux".to_string(),
        hostname: "testhost".to_string(),
        env: HashMap::new(),
        tags: vec![],
        recovered: None,
        started_at: 1_700_000_000.123,
    }
}

// ---------------------------------------------------------------------------
// REC-15: _hook preexec exits cleanly when not recording
// ---------------------------------------------------------------------------

#[test]
fn test_hook_preexec_exits_cleanly_when_not_recording() {
    // When not recording, `rec _hook preexec <command>` should:
    // - Exit with code 0 (success)
    // - Produce no stdout output (critical for shell hooks - stdout goes to prompt)
    // Note: --quiet suppresses debug output that may appear from user config
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["--quiet", "_hook", "preexec", "echo hello"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

// ---------------------------------------------------------------------------
// REC-15: _hook precmd exits cleanly when not recording
// ---------------------------------------------------------------------------

#[test]
fn test_hook_precmd_exits_cleanly_when_not_recording() {
    // When not recording, `rec _hook precmd <exit_code>` should:
    // - Exit with code 0 (success)
    // - Produce no stdout output (critical for shell hooks - stdout goes to prompt)
    // Note: --quiet suppresses debug output that may appear from user config
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["--quiet", "_hook", "precmd", "0"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

// ---------------------------------------------------------------------------
// REC-14: append_command includes exit code
// ---------------------------------------------------------------------------

#[test]
fn test_append_command_includes_exit_code() {
    // Verify that CommandCapture::append_command correctly records
    // the exit code in the serialized NDJSON
    let env = TestEnv::new();
    let session_path = env.paths.data_dir.join("exit-code-test.ndjson");

    // Write header first
    CommandCapture::write_header(&session_path, &sample_header())
        .expect("write_header should succeed");

    // Append a command with a specific exit code (non-zero)
    let cmd = Command {
        index: 0,
        command: "failing-command".to_string(),
        cwd: PathBuf::from("/test/dir"),
        started_at: 1_700_000_001.0,
        ended_at: Some(1_700_000_002.0),
        exit_code: Some(127),
        duration_ms: Some(1000),
    };

    CommandCapture::append_command(&session_path, &cmd).expect("append_command should succeed");

    // Read and verify the NDJSON content
    let content = std::fs::read_to_string(&session_path).expect("read session file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "Should have header + 1 command");

    // Parse the command line and verify exit_code
    let cmd_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("command line should be valid JSON");

    assert_eq!(cmd_line["type"], "command");
    assert_eq!(cmd_line["exit_code"], 127, "exit_code should be 127");
}

// ---------------------------------------------------------------------------
// REC-13: Command capture preserves cwd
// ---------------------------------------------------------------------------

#[test]
fn test_command_capture_preserves_cwd() {
    // Verify that CommandCapture::append_command correctly records
    // the working directory in the serialized NDJSON
    let env = TestEnv::new();
    let session_path = env.paths.data_dir.join("cwd-test.ndjson");

    // Write header first
    CommandCapture::write_header(&session_path, &sample_header())
        .expect("write_header should succeed");

    // Append a command with a specific cwd
    let test_cwd = PathBuf::from("/home/user/project/src");
    let cmd = Command {
        index: 0,
        command: "cargo build".to_string(),
        cwd: test_cwd.clone(),
        started_at: 1_700_000_001.0,
        ended_at: Some(1_700_000_005.0),
        exit_code: Some(0),
        duration_ms: Some(4000),
    };

    CommandCapture::append_command(&session_path, &cmd).expect("append_command should succeed");

    // Read and verify the NDJSON content
    let content = std::fs::read_to_string(&session_path).expect("read session file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "Should have header + 1 command");

    // Parse the command line and verify cwd
    let cmd_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("command line should be valid JSON");

    assert_eq!(cmd_line["type"], "command");
    assert_eq!(
        cmd_line["cwd"], "/home/user/project/src",
        "cwd should be preserved"
    );
}

// ---------------------------------------------------------------------------
// Command NDJSON structure verification
// ---------------------------------------------------------------------------

#[test]
fn test_command_ndjson_structure() {
    // Verify that CommandCapture::append_command produces the correct
    // NDJSON structure with all expected fields
    let env = TestEnv::new();
    let session_path = env.paths.data_dir.join("structure-test.ndjson");

    // Write header first
    CommandCapture::write_header(&session_path, &sample_header())
        .expect("write_header should succeed");

    // Append a command with all fields populated
    let cmd = Command {
        index: 42,
        command: "git commit -m 'test'".to_string(),
        cwd: PathBuf::from("/home/user/repo"),
        started_at: 1_700_000_010.123,
        ended_at: Some(1_700_000_015.456),
        exit_code: Some(0),
        duration_ms: Some(5333),
    };

    CommandCapture::append_command(&session_path, &cmd).expect("append_command should succeed");

    // Read and parse the command line
    let content = std::fs::read_to_string(&session_path).expect("read session file");
    let lines: Vec<&str> = content.lines().collect();
    let cmd_json: serde_json::Value =
        serde_json::from_str(lines[1]).expect("command line should be valid JSON");

    // Verify structure matches NDJSON schema
    assert_eq!(
        cmd_json["type"], "command",
        "type field should be 'command'"
    );
    assert_eq!(cmd_json["index"], 42, "index should match");
    assert_eq!(
        cmd_json["command"], "git commit -m 'test'",
        "command text should match"
    );
    assert_eq!(cmd_json["cwd"], "/home/user/repo", "cwd should match");
    assert_eq!(
        cmd_json["started_at"], 1_700_000_010.123,
        "started_at should match"
    );
    assert_eq!(
        cmd_json["ended_at"], 1_700_000_015.456,
        "ended_at should match"
    );
    assert_eq!(cmd_json["exit_code"], 0, "exit_code should match");
    assert_eq!(cmd_json["duration_ms"], 5333, "duration_ms should match");

    // Verify no unexpected fields (only known fields present)
    let obj = cmd_json.as_object().expect("should be JSON object");
    let expected_fields: std::collections::HashSet<&str> = [
        "type",
        "index",
        "command",
        "cwd",
        "started_at",
        "ended_at",
        "exit_code",
        "duration_ms",
    ]
    .into_iter()
    .collect();

    for key in obj.keys() {
        assert!(
            expected_fields.contains(key.as_str()),
            "Unexpected field '{key}' in command JSON"
        );
    }
}
