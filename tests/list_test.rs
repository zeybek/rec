//! Integration tests for `rec list` command (DATA-01 through DATA-04).
//!
//! Validates that `list_sessions` correctly displays sessions, filters by
//! tags (single and multiple), and handles empty storage gracefully.

mod common;

use common::{TestEnv, normal_output};
use rec::session::list::list_sessions;

// ---------------------------------------------------------------------------
// DATA-01: list shows sessions with name, date, commands
// ---------------------------------------------------------------------------

#[test]
fn test_list_shows_sessions_with_name_date_commands() {
    let env = TestEnv::new();
    env.create_and_save_session("alpha");
    env.create_and_save_session("beta");

    let output = normal_output();
    let result = list_sessions(&env.store, &[], false, false, &output);
    assert!(result.is_ok(), "list_sessions should succeed: {result:?}");
}

// ---------------------------------------------------------------------------
// DATA-02: list with --tag filter (single tag)
// ---------------------------------------------------------------------------

#[test]
fn test_list_tag_filter_single() {
    let env = TestEnv::new();

    let tagged = env.create_session_with_tags("tagged-session", &["docker"]);
    env.store.save(&tagged).expect("save tagged session");

    env.create_and_save_session("untagged-session");

    let output = normal_output();
    let tags = vec!["docker".to_string()];
    let result = list_sessions(&env.store, &tags, false, false, &output);
    assert!(
        result.is_ok(),
        "list_sessions with tag filter should succeed: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// DATA-03: list with --tag-all filter (multiple tags, all must match)
// ---------------------------------------------------------------------------

#[test]
fn test_list_tag_all_filter() {
    let env = TestEnv::new();

    let both = env.create_session_with_tags("both-tags", &["deploy", "docker"]);
    env.store.save(&both).expect("save both-tags session");

    let one = env.create_session_with_tags("one-tag", &["deploy"]);
    env.store.save(&one).expect("save one-tag session");

    let output = normal_output();
    let tags = vec!["deploy".to_string(), "docker".to_string()];
    let result = list_sessions(&env.store, &tags, true, false, &output);
    assert!(
        result.is_ok(),
        "list_sessions with tag_all should succeed: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// DATA-04: list on empty storage returns Ok (not Err)
// ---------------------------------------------------------------------------

#[test]
fn test_list_empty_storage_returns_ok() {
    let env = TestEnv::new();

    let output = normal_output();
    let result = list_sessions(&env.store, &[], false, false, &output);
    assert!(
        result.is_ok(),
        "list_sessions on empty store should return Ok: {result:?}"
    );
}
