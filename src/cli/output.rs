use std::io::IsTerminal;

use crate::config::load_config;
use crate::models::{ColorMode, SymbolMode, Verbosity};

/// Output handler with styled terminal output.
///
/// Respects configuration for colors, symbols, and verbosity levels.
/// Supports JSON output mode for scripting and machine consumption.
#[derive(Clone, Copy)]
pub struct Output {
    /// Whether to use ANSI colors
    pub colors: bool,
    /// Symbol mode (unicode or ascii)
    pub symbols: SymbolMode,
    /// Verbosity level
    pub verbosity: Verbosity,
    /// Whether to output JSON instead of human-readable text
    pub json: bool,
}

impl Output {
    /// Create output handler from CLI flags.
    ///
    /// Loads config to get style settings, then applies CLI flag overrides.
    /// `NO_COLOR` environment variable takes precedence over config.
    #[must_use]
    pub fn new(verbose: bool, quiet: bool, json: bool) -> Self {
        let config = load_config().unwrap_or_default();

        // Determine colors: NO_COLOR env var takes precedence
        let colors = match config.style.colors {
            ColorMode::Always => std::env::var("NO_COLOR").is_err(),
            ColorMode::Never => false,
            ColorMode::Auto => {
                std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
            }
        };

        // CLI flags override config verbosity
        // Priority: quiet > verbose > config
        let verbosity = if quiet {
            Verbosity::Quiet
        } else if verbose {
            Verbosity::Verbose
        } else {
            config.style.verbosity
        };

        Self {
            colors,
            symbols: config.style.symbols,
            verbosity,
            json,
        }
    }

    /// Whether ANSI color codes are enabled.
    ///
    /// Used by modules like `doctor::format_report()` to decide whether
    /// to include color escapes in their output.
    #[must_use]
    pub fn use_color(&self) -> bool {
        self.colors
    }

    /// Get success symbol based on symbol mode.
    #[must_use]
    pub fn success_symbol(&self) -> &'static str {
        match self.symbols {
            SymbolMode::Unicode => "\u{2713}", // checkmark
            SymbolMode::Ascii => "[OK]",
        }
    }

    /// Get error symbol based on symbol mode.
    #[must_use]
    pub fn error_symbol(&self) -> &'static str {
        match self.symbols {
            SymbolMode::Unicode => "\u{2717}", // X mark
            SymbolMode::Ascii => "[ERR]",
        }
    }

    /// Get info symbol based on symbol mode.
    #[must_use]
    pub fn info_symbol(&self) -> &'static str {
        match self.symbols {
            SymbolMode::Unicode => "\u{2192}", // right arrow
            SymbolMode::Ascii => "->",
        }
    }

    /// Get warning symbol based on symbol mode.
    #[must_use]
    pub fn warning_symbol(&self) -> &'static str {
        match self.symbols {
            SymbolMode::Unicode => "\u{26a0}", // warning sign
            SymbolMode::Ascii => "[WARN]",
        }
    }

    /// Print a success message.
    ///
    /// Suppressed in quiet mode. Uses green color when colors are enabled.
    /// Writes to stderr to avoid corrupting stdout data output.
    pub fn success(&self, message: &str) {
        if matches!(self.verbosity, Verbosity::Quiet) {
            return;
        }

        if self.json {
            eprintln!(
                r#"{{"status": "success", "message": "{}"}}"#,
                escape_json_string(message)
            );
        } else if self.colors {
            eprintln!("\x1b[32m{}\x1b[0m {}", self.success_symbol(), message);
        } else {
            eprintln!("{} {}", self.success_symbol(), message);
        }
    }

    /// Print an error message with optional cause and help text.
    ///
    /// Always printed (even in quiet mode). Uses red color when colors are enabled.
    pub fn error(&self, error_type: &str, message: &str, cause: Option<&str>, help: Option<&str>) {
        if self.json {
            let mut obj = format!(
                r#"{{"status": "error", "type": "{}", "message": "{}""#,
                escape_json_string(error_type),
                escape_json_string(message)
            );
            if let Some(c) = cause {
                use std::fmt::Write;
                let _ = write!(obj, r#", "cause": "{}""#, escape_json_string(c));
            }
            if let Some(h) = help {
                use std::fmt::Write;
                let _ = write!(obj, r#", "help": "{}""#, escape_json_string(h));
            }
            obj.push('}');
            eprintln!("{obj}");
        } else {
            let symbol = self.error_symbol();
            if self.colors {
                eprintln!("\x1b[31m{symbol} {error_type}\x1b[0m: {message}");
            } else {
                eprintln!("{symbol} {error_type}: {message}");
            }

            if let Some(c) = cause {
                eprintln!("  cause: {c}");
            }
            if let Some(h) = help {
                eprintln!("  help: {h}");
            }
        }
    }

    /// Print a warning message.
    ///
    /// Suppressed in quiet mode. Uses yellow color when colors are enabled.
    /// Writes to stderr to avoid corrupting stdout data output.
    pub fn warning(&self, message: &str) {
        if matches!(self.verbosity, Verbosity::Quiet) {
            return;
        }

        if self.json {
            eprintln!(
                r#"{{"level": "warning", "message": "{}"}}"#,
                escape_json_string(message)
            );
        } else if self.colors {
            eprintln!("\x1b[33m{}\x1b[0m {}", self.warning_symbol(), message);
        } else {
            eprintln!("{} {}", self.warning_symbol(), message);
        }
    }

    /// Print a debug message.
    ///
    /// Only printed in verbose mode. Uses gray color when colors are enabled.
    /// Writes to stderr to avoid corrupting stdout data output.
    pub fn debug(&self, message: &str) {
        if !matches!(self.verbosity, Verbosity::Verbose) {
            return;
        }

        if self.json {
            eprintln!(
                r#"{{"level": "debug", "message": "{}"}}"#,
                escape_json_string(message)
            );
        } else if self.colors {
            eprintln!("\x1b[90m[DEBUG] {message}\x1b[0m");
        } else {
            eprintln!("[DEBUG] {message}");
        }
    }

    /// Print an info message.
    ///
    /// Suppressed in quiet mode.
    /// Writes to stderr to avoid corrupting stdout data output.
    pub fn info(&self, message: &str) {
        if matches!(self.verbosity, Verbosity::Quiet) {
            return;
        }

        if self.json {
            eprintln!(
                r#"{{"level": "info", "message": "{}"}}"#,
                escape_json_string(message)
            );
        } else {
            eprintln!("{} {}", self.info_symbol(), message);
        }
    }
    /// Style text as an inline command (cyan with color, backtick-wrapped without).
    ///
    /// Used for displaying command suggestions like `rec start` or `rec stop`.
    #[must_use]
    pub fn style_command(&self, text: &str) -> String {
        if self.colors {
            format!("\x1b[36m{text}\x1b[0m")
        } else {
            format!("`{text}`")
        }
    }

    /// Style text as success (green with color, plain without).
    #[must_use]
    pub fn style_success(&self, text: &str) -> String {
        if self.colors {
            format!("\x1b[32m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// Style text as error (red with color, plain without).
    #[must_use]
    pub fn style_error(&self, text: &str) -> String {
        if self.colors {
            format!("\x1b[31m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// Check if verbose mode is enabled.
    #[must_use]
    pub fn is_verbose(&self) -> bool {
        matches!(self.verbosity, Verbosity::Verbose)
    }

    /// Check if quiet mode is enabled.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        matches!(self.verbosity, Verbosity::Quiet)
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new(false, false, false)
    }
}

/// Convenience function to print a success message with default settings.
pub fn print_success(message: &str) {
    Output::default().success(message);
}

/// Convenience function to print an error message with default settings.
pub fn print_error(error_type: &str, message: &str, cause: Option<&str>, help: Option<&str>) {
    Output::default().error(error_type, message, cause, help);
}

/// Escape special characters in a string for JSON output.
fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_default() {
        // Use explicit construction to avoid environment-dependent config loading.
        // Output::default() delegates to Output::new() which reads REC_VERBOSE etc.
        let output = Output {
            colors: false,
            symbols: SymbolMode::Unicode,
            verbosity: Verbosity::Normal,
            json: false,
        };
        assert!(!output.json);
        assert_eq!(output.verbosity, Verbosity::Normal);
    }

    #[test]
    fn test_output_symbols_unicode() {
        let output = Output {
            colors: false,
            symbols: SymbolMode::Unicode,
            verbosity: Verbosity::Normal,
            json: false,
        };

        assert_eq!(output.success_symbol(), "\u{2713}");
        assert_eq!(output.error_symbol(), "\u{2717}");
        assert_eq!(output.info_symbol(), "\u{2192}");
    }

    #[test]
    fn test_output_symbols_ascii() {
        let output = Output {
            colors: false,
            symbols: SymbolMode::Ascii,
            verbosity: Verbosity::Normal,
            json: false,
        };

        assert_eq!(output.success_symbol(), "[OK]");
        assert_eq!(output.error_symbol(), "[ERR]");
        assert_eq!(output.info_symbol(), "->");
    }

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello"), "hello");
        assert_eq!(escape_json_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_json_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_json_string("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_verbosity_quiet() {
        let output = Output::new(false, true, false);
        assert_eq!(output.verbosity, Verbosity::Quiet);
    }

    #[test]
    fn test_verbosity_verbose() {
        let output = Output::new(true, false, false);
        assert_eq!(output.verbosity, Verbosity::Verbose);
    }

    #[test]
    fn test_quiet_overrides_verbose() {
        // When both flags are set, quiet takes precedence
        let output = Output::new(true, true, false);
        assert_eq!(output.verbosity, Verbosity::Quiet);
    }
}
