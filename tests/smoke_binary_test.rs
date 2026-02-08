#![allow(deprecated)]
//! Binary-level smoke tests using `assert_cmd`.
//!
//! These tests spawn the actual `rec` binary and verify:
//! - --help and --version flags work correctly
//! - Unknown commands produce non-zero exit codes
//! - Global flags (--verbose, --quiet, --json) are accepted
//! - Exit code contract: 0=success, 1=user error

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Helper: create a Command for the `rec` binary with isolated XDG dirs.
fn rec_cmd_isolated() -> (TempDir, Command) {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("rec").unwrap();
    cmd.env("XDG_DATA_HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("XDG_STATE_HOME", tmp.path());
    (tmp, cmd)
}

// ---------------------------------------------------------------------------
// GLOB-04: Help and version
// ---------------------------------------------------------------------------

#[test]
fn test_help_flag() {
    Command::cargo_bin("rec")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Record, replay, and export"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("rec")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_subcommand_help() {
    Command::cargo_bin("rec")
        .unwrap()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List"));
}

// ---------------------------------------------------------------------------
// GLOB-05: Unknown commands
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_command_exits_nonzero() {
    Command::cargo_bin("rec")
        .unwrap()
        .arg("notacommand")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

// ---------------------------------------------------------------------------
// GLOB-06+: Exit code smoke tests
// ---------------------------------------------------------------------------

#[test]
fn test_no_args_exits_cleanly() {
    // rec with no args shows help and exits 0
    Command::cargo_bin("rec").unwrap().assert().success();
}

#[test]
fn test_list_empty_exits_zero() {
    let (_tmp, mut cmd) = rec_cmd_isolated();
    cmd.arg("list").assert().success();
}

#[test]
fn test_show_missing_session_exits_one() {
    let (_tmp, mut cmd) = rec_cmd_isolated();
    cmd.args(["show", "nonexistent"]).assert().code(1);
}

// ---------------------------------------------------------------------------
// GLOB: Flag combinations
// ---------------------------------------------------------------------------

#[test]
fn test_verbose_flag_accepted() {
    let (_tmp, mut cmd) = rec_cmd_isolated();
    cmd.args(["--verbose", "list"]).assert().success();
}

#[test]
fn test_quiet_flag_accepted() {
    let (_tmp, mut cmd) = rec_cmd_isolated();
    cmd.args(["--quiet", "list"]).assert().success();
}

#[test]
fn test_json_flag_accepted() {
    let (_tmp, mut cmd) = rec_cmd_isolated();
    cmd.args(["--json", "list"]).assert().success();
}
