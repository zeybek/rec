use crate::error::{RecError, Result};
use crate::models::config::{KNOWN_KEYS, env_var_for_key, validate_key};
use crate::models::{ColorMode, Config, Verbosity};
use crate::storage::Paths;
use std::fmt;
use std::fs;
use std::path::Path;
use toml_edit::DocumentMut;

/// Source of a configuration value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// Value comes from hardcoded defaults.
    Default,
    /// Value is explicitly set in the config file.
    File,
    /// Value is overridden by an environment variable.
    Env(String),
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::File => write!(f, "file"),
            ConfigSource::Env(var) => write!(f, "env:{var}"),
        }
    }
}

/// A resolved configuration value with source information.
#[derive(Debug, Clone)]
pub struct ConfigValue {
    /// The dot-path key (e.g., "safety.preset").
    pub key: String,
    /// The current effective value.
    pub value: String,
    /// The value from the config file, if explicitly set.
    pub file_value: Option<String>,
    /// Environment variable override, if any: `(var_name, var_value)`.
    pub env_override: Option<(String, String)>,
    /// Where the effective value comes from.
    pub source: ConfigSource,
}

/// Configuration loader with merge semantics.
///
/// Loads configuration from multiple sources with proper precedence:
/// 1. Hardcoded defaults (`Config::default()`)
/// 2. User config file (~/.config/rec/config.toml)
/// 3. Environment variables (REC_*, `NO_COLOR`)
/// 4. CLI flags (handled later in CLI layer)
pub struct ConfigLoader {
    paths: Paths,
}

impl ConfigLoader {
    /// Create a new `ConfigLoader` with the given paths.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// Load config with merge semantics: defaults < user config < env vars.
    ///
    /// Starts with default config, merges user config if it exists,
    /// then applies environment variable overrides.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but is invalid TOML.
    pub fn load(&self) -> Result<Config> {
        let mut config = Config::default();

        // Load user config if exists
        if self.paths.config_file.exists() {
            let user_config = self.load_from_file(&self.paths.config_file)?;
            config.merge_with(user_config);
        }

        // Apply environment variable overrides
        self.apply_env_overrides(&mut config);

        Ok(config)
    }

    /// Load config from a specific file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is invalid TOML.
    #[allow(clippy::unused_self)]
    fn load_from_file(&self, path: &Path) -> Result<Config> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Apply environment variable overrides to the config.
    ///
    /// Supported environment variables:
    /// - `REC_EDITOR`: Override editor setting
    /// - `REC_SHELL`: Override shell setting
    /// - `REC_STORAGE_PATH`: Override storage path
    /// - `NO_COLOR`: Disable colors (standard env var)
    /// - `REC_VERBOSE`: Enable verbose mode
    /// - `REC_QUIET`: Enable quiet mode
    #[allow(clippy::unused_self)]
    fn apply_env_overrides(&self, config: &mut Config) {
        // REC_EDITOR overrides editor
        if let Ok(editor) = std::env::var("REC_EDITOR") {
            config.general.editor = Some(editor);
        }

        // REC_SHELL overrides shell
        if let Ok(shell) = std::env::var("REC_SHELL") {
            config.general.shell = Some(shell);
        }

        // REC_STORAGE_PATH overrides storage path
        if let Ok(path) = std::env::var("REC_STORAGE_PATH") {
            config.general.storage_path = Some(path.into());
        }

        // NO_COLOR disables colors (standard env var)
        // https://no-color.org/
        if std::env::var("NO_COLOR").is_ok() {
            config.style.colors = ColorMode::Never;
        }

        // REC_VERBOSE enables verbose mode
        if std::env::var("REC_VERBOSE").is_ok() {
            config.style.verbosity = Verbosity::Verbose;
        }

        // REC_QUIET enables quiet mode (overrides REC_VERBOSE if both set)
        if std::env::var("REC_QUIET").is_ok() {
            config.style.verbosity = Verbosity::Quiet;
        }
    }

    /// Save config to the default location.
    ///
    /// Creates the config directory if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or file write fails.
    pub fn save(&self, config: &Config) -> Result<()> {
        self.paths.ensure_dirs()?;

        let contents = toml::to_string_pretty(config)
            .map_err(|e| RecError::Config(format!("Serialization error: {e}")))?;

        fs::write(&self.paths.config_file, contents)?;
        Ok(())
    }

    /// Create a default config file if it doesn't exist.
    ///
    /// Returns `true` if the file was created, `false` if it already existed.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or file write fails.
    pub fn create_default_if_missing(&self) -> Result<bool> {
        if self.paths.config_file.exists() {
            return Ok(false);
        }

        self.save(&Config::default())?;
        Ok(true)
    }

    /// Get a single config key with source information.
    ///
    /// Reads the TOML file to find the file value, checks env overrides,
    /// and returns the effective value with source annotation.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is unknown or the file cannot be read.
    pub fn get_key(&self, key: &str) -> Result<ConfigValue> {
        validate_key(key).map_err(RecError::Config)?;

        let file_value = self.read_file_value(key)?;
        let env_override = self.check_env_override(key);
        let default_value = self.default_value_for_key(key);

        let (value, source) = if let Some((ref var, ref val)) = env_override {
            (val.clone(), ConfigSource::Env(var.clone()))
        } else if let Some(ref fv) = file_value {
            (fv.clone(), ConfigSource::File)
        } else {
            (default_value, ConfigSource::Default)
        };

        Ok(ConfigValue {
            key: key.to_string(),
            value,
            file_value,
            env_override,
            source,
        })
    }

    /// Set a config key to a new value, preserving TOML comments and formatting.
    ///
    /// Uses `toml_edit::DocumentMut` for format-preserving writes. Creates
    /// sections if they don't exist. Validates the full config after modification
    /// by deserializing (round-trip check).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The key is unknown
    /// - The key is an array type (use `--edit` instead)
    /// - The value fails round-trip validation
    /// - File I/O fails
    pub fn set_key(&self, key: &str, value: &str) -> Result<()> {
        let type_info = validate_key(key).map_err(RecError::Config)?;

        // Reject array types
        if type_info.starts_with("array:") {
            return Err(RecError::Config(
                "Array values cannot be set via --set. Use 'rec config --edit' instead."
                    .to_string(),
            ));
        }

        // Read existing file or start fresh
        let contents = if self.paths.config_file.exists() {
            fs::read_to_string(&self.paths.config_file)?
        } else {
            String::new()
        };

        let mut doc: DocumentMut = contents
            .parse()
            .map_err(|e| RecError::Config(format!("Failed to parse config: {e}")))?;

        // Split "section.field" into parts
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(RecError::Config(format!("Invalid key format: '{key}'")));
        }
        let (section, field) = (parts[0], parts[1]);

        // Ensure section exists as a table
        if doc.get(section).is_none() {
            doc[section] = toml_edit::table();
        }

        // Set the value as a TOML string
        doc[section][field] = toml_edit::value(value);

        // Validate by deserializing full config (round-trip check)
        let new_contents = doc.to_string();
        toml::from_str::<Config>(&new_contents)?;

        // Ensure parent directories exist
        if let Some(parent) = self.paths.config_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write back (preserves comments!)
        fs::write(&self.paths.config_file, new_contents)?;
        Ok(())
    }

    /// List all config keys with their values and sources.
    ///
    /// For each known key, determines the effective value and whether it
    /// comes from the default, file, or an environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be read.
    pub fn list_config(&self) -> Result<Vec<ConfigValue>> {
        let mut values = Vec::new();
        for (key, _) in KNOWN_KEYS {
            values.push(self.get_key(key)?);
        }
        Ok(values)
    }

    /// Read a raw value from the TOML file for a given dot-path key.
    fn read_file_value(&self, key: &str) -> Result<Option<String>> {
        if !self.paths.config_file.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&self.paths.config_file)?;
        let doc: DocumentMut = contents
            .parse()
            .map_err(|e| RecError::Config(format!("Failed to parse config: {e}")))?;

        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        let value = doc
            .get(parts[0])
            .and_then(|section| section.get(parts[1]))
            .and_then(|item| item.as_value())
            .map(|v| {
                // Return the value without quotes for strings
                match v.as_str() {
                    Some(s) => s.to_string(),
                    None => v.to_string(),
                }
            });

        Ok(value)
    }

    /// Check if an environment variable overrides the given key.
    #[allow(clippy::unused_self)]
    fn check_env_override(&self, key: &str) -> Option<(String, String)> {
        let env_var = env_var_for_key(key)?;
        let env_value = std::env::var(env_var).ok()?;
        Some((env_var.to_string(), env_value))
    }

    /// Get the default value for a known key.
    #[allow(clippy::unused_self)]
    fn default_value_for_key(&self, key: &str) -> String {
        let config = Config::default();
        match key {
            "general.editor" => config
                .general
                .editor
                .unwrap_or_else(|| "(not set)".to_string()),
            "general.shell" => config
                .general
                .shell
                .unwrap_or_else(|| "(not set)".to_string()),
            "general.storage_path" => config
                .general
                .storage_path
                .map_or_else(|| "(not set)".to_string(), |p| p.display().to_string()),
            "style.colors" => format!("{:?}", config.style.colors).to_lowercase(),
            "style.symbols" => format!("{:?}", config.style.symbols).to_lowercase(),
            "style.verbosity" => format!("{:?}", config.style.verbosity).to_lowercase(),
            "safety.preset" => format!("{:?}", config.safety.preset).to_lowercase(),
            "safety.custom_patterns" => "[]".to_string(),
            _ => "(unknown)".to_string(),
        }
    }
}

/// Convenience function to load config with default paths.
///
/// Creates a Paths instance and loads config using `ConfigLoader`.
///
/// # Errors
///
/// Returns an error if config loading fails.
pub fn load_config() -> Result<Config> {
    let paths = Paths::new();
    let loader = ConfigLoader::new(paths);
    loader.load()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GeneralConfig, SafetyConfig, SafetyPreset, StyleConfig, SymbolMode};
    use tempfile::TempDir;

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    #[test]
    fn test_load_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let loader = ConfigLoader::new(paths);

        // No config file exists, should return defaults
        let config = loader.load().unwrap();

        assert!(config.general.editor.is_none());
        assert!(config.general.shell.is_none());
        assert_eq!(config.style.colors, ColorMode::Auto);
        assert_eq!(config.style.verbosity, Verbosity::Normal);
        assert_eq!(config.safety.preset, SafetyPreset::Moderate);
    }

    #[test]
    fn test_load_user_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        // Create config file
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            &paths.config_file,
            r#"
[general]
editor = "nvim"
shell = "zsh"

[style]
colors = "always"
symbols = "ascii"
verbosity = "verbose"

[safety]
preset = "strict"
custom_patterns = ["kubectl delete"]
"#,
        )
        .unwrap();

        let loader = ConfigLoader::new(paths);
        let config = loader.load().unwrap();

        assert_eq!(config.general.editor, Some("nvim".to_string()));
        assert_eq!(config.general.shell, Some("zsh".to_string()));
        assert_eq!(config.style.colors, ColorMode::Always);
        assert_eq!(config.style.symbols, SymbolMode::Ascii);
        assert_eq!(config.style.verbosity, Verbosity::Verbose);
        assert_eq!(config.safety.preset, SafetyPreset::Strict);
        assert_eq!(config.safety.custom_patterns, vec!["kubectl delete"]);
    }

    #[test]
    fn test_save_config() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let loader = ConfigLoader::new(paths.clone());

        let config = Config {
            general: GeneralConfig {
                editor: Some("vim".to_string()),
                shell: Some("bash".to_string()),
                storage_path: None,
            },
            style: StyleConfig {
                colors: ColorMode::Never,
                symbols: SymbolMode::Unicode,
                verbosity: Verbosity::Quiet,
            },
            safety: SafetyConfig {
                preset: SafetyPreset::Minimal,
                custom_patterns: vec!["rm -rf".to_string()],
            },
        };

        loader.save(&config).unwrap();

        // Verify file exists and is valid TOML
        let contents = fs::read_to_string(&paths.config_file).unwrap();
        assert!(contents.contains("editor = \"vim\""));
        assert!(contents.contains("shell = \"bash\""));
        assert!(contents.contains("colors = \"never\""));
        assert!(contents.contains("preset = \"minimal\""));
    }

    #[test]
    fn test_create_default_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let loader = ConfigLoader::new(paths.clone());

        // First call should create the file
        assert!(loader.create_default_if_missing().unwrap());
        assert!(paths.config_file.exists());

        // Second call should not create (already exists)
        assert!(!loader.create_default_if_missing().unwrap());
    }

    #[test]
    fn test_invalid_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        // Create invalid TOML file
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(&paths.config_file, "this is not { valid toml").unwrap();

        let loader = ConfigLoader::new(paths);
        let result = loader.load();

        assert!(result.is_err());
        match result {
            Err(RecError::Toml(e)) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("expected") || msg.contains("invalid"),
                    "TOML error should contain parse info: {msg}"
                );
            }
            _ => panic!("Expected Toml error"),
        }
    }

    #[test]
    fn test_merge_semantics() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        // Create partial config (only some fields)
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            &paths.config_file,
            r#"
[general]
editor = "nvim"
# shell is not set - should remain None (default)

[style]
colors = "always"
# symbols, verbosity use defaults
"#,
        )
        .unwrap();

        let loader = ConfigLoader::new(paths);
        let config = loader.load().unwrap();

        // User-specified values
        assert_eq!(config.general.editor, Some("nvim".to_string()));
        assert_eq!(config.style.colors, ColorMode::Always);

        // Default values preserved
        assert!(config.general.shell.is_none());
        // Note: merge_with overwrites style entirely, so defaults from file
        assert_eq!(config.style.symbols, SymbolMode::Unicode);
        assert_eq!(config.style.verbosity, Verbosity::Normal);
    }

    // Note: Environment variable tests need to be run with env vars set.
    // These are marked as ignore since they would affect other tests.
    // Run with: cargo test env_override -- --ignored

    #[test]
    #[ignore = "requires NO_COLOR env var to be set before process start"]
    fn test_no_color_env_override() {
        // Set NO_COLOR before running this test
        // NO_COLOR=1 cargo test test_no_color_env_override -- --ignored

        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let loader = ConfigLoader::new(paths);

        let config = loader.load().unwrap();

        // If NO_COLOR is set, colors should be Never
        if std::env::var("NO_COLOR").is_ok() {
            assert_eq!(config.style.colors, ColorMode::Never);
        }
    }

    #[test]
    fn test_load_config_convenience_function() {
        // This test just ensures the function compiles and runs
        // It uses the system's actual XDG paths
        let result = load_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_key_preserves_comments() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        // Create config with comments
        fs::create_dir_all(&paths.config_dir).unwrap();
        let original = r#"# My rec configuration
[general]
# The editor to use
editor = "vim"

[safety]
preset = "moderate"
"#;
        fs::write(&paths.config_file, original).unwrap();

        let loader = ConfigLoader::new(paths.clone());
        loader.set_key("safety.preset", "strict").unwrap();

        let result = fs::read_to_string(&paths.config_file).unwrap();
        // Comments must be preserved
        assert!(
            result.contains("# My rec configuration"),
            "Top-level comment lost: {result}"
        );
        assert!(
            result.contains("# The editor to use"),
            "Inline comment lost: {result}"
        );
        // Value must be updated
        assert!(result.contains("\"strict\""), "Value not updated: {result}");
        // Old value must be gone
        assert!(
            !result.contains("\"moderate\""),
            "Old value still present: {result}"
        );
    }

    #[test]
    fn test_set_key_creates_missing_section() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        // Start with empty file (no sections)
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(&paths.config_file, "").unwrap();

        let loader = ConfigLoader::new(paths.clone());
        loader.set_key("general.editor", "nvim").unwrap();

        let result = fs::read_to_string(&paths.config_file).unwrap();
        assert!(
            result.contains("[general]"),
            "Section not created: {result}"
        );
        assert!(result.contains("\"nvim\""), "Value not set: {result}");

        // Verify round-trip: the file is valid config
        let config: Config = toml::from_str(&result).unwrap();
        assert_eq!(config.general.editor, Some("nvim".to_string()));
    }

    #[test]
    fn test_set_key_rejects_invalid_value() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(&paths.config_file, "").unwrap();

        let loader = ConfigLoader::new(paths);
        // "invalid" is not a valid SafetyPreset enum value
        let result = loader.set_key("safety.preset", "invalid");
        assert!(result.is_err(), "Should reject invalid enum value");
    }

    #[test]
    fn test_set_key_rejects_array_type() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(&paths.config_file, "").unwrap();

        let loader = ConfigLoader::new(paths);
        let result = loader.set_key("safety.custom_patterns", "some value");
        assert!(result.is_err(), "Should reject array type");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Array values cannot be set via --set"),
            "Unexpected error: {err}"
        );
        assert!(err.contains("--edit"), "Should suggest --edit: {err}");
    }

    #[test]
    fn test_get_key_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            &paths.config_file,
            r#"
[safety]
preset = "strict"
"#,
        )
        .unwrap();

        let loader = ConfigLoader::new(paths);
        let cv = loader.get_key("safety.preset").unwrap();

        assert_eq!(cv.key, "safety.preset");
        assert_eq!(cv.value, "strict");
        assert_eq!(cv.file_value, Some("strict".to_string()));
        assert_eq!(cv.source, ConfigSource::File);
    }

    #[test]
    fn test_get_key_not_set() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        // No config file at all

        let loader = ConfigLoader::new(paths);
        let cv = loader.get_key("general.editor").unwrap();

        assert_eq!(cv.key, "general.editor");
        assert_eq!(cv.value, "(not set)");
        assert!(cv.file_value.is_none());
        assert_eq!(cv.source, ConfigSource::Default);
    }

    #[test]
    fn test_list_config_sources() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);

        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            &paths.config_file,
            r#"
[general]
editor = "nvim"

[safety]
preset = "strict"
"#,
        )
        .unwrap();

        let loader = ConfigLoader::new(paths);
        let values = loader.list_config().unwrap();

        // Should have all KNOWN_KEYS entries
        assert_eq!(values.len(), 8, "Should list all 8 known keys");

        // Check a file-set value
        let editor = values.iter().find(|v| v.key == "general.editor").unwrap();
        assert_eq!(editor.value, "nvim");
        assert_eq!(editor.source, ConfigSource::File);

        // Check a default value
        let symbols = values.iter().find(|v| v.key == "style.symbols").unwrap();
        assert_eq!(symbols.value, "unicode");
        assert_eq!(symbols.source, ConfigSource::Default);

        // Check another file-set value
        let preset = values.iter().find(|v| v.key == "safety.preset").unwrap();
        assert_eq!(preset.value, "strict");
        assert_eq!(preset.source, ConfigSource::File);
    }
}
