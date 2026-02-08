use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Color output mode for terminal display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// Auto-detect based on terminal capabilities and `NO_COLOR` env var
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

/// Symbol mode for terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SymbolMode {
    /// Use modern Unicode symbols (checkmark, cross, etc.)
    #[default]
    Unicode,
    /// Use ASCII-only symbols for compatibility
    Ascii,
}

/// Verbosity level for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    /// Only show errors
    Quiet,
    /// Normal output level
    #[default]
    Normal,
    /// Show debug information
    Verbose,
}

/// Safety preset for dangerous command detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SafetyPreset {
    /// Maximum protection - warns on many potentially dangerous commands
    Strict,
    /// Balanced protection (default)
    #[default]
    Moderate,
    /// Minimal protection - only warns on most dangerous commands
    Minimal,
}

/// General configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralConfig {
    /// Editor for editing sessions (falls back to $EDITOR)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,

    /// Default shell for replay (falls back to $SHELL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Custom storage path (falls back to XDG data directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<PathBuf>,
}

/// Style configuration for terminal output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StyleConfig {
    /// Color output mode
    #[serde(default)]
    pub colors: ColorMode,

    /// Symbol mode (unicode or ascii)
    #[serde(default)]
    pub symbols: SymbolMode,

    /// Output verbosity level
    #[serde(default)]
    pub verbosity: Verbosity,
}

/// Safety configuration for dangerous command detection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SafetyConfig {
    /// Built-in safety preset
    #[serde(default)]
    pub preset: SafetyPreset,

    /// Custom patterns to warn about (in addition to preset)
    #[serde(default)]
    pub custom_patterns: Vec<String>,
}

/// Complete application configuration.
///
/// Matches the TOML schema from CONTEXT.md:
/// ```toml
/// [general]
/// editor = "vim"
/// shell = "bash"
/// storage_path = ""
///
/// [style]
/// colors = "auto"
/// symbols = "unicode"
/// verbosity = "normal"
///
/// [safety]
/// preset = "moderate"
/// custom_patterns = ["kubectl delete", "terraform destroy"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// General settings
    #[serde(default)]
    pub general: GeneralConfig,

    /// Style settings for terminal output
    #[serde(default)]
    pub style: StyleConfig,

    /// Safety settings for command warnings
    #[serde(default)]
    pub safety: SafetyConfig,
}

impl Config {
    /// Merge another config into this one.
    ///
    /// Values from `other` override values in `self` where `other` has
    /// non-default values. Useful for layering config from multiple sources
    /// (defaults -> user config -> CLI args).
    pub fn merge_with(&mut self, other: Config) {
        // Merge general config
        if other.general.editor.is_some() {
            self.general.editor = other.general.editor;
        }
        if other.general.shell.is_some() {
            self.general.shell = other.general.shell;
        }
        if other.general.storage_path.is_some() {
            self.general.storage_path = other.general.storage_path;
        }

        // Merge style config (always override if explicitly set)
        self.style.colors = other.style.colors;
        self.style.symbols = other.style.symbols;
        self.style.verbosity = other.style.verbosity;

        // Merge safety config
        self.safety.preset = other.safety.preset;
        if !other.safety.custom_patterns.is_empty() {
            self.safety.custom_patterns = other.safety.custom_patterns;
        }
    }
}

/// Known configuration keys with their type descriptors.
///
/// Each entry is `(dot_path, type_descriptor)` where `type_descriptor` is:
/// - `"string"` — free-form string value
/// - `"enum:val1,val2,..."` — one of the listed values
/// - `"array:string"` — array of strings (cannot be set via `--set`)
pub const KNOWN_KEYS: &[(&str, &str)] = &[
    ("general.editor", "string"),
    ("general.shell", "string"),
    ("general.storage_path", "string"),
    ("style.colors", "enum:auto,always,never"),
    ("style.symbols", "enum:unicode,ascii"),
    ("style.verbosity", "enum:quiet,normal,verbose"),
    ("safety.preset", "enum:strict,moderate,minimal"),
    ("safety.custom_patterns", "array:string"),
];

/// Environment variable overrides for config keys.
///
/// Each entry is `(dot_path, env_var_name)`.
/// Special cases:
/// - `NO_COLOR`: any value → `style.colors = "never"`
/// - `REC_VERBOSE`: any value → `style.verbosity = "verbose"`
pub const ENV_VAR_MAPPING: &[(&str, &str)] = &[
    ("general.editor", "REC_EDITOR"),
    ("general.shell", "REC_SHELL"),
    ("general.storage_path", "REC_STORAGE_PATH"),
    ("style.colors", "NO_COLOR"),
    ("style.verbosity", "REC_VERBOSE"),
];

/// Validate that a config key is known.
///
/// Returns the type descriptor on success, or a helpful error message
/// with fuzzy suggestions on failure.
///
/// # Errors
///
/// Returns an error string if the key is not a known configuration key.
pub fn validate_key(key: &str) -> std::result::Result<&'static str, String> {
    KNOWN_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, type_info)| *type_info)
        .ok_or_else(|| {
            let suggestion = suggest_config_key(key);
            if suggestion.is_empty() {
                format!("Unknown config key '{key}'.")
            } else {
                format!("Unknown config key '{key}'. {suggestion}")
            }
        })
}

/// Look up the environment variable name for a config key.
///
/// Returns `Some(env_var_name)` if the key has an env override, `None` otherwise.
#[must_use]
pub fn env_var_for_key(key: &str) -> Option<&'static str> {
    ENV_VAR_MAPPING
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, env_var)| *env_var)
}

/// Suggest a similar config key using fuzzy matching.
///
/// Returns a suggestion string like `"Did you mean 'safety.preset'?"` or empty string.
#[must_use]
pub fn suggest_config_key(key: &str) -> String {
    use strsim::levenshtein;

    let mut best: Option<(&str, usize)> = None;
    for (known_key, _) in KNOWN_KEYS {
        let dist = levenshtein(key, known_key);
        if dist <= 3 {
            if let Some((_, best_dist)) = best {
                if dist < best_dist {
                    best = Some((known_key, dist));
                }
            } else {
                best = Some((known_key, dist));
            }
        }
    }
    match best {
        Some((suggestion, _)) => format!("Did you mean '{suggestion}'?"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();

        assert!(config.general.editor.is_none());
        assert!(config.general.shell.is_none());
        assert!(config.general.storage_path.is_none());
        assert_eq!(config.style.colors, ColorMode::Auto);
        assert_eq!(config.style.symbols, SymbolMode::Unicode);
        assert_eq!(config.style.verbosity, Verbosity::Normal);
        assert_eq!(config.safety.preset, SafetyPreset::Moderate);
        assert!(config.safety.custom_patterns.is_empty());
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = Config {
            general: GeneralConfig {
                editor: Some("vim".to_string()),
                shell: Some("bash".to_string()),
                storage_path: None,
            },
            style: StyleConfig {
                colors: ColorMode::Always,
                symbols: SymbolMode::Ascii,
                verbosity: Verbosity::Verbose,
            },
            safety: SafetyConfig {
                preset: SafetyPreset::Strict,
                custom_patterns: vec!["kubectl delete".to_string()],
            },
        };

        let toml_str = toml::to_string_pretty(&config).expect("Failed to serialize");
        assert!(toml_str.contains("editor = \"vim\""));
        assert!(toml_str.contains("colors = \"always\""));
        assert!(toml_str.contains("preset = \"strict\""));

        let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize");
        assert_eq!(deserialized.general.editor, Some("vim".to_string()));
        assert_eq!(deserialized.style.colors, ColorMode::Always);
    }

    #[test]
    fn test_config_merge() {
        let mut base = Config::default();
        let override_config = Config {
            general: GeneralConfig {
                editor: Some("nvim".to_string()),
                shell: None,
                storage_path: Some(PathBuf::from("/custom/path")),
            },
            style: StyleConfig {
                colors: ColorMode::Never,
                symbols: SymbolMode::Ascii,
                verbosity: Verbosity::Quiet,
            },
            safety: SafetyConfig {
                preset: SafetyPreset::Strict,
                custom_patterns: vec!["rm -rf".to_string()],
            },
        };

        base.merge_with(override_config);

        assert_eq!(base.general.editor, Some("nvim".to_string()));
        assert!(base.general.shell.is_none());
        assert_eq!(
            base.general.storage_path,
            Some(PathBuf::from("/custom/path"))
        );
        assert_eq!(base.style.colors, ColorMode::Never);
        assert_eq!(base.safety.preset, SafetyPreset::Strict);
        assert_eq!(base.safety.custom_patterns, vec!["rm -rf".to_string()]);
    }

    #[test]
    fn test_color_mode_serialization() {
        assert_eq!(serde_json::to_string(&ColorMode::Auto).unwrap(), "\"auto\"");
        assert_eq!(
            serde_json::to_string(&ColorMode::Always).unwrap(),
            "\"always\""
        );
        assert_eq!(
            serde_json::to_string(&ColorMode::Never).unwrap(),
            "\"never\""
        );
    }

    #[test]
    fn test_safety_preset_serialization() {
        assert_eq!(
            serde_json::to_string(&SafetyPreset::Strict).unwrap(),
            "\"strict\""
        );
        assert_eq!(
            serde_json::to_string(&SafetyPreset::Moderate).unwrap(),
            "\"moderate\""
        );
        assert_eq!(
            serde_json::to_string(&SafetyPreset::Minimal).unwrap(),
            "\"minimal\""
        );
    }

    #[test]
    fn test_validate_known_key() {
        assert_eq!(
            validate_key("safety.preset").unwrap(),
            "enum:strict,moderate,minimal"
        );
        assert_eq!(validate_key("general.editor").unwrap(), "string");
        assert_eq!(
            validate_key("safety.custom_patterns").unwrap(),
            "array:string"
        );
        assert_eq!(
            validate_key("style.colors").unwrap(),
            "enum:auto,always,never"
        );
    }

    #[test]
    fn test_validate_unknown_key_with_suggestion() {
        let err = validate_key("safety.presets").unwrap_err();
        assert!(
            err.contains("Unknown config key 'safety.presets'"),
            "got: {err}"
        );
        assert!(err.contains("Did you mean 'safety.preset'?"), "got: {err}");

        let err = validate_key("bogus.key").unwrap_err();
        assert!(err.contains("Unknown config key"), "got: {err}");
    }

    #[test]
    fn test_env_var_mapping() {
        assert_eq!(env_var_for_key("general.editor"), Some("REC_EDITOR"));
        assert_eq!(env_var_for_key("style.colors"), Some("NO_COLOR"));
        assert_eq!(env_var_for_key("safety.preset"), None);
    }
}
