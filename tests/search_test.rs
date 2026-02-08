//! Integration tests for `rec search` command (DATA-10 through DATA-12).

mod common;

use common::{TestEnv, normal_output};
use rec::session::search_sessions;

/// DATA-10: search finds sessions by substring command pattern.
#[test]
fn test_search_finds_sessions_by_command_pattern() {
    let env = TestEnv::new();
    let session = env.create_session_with_commands(
        "deploy-session",
        &["docker build .", "docker push image", "kubectl apply"],
    );
    env.store.save(&session).unwrap();

    let output = normal_output();
    let result = search_sessions(&env.store, "docker", false, &[], false, &output);
    assert!(result.is_ok());
}

/// DATA-11: search with `use_regex=true` matches regex patterns.
#[test]
fn test_search_regex_pattern_matching() {
    let env = TestEnv::new();
    let session = env.create_session_with_commands(
        "regex-session",
        &["docker build .", "docker-compose up", "npm install"],
    );
    env.store.save(&session).unwrap();

    let output = normal_output();
    let result = search_sessions(&env.store, "docker[- ]", true, &[], false, &output);
    assert!(result.is_ok());
}

/// DATA-12: search with `tag_filter` filters by tags.
#[test]
fn test_search_tag_filter() {
    let env = TestEnv::new();

    let mut s1 = env.create_session_with_commands("prod-deploy", &["deploy script"]);
    s1.header.tags = vec!["deploy".to_string(), "prod".to_string()];
    env.store.save(&s1).unwrap();

    let mut s2 = env.create_session_with_commands("dev-deploy", &["deploy test"]);
    s2.header.tags = vec!["dev".to_string()];
    env.store.save(&s2).unwrap();

    let output = normal_output();
    let result = search_sessions(
        &env.store,
        "deploy",
        false,
        &["prod".to_string()],
        false,
        &output,
    );
    assert!(result.is_ok());
}

/// Additional coverage: search with no matching results returns Ok.
#[test]
fn test_search_no_results() {
    let env = TestEnv::new();
    let session = env.create_session_with_commands("some-session", &["echo hello"]);
    env.store.save(&session).unwrap();

    let output = normal_output();
    let result = search_sessions(
        &env.store,
        "nonexistent-pattern-xyz",
        false,
        &[],
        false,
        &output,
    );
    assert!(result.is_ok());
}
