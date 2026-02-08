//! Integration tests for `rec diff` command (DATA-13, DATA-14).
//!
//! Validates that diff compares two sessions and returns Ok,
//! and that the binary returns exit code 1 for nonexistent sessions.

mod common;

use common::{TestEnv, json_output, normal_output};
use rec::session::diff_sessions;

// ---------------------------------------------------------------------------
// DATA-13: diff_sessions compares two sessions and returns Ok
// ---------------------------------------------------------------------------

#[test]
fn test_diff_compares_two_sessions() {
    let env = TestEnv::new();
    let s1 = env.create_session_with_commands("v1", &["echo hello", "ls"]);
    let s2 = env.create_session_with_commands("v2", &["echo hello", "pwd"]);
    let output = normal_output();

    let result = diff_sessions(&s1, &s2, false, &output);
    assert!(result.is_ok(), "diff_sessions should succeed: {result:?}");
}

// ---------------------------------------------------------------------------
// DATA-13 (additional): JSON output mode
// ---------------------------------------------------------------------------

#[test]
fn test_diff_json_output() {
    let env = TestEnv::new();
    let s1 = env.create_session_with_commands("v1", &["echo hello", "ls"]);
    let s2 = env.create_session_with_commands("v2", &["echo hello", "pwd"]);
    let output = json_output();

    let result = diff_sessions(&s1, &s2, true, &output);
    assert!(
        result.is_ok(),
        "diff_sessions with json should succeed: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// DATA-14: rec diff with nonexistent session returns exit code 1
// ---------------------------------------------------------------------------

#[test]
fn test_diff_missing_session_exit_code_1() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["diff", "nonexistent-session-1", "nonexistent-session-2"])
        .assert()
        .failure()
        .code(1);
}
