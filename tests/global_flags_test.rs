//! Integration tests for global flags: --verbose, --quiet, --json (GLOB-01 through GLOB-03).
//!
//! Validates that verbose, quiet, and JSON output modes work across
//! list, show, and stats commands without errors.

mod common;

use common::{TestEnv, json_output, quiet_output, verbose_output};
use rec::session::list::list_sessions;
use rec::session::{compute_stats, format_stats, show_session};

// ===========================================================================
// GLOB-01: Verbose mode across commands
// ===========================================================================

#[test]
fn test_verbose_list_sessions() {
    let env = TestEnv::new();
    env.create_and_save_session("verbose-list");

    let output = verbose_output();
    let result = list_sessions(&env.store, &[], false, false, &output);
    assert!(
        result.is_ok(),
        "verbose list_sessions should succeed: {result:?}"
    );
}

#[test]
fn test_verbose_show_session() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("verbose-show");

    let output = verbose_output();
    let result = show_session(&session, None, false, &output);
    assert!(
        result.is_ok(),
        "verbose show_session should succeed: {result:?}"
    );
}

#[test]
fn test_verbose_stats() {
    let env = TestEnv::new();
    env.create_and_save_session("verbose-stats");

    let stats = compute_stats(&env.store).unwrap();
    let output = verbose_output();
    let result = format_stats(&stats, false, &output);
    assert!(
        result.is_ok(),
        "verbose format_stats should succeed: {result:?}"
    );
}

// ===========================================================================
// GLOB-02: Quiet mode across commands
// ===========================================================================

#[test]
fn test_quiet_list_sessions() {
    let env = TestEnv::new();
    env.create_and_save_session("quiet-list");

    let output = quiet_output();
    let result = list_sessions(&env.store, &[], false, false, &output);
    assert!(
        result.is_ok(),
        "quiet list_sessions should succeed: {result:?}"
    );
}

#[test]
fn test_quiet_show_session() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("quiet-show");

    let output = quiet_output();
    let result = show_session(&session, None, false, &output);
    assert!(
        result.is_ok(),
        "quiet show_session should succeed: {result:?}"
    );
}

#[test]
fn test_quiet_stats() {
    let env = TestEnv::new();
    env.create_and_save_session("quiet-stats");

    let stats = compute_stats(&env.store).unwrap();
    let output = quiet_output();
    let result = format_stats(&stats, false, &output);
    assert!(
        result.is_ok(),
        "quiet format_stats should succeed: {result:?}"
    );
}

// ===========================================================================
// GLOB-03: JSON mode across commands
// ===========================================================================

#[test]
fn test_json_list_sessions() {
    let env = TestEnv::new();
    env.create_and_save_session("json-list");

    let output = json_output();
    let result = list_sessions(&env.store, &[], false, true, &output);
    assert!(
        result.is_ok(),
        "json list_sessions should succeed: {result:?}"
    );
}

#[test]
fn test_json_show_session() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("json-show");

    let output = json_output();
    let result = show_session(&session, None, true, &output);
    assert!(
        result.is_ok(),
        "json show_session should succeed: {result:?}"
    );
}

#[test]
fn test_json_stats() {
    let env = TestEnv::new();
    env.create_and_save_session("json-stats");

    let stats = compute_stats(&env.store).unwrap();
    let output = json_output();
    let result = format_stats(&stats, true, &output);
    assert!(
        result.is_ok(),
        "json format_stats should succeed: {result:?}"
    );
}
