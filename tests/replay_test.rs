//! Integration tests for replay engine options (CONF-20 to CONF-28).
//!
//! Tests dry-run, skip-indices, from-index, skip-pattern, force flag,
//! and nonexistent session error handling.
//!
//! Skips: CONF-21 (step mode, requires TTY), CONF-26 (original cwd),
//! CONF-27 (danger-policy, tested in `replay_danger_policy.rs`).

mod common;

use rec::cli::Output;
use rec::models::config::Config;
use rec::models::{Command, Session};
use rec::replay::{ReplayEngine, ReplayOptions};
use std::path::PathBuf;

/// Create a test session with the given command strings.
fn test_session(commands: &[&str]) -> Session {
    let mut session = Session::new("replay-option-test");
    for (i, cmd_text) in commands.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let mut cmd = Command::new(i as u32, cmd_text.to_string(), PathBuf::from("/tmp"));
        cmd.complete(0);
        session.add_command(cmd);
    }
    session
}

/// Quiet output for tests.
fn quiet_output() -> Output {
    Output::new(false, true, false)
}

/// CONF-20: Dry-run mode previews commands without executing any.
#[test]
fn test_replay_dry_run() {
    let session = test_session(&["echo hello", "echo world", "echo done"]);
    let config = Config::default();
    let options = ReplayOptions {
        dry_run: true,
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, quiet_output());
    let summary = engine.run().unwrap();

    assert_eq!(summary.total, 3);
    assert_eq!(
        summary.executed, 0,
        "dry-run should not execute any commands"
    );
    assert_eq!(summary.skipped, 0);
    assert!(!summary.aborted);
}

/// CONF-22: Skip indices excludes specified command positions.
#[test]
fn test_replay_skip_indices() {
    let session = test_session(&["echo a", "echo b", "echo c"]);
    let config = Config::default();
    let mut options = ReplayOptions {
        dry_run: true,
        ..Default::default()
    };
    options.skip_indices.insert(1); // skip second command (0-based)

    let mut engine = ReplayEngine::new(session, options, &config, quiet_output());
    let summary = engine.run().unwrap();

    assert_eq!(summary.total, 3);
    assert_eq!(summary.skipped, 1, "should skip command at index 1");
    // In dry-run, non-skipped commands are displayed but not executed
    assert_eq!(summary.executed, 0);
}

/// CONF-23: From-index starts replay at the specified position.
#[test]
fn test_replay_from_index() {
    let session = test_session(&["echo a", "echo b", "echo c", "echo d"]);
    let config = Config::default();
    let options = ReplayOptions {
        dry_run: true,
        from_index: Some(2), // start from third command (0-based)
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, quiet_output());
    let summary = engine.run().unwrap();

    assert_eq!(summary.total, 4);
    // Commands before from_index are silently skipped (not counted as skipped)
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.executed, 0); // dry-run
}

/// CONF-24: Skip-pattern excludes commands matching glob patterns.
#[test]
fn test_replay_skip_pattern() {
    let session = test_session(&["echo hello", "rm something", "echo world"]);
    let config = Config::default();
    let mut options = ReplayOptions {
        dry_run: true,
        ..Default::default()
    };
    options
        .skip_patterns
        .push(glob::Pattern::new("rm*").unwrap());

    let mut engine = ReplayEngine::new(session, options, &config, quiet_output());
    let summary = engine.run().unwrap();

    assert_eq!(summary.total, 3);
    assert_eq!(
        summary.skipped, 1,
        "rm command should be skipped by pattern"
    );
}

/// CONF-25: Force flag is accepted and bypasses destructive prompts.
#[test]
fn test_replay_force_flag_accepted() {
    // Use only safe commands to avoid actual destructive execution
    let session = test_session(&["echo safe1", "echo safe2"]);
    let config = Config::default();
    let options = ReplayOptions {
        force: true,
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, quiet_output());
    let summary = engine.run().unwrap();

    assert_eq!(summary.executed, 2, "force should allow execution");
    assert!(!summary.aborted);
    assert_eq!(summary.failed, 0);
}

/// CONF-28: Replaying a nonexistent session produces non-zero exit.
#[test]
fn test_replay_nonexistent_session_errors() {
    let env = common::TestEnv::new();

    #[allow(deprecated)]
    assert_cmd::Command::cargo_bin("rec")
        .expect("binary exists")
        .arg("replay")
        .arg("nonexistent-session")
        .env("REC_DATA_DIR", env.paths.data_dir.as_os_str())
        .env("REC_CONFIG_DIR", env.paths.config_dir.as_os_str())
        .env("REC_STATE_DIR", env.paths.state_dir.as_os_str())
        .assert()
        .failure();
}
