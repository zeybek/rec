//! Integration tests for `rec show` command (DATA-05 through DATA-07).

mod common;

use common::TestEnv;

// ---------------------------------------------------------------------------
// DATA-05: show_session displays session details
// ---------------------------------------------------------------------------

#[test]
fn test_show_displays_session_details() {
    let env = TestEnv::new();
    let session = env.create_session_with_commands("show-test", &["echo hello", "ls -la", "pwd"]);
    env.store.save(&session).expect("Failed to save session");

    let output = common::normal_output();
    let result = rec::session::show_session(&session, None, false, &output);
    assert!(result.is_ok(), "show_session should return Ok: {result:?}");
}

// ---------------------------------------------------------------------------
// DATA-06: show with --grep filters commands
// ---------------------------------------------------------------------------

#[test]
fn test_show_grep_filters_commands() {
    let env = TestEnv::new();
    let session =
        env.create_session_with_commands("grep-test", &["echo hello", "ls -la", "echo world"]);
    env.store.save(&session).expect("Failed to save session");

    let output = common::normal_output();
    let result = rec::session::show_session(&session, Some("echo"), false, &output);
    assert!(
        result.is_ok(),
        "show_session with grep should return Ok: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// DATA-07: show nonexistent session returns exit code 1
// ---------------------------------------------------------------------------

#[test]
fn test_show_nonexistent_session_exit_code_1() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["show", "nonexistent-session-xyz-12345"]);
    cmd.assert().failure().code(1);
}
