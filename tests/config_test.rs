//! Integration tests for config commands (CONF-01 through CONF-06).

mod common;

use common::TestEnv;
use rec::config::{ConfigLoader, ConfigSource};
use std::fs;

/// CONF-01: Config list returns all keys with source tracking.
#[test]
fn test_config_list_returns_all_keys() {
    let env = TestEnv::new();
    let loader = ConfigLoader::new(env.paths.clone());

    let values = loader.list_config().unwrap();

    // Should return all 8 known keys
    assert_eq!(values.len(), 8, "Expected 8 known config keys");

    // Each value should have key, value, and source
    for cv in &values {
        assert!(!cv.key.is_empty(), "Key should not be empty");
        assert!(
            !cv.value.is_empty(),
            "Value should not be empty for {}",
            cv.key
        );
    }

    // Verify known defaults exist
    let keys: Vec<&str> = values.iter().map(|v| v.key.as_str()).collect();
    assert!(keys.contains(&"general.editor"));
    assert!(keys.contains(&"safety.preset"));
    assert!(keys.contains(&"style.colors"));

    // All should be defaults when no config file exists
    for cv in &values {
        assert_eq!(
            cv.source,
            ConfigSource::Default,
            "Expected default source for {}",
            cv.key
        );
    }
}

/// CONF-02: Config get retrieves specific key value from file.
#[test]
fn test_config_get_retrieves_value() {
    let env = TestEnv::new();

    // Write a config TOML file
    fs::write(&env.paths.config_file, "[general]\neditor = \"vim\"\n").unwrap();

    let loader = ConfigLoader::new(env.paths.clone());
    let cv = loader.get_key("general.editor").unwrap();

    assert_eq!(cv.value, "vim");
    assert_eq!(cv.source, ConfigSource::File);
    assert_eq!(cv.file_value, Some("vim".to_string()));
}

/// CONF-03: Config set persists value to TOML file.
#[test]
fn test_config_set_persists_value() {
    let env = TestEnv::new();
    let loader = ConfigLoader::new(env.paths.clone());

    // Set a value
    loader.set_key("general.editor", "nano").unwrap();

    // Read file back and verify
    let contents = fs::read_to_string(&env.paths.config_file).unwrap();
    assert!(
        contents.contains("nano"),
        "File should contain 'nano': {contents}"
    );

    // Round-trip via get_key
    let cv = loader.get_key("general.editor").unwrap();
    assert_eq!(cv.value, "nano");
    assert_eq!(cv.source, ConfigSource::File);
}

/// CONF-04: Config get with invalid key returns error with fuzzy suggestion.
#[test]
fn test_config_get_invalid_key_returns_error() {
    let env = TestEnv::new();
    let loader = ConfigLoader::new(env.paths.clone());

    let result = loader.get_key("safety.presets");
    assert!(result.is_err(), "Should fail for invalid key");

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Unknown config key"),
        "Error should mention unknown key: {err}"
    );
    assert!(
        err.contains("Did you mean"),
        "Error should suggest similar key: {err}"
    );
}

/// CONF-05: Config path points within the temp directory.
#[test]
fn test_config_path_returns_config_file_path() {
    let env = TestEnv::new();

    // config_file should be a valid PathBuf within the temp directory
    let path = &env.paths.config_file;
    assert!(path.to_str().unwrap().contains("config.toml"));
    // Parent directory should exist (TestEnv calls ensure_dirs)
    assert!(path.parent().unwrap().exists());
}

/// CONF-06: Config set creates file if missing.
#[test]
fn test_config_set_creates_file_if_missing() {
    let env = TestEnv::new();

    // Remove config file if it exists
    let _ = fs::remove_file(&env.paths.config_file);
    assert!(
        !env.paths.config_file.exists(),
        "Config file should not exist yet"
    );

    let loader = ConfigLoader::new(env.paths.clone());
    loader.set_key("general.editor", "emacs").unwrap();

    // File should now exist
    assert!(
        env.paths.config_file.exists(),
        "Config file should be created"
    );

    // And contain the value
    let cv = loader.get_key("general.editor").unwrap();
    assert_eq!(cv.value, "emacs");
}
