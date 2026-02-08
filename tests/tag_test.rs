//! Integration tests for `rec tag` command (MUT-09, MUT-10).

mod common;

use common::TestEnv;

// ---------------------------------------------------------------------------
// MUT-09: tag adds tags to session
// ---------------------------------------------------------------------------

#[test]
fn test_tag_adds_tags_to_session() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("tag-target");
    let session_id = session.header.id.to_string();

    // Action: add tags via store API
    let tags = vec!["deploy".to_string(), "production".to_string()];
    let result = env.store.add_tags(&session_id, tags);
    assert!(result.is_ok(), "add_tags should succeed: {result:?}");

    // Verify: returned tags contain both added tags
    let all_tags = result.unwrap();
    assert!(
        all_tags.contains(&"deploy".to_string()),
        "returned tags should contain 'deploy'"
    );
    assert!(
        all_tags.contains(&"production".to_string()),
        "returned tags should contain 'production'"
    );

    // Verify: tags persist when session is reloaded
    let loaded = env.store.load(&session_id).expect("session should load");
    assert!(
        loaded.header.tags.contains(&"deploy".to_string()),
        "persisted tags should contain 'deploy'"
    );
    assert!(
        loaded.header.tags.contains(&"production".to_string()),
        "persisted tags should contain 'production'"
    );
    assert_eq!(loaded.header.tags.len(), 2, "should have exactly 2 tags");
}

// ---------------------------------------------------------------------------
// MUT-09 (additional): adding same tag twice is idempotent
// ---------------------------------------------------------------------------

#[test]
fn test_tag_is_idempotent() {
    let env = TestEnv::new();
    let session = env.create_and_save_session("idempotent-tag");
    let session_id = session.header.id.to_string();

    // Add tag first time
    env.store
        .add_tags(&session_id, vec!["deploy".to_string()])
        .expect("first add_tags should succeed");

    // Add same tag second time
    let result = env.store.add_tags(&session_id, vec!["deploy".to_string()]);
    assert!(result.is_ok(), "second add_tags should succeed: {result:?}");

    // Verify: only one "deploy" tag exists (no duplicates)
    let all_tags = result.unwrap();
    let deploy_count = all_tags.iter().filter(|t| *t == "deploy").count();
    assert_eq!(deploy_count, 1, "should have exactly one 'deploy' tag");

    // Verify: persisted session also has no duplicates
    let loaded = env.store.load(&session_id).expect("session should load");
    let persisted_deploy_count = loaded.header.tags.iter().filter(|t| *t == "deploy").count();
    assert_eq!(
        persisted_deploy_count, 1,
        "persisted session should have exactly one 'deploy' tag"
    );
}

// ---------------------------------------------------------------------------
// MUT-10: tag nonexistent session returns exit code 1
// ---------------------------------------------------------------------------

#[test]
fn test_tag_nonexistent_session_exit_code_1() {
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary 'rec' should be findable by assert_cmd");

    cmd.args(["tag", "nonexistent-xyz", "tag1", "tag2"]);
    cmd.assert().failure().code(1);
}
