//! Smoke tests validating the entire test infrastructure.
//!
//! These tests prove that `TestEnv`, session helpers, output factories,
//! and the `assert_cmd` binary invocation all work end-to-end. If any
//! piece of the test infrastructure is broken (wrong imports, missing
//! methods, incorrect construction), these tests catch it immediately.

mod common;

use common::TestEnv;
use rec::models::config::Verbosity;

// ---------------------------------------------------------------------------
// Test 1: TestEnv creates isolated directories
// ---------------------------------------------------------------------------

#[test]
fn test_env_creates_isolated_directories() {
    let env = TestEnv::new();

    // All three directories must exist on disk
    assert!(
        env.paths.data_dir.exists(),
        "data_dir should exist: {:?}",
        env.paths.data_dir
    );
    assert!(
        env.paths.config_dir.exists(),
        "config_dir should exist: {:?}",
        env.paths.config_dir
    );
    assert!(
        env.paths.state_dir.exists(),
        "state_dir should exist: {:?}",
        env.paths.state_dir
    );

    // Paths must be inside a temp directory, NOT in real XDG/home paths
    let data_path = env.paths.data_dir.to_string_lossy();
    assert!(
        !data_path.contains(".local/share/rec"),
        "data_dir should NOT be in real XDG path: {data_path}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Create and save a session, then verify persistence
// ---------------------------------------------------------------------------

#[test]
fn test_env_create_and_save_session() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("smoke-test");

    // Verify session metadata
    assert_eq!(session.header.name, "smoke-test");
    assert!(
        !session.commands.is_empty(),
        "Session should have at least 1 command"
    );

    // Verify persistence via store
    let id_str = session.header.id.to_string();
    assert!(
        env.store.exists(&id_str),
        "Session should exist in store after save"
    );

    // Round-trip: load and verify name matches
    let loaded = env
        .store
        .load(&id_str)
        .expect("Should be able to load saved session");
    assert_eq!(loaded.header.name, "smoke-test");
}

// ---------------------------------------------------------------------------
// Test 3: List multiple sessions
// ---------------------------------------------------------------------------

#[test]
fn test_env_list_sessions() {
    let env = TestEnv::new();

    // Save 3 sessions
    env.create_and_save_session("session-alpha");
    env.create_and_save_session("session-beta");
    env.create_and_save_session("session-gamma");

    // List should return exactly 3 entries
    let listed = env.store.list().expect("list() should succeed");
    assert_eq!(
        listed.len(),
        3,
        "Expected 3 sessions, got {}: {:?}",
        listed.len(),
        listed
    );
}

// ---------------------------------------------------------------------------
// Test 4: Session with custom command list
// ---------------------------------------------------------------------------

#[test]
fn test_env_session_with_commands() {
    let env = TestEnv::new();
    let session = env.create_session_with_commands("multi-cmd", &["ls", "pwd", "whoami"]);

    assert_eq!(session.commands.len(), 3, "Should have 3 commands");
    assert_eq!(session.commands[0].command, "ls");
    assert_eq!(session.commands[1].command, "pwd");
    assert_eq!(session.commands[2].command, "whoami");
}

// ---------------------------------------------------------------------------
// Test 5: Output factory functions
// ---------------------------------------------------------------------------

#[test]
fn test_output_factories() {
    // quiet_output
    let quiet = common::quiet_output();
    assert_eq!(quiet.verbosity, Verbosity::Quiet);
    assert!(!quiet.json);

    // verbose_output
    let verbose = common::verbose_output();
    assert_eq!(verbose.verbosity, Verbosity::Verbose);

    // json_output
    let json = common::json_output();
    assert!(json.json);

    // normal_output
    let normal = common::normal_output();
    assert_eq!(normal.verbosity, Verbosity::Normal);
    assert!(!normal.json);
}

// ---------------------------------------------------------------------------
// Test 6: Binary smoke test via assert_cmd
// ---------------------------------------------------------------------------

#[test]
fn test_binary_shows_help() {
    // Use the binary name "rec" (not the package name "rec-cli")
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    let output = cmd.assert().success();

    // Stdout should mention "rec" (the binary name appears in help text)
    output.stdout(predicates::str::contains("rec"));
}
