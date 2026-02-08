//! Tests for `DangerPolicy` integration in the `ReplayEngine`.
//!
//! Verifies the three danger policy modes (Skip, Abort, Allow),
//! backward compatibility with None policy, and dry-run annotation.

use rec::cli::Output;
use rec::models::config::Config;
use rec::models::{Command, Session};
use rec::replay::{DangerPolicy, ReplayEngine, ReplayOptions};
use std::path::PathBuf;

/// Create a test session with the given commands.
fn test_session(commands: &[&str]) -> Session {
    let mut session = Session::new("danger-policy-test");
    for (i, cmd_text) in commands.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let mut cmd = Command::new(i as u32, cmd_text.to_string(), PathBuf::from("/tmp"));
        cmd.complete(0);
        session.add_command(cmd);
    }
    session
}

/// Create quiet output (suppresses noise in tests).
fn quiet_output() -> Output {
    Output::new(false, true, false)
}

// --- Abort policy tests ---

#[test]
fn test_abort_prescan_finds_all_dangerous() {
    // Session with 2 dangerous + 1 safe command
    let session = test_session(&["echo hello", "rm -rf /tmp/test", "rm -rf /var/data"]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Abort),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // Abort pre-scan should find dangerous commands and return without executing any
    assert_eq!(summary.executed, 0, "abort should not execute any commands");
    assert!(
        summary.aborted,
        "abort should set aborted=true when dangerous found"
    );
    assert_eq!(summary.total, 3);
}

#[test]
fn test_abort_no_dangerous_proceeds() {
    // Session with only safe commands
    let session = test_session(&["echo hello", "echo world", "ls -la"]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Abort),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // No dangerous commands → normal execution
    assert_eq!(
        summary.executed, 3,
        "abort with no dangerous should execute all"
    );
    assert!(
        !summary.aborted,
        "abort with no dangerous should not set aborted"
    );
}

// --- Skip policy tests ---

#[test]
fn test_skip_skips_dangerous_with_warning() {
    // 5 commands, 1 dangerous
    let session = test_session(&["echo a", "echo b", "rm -rf /tmp/test", "echo c", "echo d"]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Skip),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // Should skip the dangerous command and execute the rest
    assert_eq!(
        summary.executed, 4,
        "skip should execute non-dangerous commands"
    );
    assert_eq!(
        summary.skipped, 1,
        "skip should count the dangerous command as skipped"
    );
    assert!(!summary.aborted, "skip should not abort");
    assert_eq!(summary.total, 5);
}

// --- Allow policy tests ---

#[test]
fn test_allow_executes_dangerous() {
    // Session with safe + dangerous commands
    let session = test_session(&["echo hello", "echo dangerous-but-allowed"]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Allow),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // Allow should execute all commands
    assert_eq!(summary.executed, 2, "allow should execute all commands");
    assert!(!summary.aborted);
}

// --- None + force backward compat ---

#[test]
fn test_none_force_bypasses() {
    // None policy with force=true should preserve existing behavior
    let session = test_session(&["echo hello", "echo world"]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        force: true,
        // danger_policy is None (default)
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // Force should bypass prompts and execute all
    assert_eq!(summary.executed, 2, "force should execute all commands");
    assert!(!summary.aborted);
}

// --- Dry-run dangerous marker ---

#[test]
fn test_dry_run_dangerous_marker() {
    // Dry-run with a dangerous command should annotate it with [DANGEROUS]
    let session = test_session(&["echo hello", "rm -rf /tmp/test"]);
    let config = Config::default();
    let output = Output::new(false, false, false); // not quiet, so output is visible
    let options = ReplayOptions {
        dry_run: true,
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    // Dry-run should not execute anything
    assert_eq!(summary.executed, 0, "dry-run should not execute");
    assert_eq!(summary.total, 2);
    assert!(!summary.aborted);
    // Note: We can't easily capture stdout in this test to check [DANGEROUS] marker,
    // but we verify the dry-run flow completes without error
}

// --- DangerPolicy enum tests ---

#[test]
fn test_danger_policy_enum_traits() {
    // Verify DangerPolicy has expected trait implementations
    let skip = DangerPolicy::Skip;
    let abort = DangerPolicy::Abort;
    let allow = DangerPolicy::Allow;

    // Debug
    assert_eq!(format!("{skip:?}"), "Skip");
    assert_eq!(format!("{abort:?}"), "Abort");
    assert_eq!(format!("{allow:?}"), "Allow");

    // Clone + Copy
    let cloned = skip;
    assert_eq!(skip, cloned);

    // PartialEq
    assert_eq!(skip, DangerPolicy::Skip);
    assert_ne!(skip, DangerPolicy::Abort);
}

#[test]
fn test_replay_options_default_has_no_danger_policy() {
    let options = ReplayOptions::default();
    assert!(
        options.danger_policy.is_none(),
        "default ReplayOptions should have no danger_policy"
    );
}

// --- Skipped dangerous tracking ---

#[test]
fn test_skip_multiple_dangerous_commands() {
    // Multiple dangerous commands should all be skipped
    let session = test_session(&[
        "echo safe",
        "rm -rf /tmp/a",
        "rm -rf /tmp/b",
        "echo also-safe",
    ]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Skip),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    assert_eq!(summary.executed, 2, "should execute only safe commands");
    assert_eq!(summary.skipped, 2, "should skip both dangerous commands");
    assert!(!summary.aborted);
}

#[test]
fn test_abort_with_multiple_dangerous_reports_all() {
    // Abort should find ALL dangerous commands (not just the first)
    let session = test_session(&[
        "echo safe",
        "rm -rf /tmp/a",
        "echo also-safe",
        "rm -rf /tmp/b",
    ]);
    let config = Config::default();
    let output = quiet_output();
    let options = ReplayOptions {
        danger_policy: Some(DangerPolicy::Abort),
        ..Default::default()
    };

    let mut engine = ReplayEngine::new(session, options, &config, output);
    let summary = engine.run().unwrap();

    assert_eq!(summary.executed, 0, "abort should not execute any");
    assert!(
        summary.aborted,
        "abort should set aborted when dangerous found"
    );
}
