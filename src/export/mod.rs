//! Multi-format session export.
//!
//! Converts recorded sessions to reusable formats: Bash scripts, Makefiles,
//! Markdown docs, GitHub Actions, GitLab CI, Dockerfiles, and `CircleCI` configs.

pub mod bash;
pub mod circleci;
pub mod dockerfile;
pub mod github_action;
pub mod gitlab_ci;
pub mod makefile;
pub mod markdown;
pub mod parameterize;

pub use bash::export_bash;
pub use circleci::export_circleci;
pub use dockerfile::export_dockerfile;
pub use github_action::export_github_action;
pub use gitlab_ci::export_gitlab_ci;
pub use makefile::export_makefile;
pub use markdown::export_markdown;
pub use parameterize::{Parameter, apply_parameters, detect_all_parameters};

/// Escape a string for safe use as a YAML scalar value.
///
/// Wraps the string in double quotes if it contains any characters
/// that could be misinterpreted by a YAML parser. Returns the
/// original string if no escaping is needed.
#[must_use]
pub fn escape_yaml(s: &str) -> String {
    let needs_quoting = s.is_empty()
        || s.starts_with(' ')
        || s.starts_with('#')
        || s.starts_with('{')
        || s.starts_with('[')
        || s.starts_with('*')
        || s.starts_with('&')
        || s.starts_with('!')
        || s.starts_with('|')
        || s.starts_with('>')
        || s.starts_with('%')
        || s.starts_with('@')
        || s.starts_with('`')
        || s.starts_with('?')
        || s.starts_with('-')
        || s.starts_with(',')
        || s.starts_with('\'')
        || s.starts_with('"')
        || s.contains(": ")
        || s.contains(" #")
        || s.contains('{')
        || s.contains('}')
        || s.contains('[')
        || s.contains(']');

    if needs_quoting {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// Truncate a command string for use as a CI step name.
///
/// If the string is longer than `max_len`, truncates and appends "...".
#[must_use]
pub fn truncate_step_name(command: &str, max_len: usize) -> String {
    if command.len() <= max_len {
        command.to_string()
    } else {
        format!("{}...", &command[..max_len])
    }
}

/// Escape dollar signs for Makefile compatibility.
///
/// In Makefiles, `$` is used for variable expansion, so literal `$` in
/// shell commands must be escaped as `$$`.
#[must_use]
pub fn escape_makefile(s: &str) -> String {
    s.replace('$', "$$")
}

/// Format a Unix timestamp (f64) as "YYYY-MM-DD HH:MM".
///
/// Returns "unknown" if the timestamp cannot be converted.
#[must_use]
pub fn format_timestamp(ts: f64) -> String {
    use chrono::{DateTime, Local};

    let secs = ts as i64;
    let nanos = ((ts - secs as f64) * 1_000_000_000.0) as u32;

    match DateTime::from_timestamp(secs, nanos) {
        Some(utc) => {
            let local = utc.with_timezone(&Local);
            local.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "unknown".to_string(),
    }
}

/// Format a duration in seconds as a human-readable string.
///
/// Examples: "45s", "2m 15s", "1h 3m 0s"
#[must_use]
pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_makefile() {
        assert_eq!(escape_makefile("echo $HOME"), "echo $$HOME");
        assert_eq!(escape_makefile("no dollars"), "no dollars");
        assert_eq!(escape_makefile("$A $B"), "$$A $$B");
        assert_eq!(escape_makefile(""), "");
    }

    #[test]
    fn test_format_timestamp() {
        // Known timestamp: 2026-01-01 00:00:00 UTC = 1767225600
        let result = format_timestamp(1767225600.0);
        // Should produce a valid date string (exact value depends on local timezone)
        assert!(result.contains("2026") || result.contains("2025")); // near year boundary
        assert_ne!(result, "unknown");
    }

    #[test]
    fn test_format_timestamp_unknown() {
        // Negative timestamps that chrono can't handle
        let result = format_timestamp(-1e18);
        assert_eq!(result, "unknown");
    }

    #[test]
    fn test_escape_yaml_plain_string() {
        assert_eq!(escape_yaml("echo hello"), "echo hello");
        assert_eq!(escape_yaml("npm install"), "npm install");
    }

    #[test]
    fn test_escape_yaml_colon_space() {
        assert_eq!(escape_yaml("echo: world"), "\"echo: world\"");
    }

    #[test]
    fn test_escape_yaml_hash_start() {
        assert_eq!(escape_yaml("#comment"), "\"#comment\"");
    }

    #[test]
    fn test_escape_yaml_brace() {
        assert_eq!(escape_yaml("echo {foo}"), "\"echo {foo}\"");
    }

    #[test]
    fn test_escape_yaml_empty_string() {
        assert_eq!(escape_yaml(""), "\"\"");
    }

    #[test]
    fn test_escape_yaml_double_quote_inside() {
        // String starting with " triggers quoting, internal " gets escaped
        assert_eq!(escape_yaml("\"hello\""), "\"\\\"hello\\\"\"");
    }

    #[test]
    fn test_escape_yaml_backslash_inside() {
        // Backslash in a string that needs quoting (contains `: `)
        assert_eq!(escape_yaml("key: val\\ue"), "\"key: val\\\\ue\"");
    }

    #[test]
    fn test_escape_yaml_no_quoting_needed() {
        // Simple strings with quotes/backslashes mid-string don't need quoting
        assert_eq!(escape_yaml("echo hello"), "echo hello");
    }

    #[test]
    fn test_truncate_step_name_short() {
        assert_eq!(truncate_step_name("echo hello", 60), "echo hello");
    }

    #[test]
    fn test_truncate_step_name_long() {
        let long = "a".repeat(61);
        let result = truncate_step_name(&long, 60);
        assert_eq!(result.len(), 63); // 60 + "..."
        assert!(result.ends_with("..."));
        assert!(result.starts_with(&"a".repeat(60)));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(5.0), "5s");
        assert_eq!(format_duration(65.0), "1m 5s");
        assert_eq!(format_duration(3665.0), "1h 1m 5s");
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(3600.0), "1h 0m 0s");
    }
}
