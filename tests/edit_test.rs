//! Integration tests for session edit TOML round-trip (CONF-18, CONF-19).
//!
//! Tests `session_to_toml` and `toml_to_session` conversion logic.
//! Skips TTY/editor interaction — only tests pure conversion functions.

mod common;

use common::TestEnv;
use rec::session::edit::{session_to_toml, toml_to_session};

/// CONF-18: `session_to_toml` produces valid, parseable TOML containing session data.
#[test]
fn test_edit_session_to_toml_produces_valid_toml() {
    let env = TestEnv::new();
    let mut session =
        env.create_session_with_commands("my-edit-session", &["echo hello", "ls -la"]);
    session.header.tags = vec!["setup".to_string(), "docker".to_string()];

    let toml_str = session_to_toml(&session).unwrap();

    // Should contain comment header
    assert!(toml_str.contains("# Edit this file"));

    // Should contain session name
    assert!(toml_str.contains("my-edit-session"));

    // Should be parseable as TOML (strip comments)
    let content: String = toml_str
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let parsed: toml::Value = toml::from_str(&content).expect("TOML should be valid");

    // Verify key fields present
    assert_eq!(parsed["name"].as_str().unwrap(), "my-edit-session");
    let tags = parsed["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].as_str().unwrap(), "setup");

    let commands = parsed["commands"].as_array().unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0]["command"].as_str().unwrap(), "echo hello");
}

/// CONF-19: TOML round-trip preserves session ID, name, commands, and tags.
#[test]
fn test_edit_toml_round_trip_preserves_data() {
    let env = TestEnv::new();
    let mut session =
        env.create_session_with_commands("roundtrip-test", &["echo a", "echo b", "echo c"]);
    session.header.tags = vec!["tag1".to_string(), "tag2".to_string()];

    let original_id = session.header.id;
    let original_version = session.header.version;
    let original_started_at = session.header.started_at;

    let toml_str = session_to_toml(&session).unwrap();
    let restored = toml_to_session(&toml_str, &session).unwrap();

    // Non-editable fields preserved
    assert_eq!(restored.header.id, original_id);
    assert_eq!(restored.header.version, original_version);
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(restored.header.started_at, original_started_at);
    }

    // Editable fields preserved
    assert_eq!(restored.header.name, "roundtrip-test");
    assert_eq!(restored.header.tags, vec!["tag1", "tag2"]);
    assert_eq!(restored.commands.len(), 3);
    assert_eq!(restored.commands[0].command, "echo a");
    assert_eq!(restored.commands[1].command, "echo b");
    assert_eq!(restored.commands[2].command, "echo c");

    // Footer command count preserved
    assert_eq!(restored.footer.as_ref().unwrap().command_count, 3);
}

/// CONF-19: Editing a nonexistent session produces a non-zero exit code.
#[test]
fn test_edit_nonexistent_session_errors() {
    let env = TestEnv::new();

    // Set environment variables for isolation
    #[allow(deprecated)]
    let cmd = assert_cmd::Command::cargo_bin("rec")
        .expect("binary exists")
        .arg("edit")
        .arg("nonexistent-session-id")
        .env("REC_DATA_DIR", env.paths.data_dir.as_os_str())
        .env("REC_CONFIG_DIR", env.paths.config_dir.as_os_str())
        .env("REC_STATE_DIR", env.paths.state_dir.as_os_str())
        .assert()
        .failure();

    // Should fail with non-zero exit code (already asserted by .failure())
    let _ = cmd;
}
