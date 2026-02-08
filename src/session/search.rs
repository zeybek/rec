//! Full-text search across sessions with highlighting.
//!
//! Searches session names, tags, and command text using substring or regex
//! matching. Results are grouped by session with highlighted matches.

use regex::Regex;
use serde::Serialize;

use crate::cli::Output;
use crate::error::{RecError, Result};
use crate::session::normalize_tag;
use crate::storage::SessionStore;

/// A single command match within a session.
#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    /// Command index within the session (0-based)
    pub index: u32,
    /// The full command text
    pub command: String,
}

/// Search results for a single session.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// Session name
    pub session: String,
    /// Session ID
    pub id: String,
    /// Session tags
    pub tags: Vec<String>,
    /// Session start time
    pub started_at: f64,
    /// Matched commands
    pub matches: Vec<SearchMatch>,
    /// Whether the session name matched
    pub name_matched: bool,
    /// Whether any tag matched
    pub tag_matched: bool,
}

/// Search all sessions for a pattern in names, tags, and commands.
///
/// Supports both substring (case-insensitive) and regex matching.
/// Results are sorted by `started_at` descending (most recent first).
///
/// # Tag filter
/// If `tag_filter` is non-empty, only sessions with at least one matching
/// tag (normalized comparison) are searched.
///
/// # Output modes
/// - Human: grouped output with highlighted matches
/// - JSON: array of `SearchResult` objects
///
/// # Errors
/// Returns `RecError::Config` if `use_regex` is true and pattern is invalid regex.
pub fn search_sessions(
    store: &SessionStore,
    pattern: &str,
    use_regex: bool,
    tag_filter: &[String],
    json: bool,
    output: &Output,
) -> Result<()> {
    // Compile regex if needed
    let compiled_regex = if use_regex {
        Some(
            Regex::new(pattern)
                .map_err(|e| RecError::Config(format!("Invalid regex '{pattern}': {e}")))?,
        )
    } else {
        None
    };

    let ids = store.list()?;
    let mut results: Vec<SearchResult> = Vec::new();

    for id in &ids {
        let Ok(session) = store.load(id) else {
            continue;
        };

        // Apply tag filter FIRST (cheap check)
        if !tag_filter.is_empty() {
            let normalized_filter: Vec<String> =
                tag_filter.iter().map(|t| normalize_tag(t)).collect();
            let session_normalized: Vec<String> = session
                .header
                .tags
                .iter()
                .map(|t| normalize_tag(t))
                .collect();
            let has_matching_tag = session_normalized
                .iter()
                .any(|st| normalized_filter.iter().any(|ft| ft == st));
            if !has_matching_tag {
                continue;
            }
        }

        // Check session name
        let name_matched = matches_pattern(
            &session.header.name,
            pattern,
            use_regex,
            compiled_regex.as_ref(),
        );

        // Check tags
        let tag_matched = session
            .header
            .tags
            .iter()
            .any(|tag| matches_pattern(tag, pattern, use_regex, compiled_regex.as_ref()));

        // Check commands
        let mut command_matches: Vec<SearchMatch> = Vec::new();
        for cmd in &session.commands {
            if matches_pattern(&cmd.command, pattern, use_regex, compiled_regex.as_ref()) {
                command_matches.push(SearchMatch {
                    index: cmd.index,
                    command: cmd.command.clone(),
                });
            }
        }

        // If anything matched, add to results
        if name_matched || tag_matched || !command_matches.is_empty() {
            results.push(SearchResult {
                session: session.header.name.clone(),
                id: id.clone(),
                tags: session.header.tags.clone(),
                started_at: session.header.started_at,
                matches: command_matches,
                name_matched,
                tag_matched,
            });
        }
    }

    // Sort by started_at descending (most recent first)
    results.sort_by(|a, b| {
        b.started_at
            .partial_cmp(&a.started_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Output
    if results.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No matches found");
        }
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        let total_matches: usize = results.iter().map(|r| r.matches.len()).sum();
        let session_count = results.len();

        for (i, result) in results.iter().enumerate() {
            // Session header
            let match_count = result.matches.len();
            let header_text = if output.colors {
                format!(
                    "\x1b[1m{}\x1b[0m ({} match{})",
                    result.session,
                    match_count,
                    if match_count == 1 { "" } else { "es" }
                )
            } else {
                format!(
                    "{} ({} match{})",
                    result.session,
                    match_count,
                    if match_count == 1 { "" } else { "es" }
                )
            };
            println!("{header_text}");

            if result.name_matched {
                let highlighted = highlight_matches(
                    &result.session,
                    pattern,
                    use_regex,
                    compiled_regex.as_ref(),
                    output.colors,
                );
                println!("  name: {highlighted}");
            }

            if result.tag_matched {
                for tag in &result.tags {
                    if matches_pattern(tag, pattern, use_regex, compiled_regex.as_ref()) {
                        let highlighted = highlight_matches(
                            tag,
                            pattern,
                            use_regex,
                            compiled_regex.as_ref(),
                            output.colors,
                        );
                        println!("  tag: {highlighted}");
                    }
                }
            }

            for m in &result.matches {
                let highlighted = highlight_matches(
                    &m.command,
                    pattern,
                    use_regex,
                    compiled_regex.as_ref(),
                    output.colors,
                );
                println!("  {}. {}", m.index + 1, highlighted);
            }

            if i < results.len() - 1 {
                println!();
            }
        }

        println!();
        println!("{session_count} session(s), {total_matches} match(es)");
    }

    Ok(())
}

/// Check if text matches pattern (substring or regex).
fn matches_pattern(text: &str, pattern: &str, use_regex: bool, regex: Option<&Regex>) -> bool {
    if use_regex {
        regex.is_some_and(|r| r.is_match(text))
    } else {
        // Case-insensitive substring search
        text.to_lowercase().contains(&pattern.to_lowercase())
    }
}

/// Highlight pattern matches in text with bold ANSI codes.
///
/// - If `!colors`, returns text unchanged.
/// - For substring: replaces all case-insensitive occurrences with bold.
/// - For regex: wraps each regex match with bold ANSI codes.
fn highlight_matches(
    text: &str,
    pattern: &str,
    use_regex: bool,
    regex: Option<&Regex>,
    colors: bool,
) -> String {
    if !colors {
        return text.to_string();
    }

    if use_regex {
        if let Some(re) = regex {
            let mut result = String::new();
            let mut last_end = 0;
            for m in re.find_iter(text) {
                result.push_str(&text[last_end..m.start()]);
                result.push_str("\x1b[1m");
                result.push_str(m.as_str());
                result.push_str("\x1b[0m");
                last_end = m.end();
            }
            result.push_str(&text[last_end..]);
            result
        } else {
            text.to_string()
        }
    } else {
        // Case-insensitive substring highlighting
        let lower_text = text.to_lowercase();
        let lower_pattern = pattern.to_lowercase();
        let mut result = String::new();
        let mut last_end = 0;

        for (start, _) in lower_text.match_indices(&lower_pattern) {
            result.push_str(&text[last_end..start]);
            result.push_str("\x1b[1m");
            result.push_str(&text[start..start + pattern.len()]);
            result.push_str("\x1b[0m");
            last_end = start + pattern.len();
        }
        result.push_str(&text[last_end..]);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionStatus};
    use crate::storage::{Paths, SessionStore};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_store(temp_dir: &TempDir) -> SessionStore {
        let paths = Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        };
        SessionStore::new(paths)
    }

    fn create_session_with_commands(name: &str, tags: Vec<String>, commands: &[&str]) -> Session {
        let mut session = Session::new(name);
        session.header.tags = tags;
        for (i, cmd_text) in commands.iter().enumerate() {
            session.commands.push(Command::new(
                i as u32,
                cmd_text.to_string(),
                PathBuf::from("/tmp"),
            ));
        }
        session.complete(SessionStatus::Completed);
        session
    }

    // ── highlight_matches tests ───────────────────────────────────────

    #[test]
    fn test_highlight_matches_substring() {
        let result = highlight_matches("echo hello world", "hello", false, None, true);
        assert_eq!(result, "echo \x1b[1mhello\x1b[0m world");
    }

    #[test]
    fn test_highlight_matches_no_color() {
        let result = highlight_matches("echo hello world", "hello", false, None, false);
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_highlight_matches_regex() {
        let re = Regex::new("hel+o").unwrap();
        let result = highlight_matches("echo hello world", "hel+o", true, Some(&re), true);
        assert_eq!(result, "echo \x1b[1mhello\x1b[0m world");
    }

    #[test]
    fn test_highlight_matches_case_insensitive_substring() {
        let result = highlight_matches("echo Docker build", "docker", false, None, true);
        assert_eq!(result, "echo \x1b[1mDocker\x1b[0m build");
    }

    #[test]
    fn test_highlight_matches_multiple_occurrences() {
        let result = highlight_matches("echo echo echo", "echo", false, None, true);
        assert_eq!(
            result,
            "\x1b[1mecho\x1b[0m \x1b[1mecho\x1b[0m \x1b[1mecho\x1b[0m"
        );
    }

    // ── search tests ──────────────────────────────────────────────────

    #[test]
    fn test_search_finds_matching_commands() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands(
            "deploy-session",
            vec![],
            &[
                "docker build .",
                "docker push image",
                "kubectl apply -f deploy.yaml",
            ],
        );
        let s2 =
            create_session_with_commands("setup-session", vec![], &["npm install", "npm test"]);
        store.save(&s1).unwrap();
        store.save(&s2).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search for "docker" — should find deploy-session with 2 matches
        let result = search_sessions(&store, "docker", false, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_tag_filter() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands(
            "tagged-session",
            vec!["deploy".to_string()],
            &["echo hello", "echo world"],
        );
        let s2 = create_session_with_commands(
            "other-session",
            vec!["rust".to_string()],
            &["echo hello", "cargo build"],
        );
        store.save(&s1).unwrap();
        store.save(&s2).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search for "echo" with tag filter "deploy" — should only find tagged-session
        let result = search_sessions(
            &store,
            "echo",
            false,
            &["deploy".to_string()],
            false,
            &output,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_regex_mode() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands(
            "regex-test",
            vec![],
            &["docker build .", "docker-compose up", "npm install"],
        );
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Regex pattern matching "docker" followed by space or hyphen
        let result = search_sessions(&store, "docker[- ]", true, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands("my-session", vec![], &["echo hello"]);
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search for something that doesn't exist
        let result = search_sessions(&store, "zzzznonexistent", false, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_case_insensitive_substring() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands(
            "docker-session",
            vec![],
            &["Docker build .", "DOCKER push image"],
        );
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search with lowercase "docker" should match "Docker" and "DOCKER"
        let result = search_sessions(&store, "docker", false, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_invalid_regex() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Invalid regex should return RecError::Config
        let result = search_sessions(&store, "[invalid", true, &[], false, &output);
        assert!(result.is_err());
        match result {
            Err(RecError::Config(msg)) => {
                assert!(msg.contains("Invalid regex"));
            }
            _ => panic!("Expected RecError::Config"),
        }
    }

    #[test]
    fn test_search_matches_session_name() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands("deploy-production", vec![], &["echo unrelated"]);
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search for "deploy" should match the session name
        let result = search_sessions(&store, "deploy", false, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_matches_tags() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 = create_session_with_commands(
            "my-session",
            vec!["kubernetes".to_string(), "production".to_string()],
            &["echo unrelated"],
        );
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        // Search for "kubernetes" should match the tag
        let result = search_sessions(&store, "kubernetes", false, &[], false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_json_output() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let s1 =
            create_session_with_commands("json-session", vec!["test".to_string()], &["echo hello"]);
        store.save(&s1).unwrap();

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: true,
        };

        // JSON search should not error
        let result = search_sessions(&store, "hello", false, &[], true, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_empty_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = search_sessions(&store, "anything", false, &[], false, &output);
        assert!(result.is_ok());
    }
}
