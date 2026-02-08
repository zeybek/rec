//! Integration tests for `rec alias` command (MUT-11 through MUT-14).

mod common;

use common::TestEnv;
use rec::session::resolve_session_with_alias;

// ---------------------------------------------------------------------------
// MUT-11: alias creates mapping to session
// ---------------------------------------------------------------------------

#[test]
fn test_alias_creates_mapping_to_session() {
    let env = TestEnv::new();

    // Note: AliasStore::set does NOT validate target exists (by design)
    let result = env.alias_store.set("deploy", "my-deploy-session");
    assert!(result.is_ok(), "alias set should succeed: {result:?}");

    // Verify: lookup returns the target session name
    let target = env.alias_store.get("deploy").unwrap();
    assert_eq!(
        target,
        Some("my-deploy-session".to_string()),
        "alias should resolve to target session"
    );
}

// ---------------------------------------------------------------------------
// MUT-12: alias list shows all aliases
// ---------------------------------------------------------------------------

#[test]
fn test_alias_list_shows_all_aliases() {
    let env = TestEnv::new();

    // Create multiple aliases
    env.alias_store.set("deploy", "deploy-session").unwrap();
    env.alias_store.set("staging", "staging-session").unwrap();

    // List all aliases
    let aliases = env.alias_store.list().unwrap();
    assert_eq!(aliases.len(), 2, "should have 2 aliases");

    // Note: list is sorted alphabetically by alias name
    assert_eq!(aliases[0].0, "deploy", "first alias should be 'deploy'");
    assert_eq!(
        aliases[0].1, "deploy-session",
        "deploy should map to deploy-session"
    );
    assert_eq!(aliases[1].0, "staging", "second alias should be 'staging'");
    assert_eq!(
        aliases[1].1, "staging-session",
        "staging should map to staging-session"
    );
}

// ---------------------------------------------------------------------------
// MUT-13: alias remove deletes alias
// ---------------------------------------------------------------------------

#[test]
fn test_alias_remove_deletes_alias() {
    let env = TestEnv::new();

    // Create alias
    env.alias_store.set("temp", "temp-session").unwrap();

    // Verify alias exists
    let exists = env.alias_store.get("temp").unwrap();
    assert!(exists.is_some(), "alias should exist before removal");

    // Remove alias
    let removed = env.alias_store.remove("temp").unwrap();
    assert!(removed, "remove should return true (found and removed)");

    // Verify alias no longer exists
    let lookup = env.alias_store.get("temp").unwrap();
    assert!(lookup.is_none(), "alias should not exist after removal");
}

// ---------------------------------------------------------------------------
// MUT-14: alias detects dangling after session deletion
// ---------------------------------------------------------------------------

#[test]
fn test_alias_detects_dangling_after_session_deletion() {
    let env = TestEnv::new();

    // Create and save a real session
    let session = env.create_and_save_session("target-session");

    // Create alias pointing to the session
    env.alias_store.set("shortcut", "target-session").unwrap();

    // Verify: resolve_session_with_alias succeeds while session exists
    let resolved = resolve_session_with_alias(
        &env.store,
        &env.alias_store,
        "shortcut",
        false, // non-interactive mode
    );
    assert!(
        resolved.is_ok(),
        "alias should resolve while target exists: {resolved:?}"
    );

    // Delete the target session
    env.store
        .delete(&session.header.id.to_string())
        .expect("delete should succeed");

    // Now alias resolution should fail (dangling alias detected)
    let dangling = resolve_session_with_alias(
        &env.store,
        &env.alias_store,
        "shortcut",
        false, // non-interactive mode
    );
    assert!(
        dangling.is_err(),
        "alias should fail to resolve after target deletion (dangling)"
    );
}
