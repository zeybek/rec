//! Command-level diff between two sessions.
//!
//! Produces unified diff output comparing commands from two sessions,
//! with color support and JSON output mode.

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

use crate::cli::Output;
use crate::error::Result;
use crate::models::Session;

/// Summary of changes between two sessions.
#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    /// Number of commands present in both sessions
    pub equal: usize,
    /// Number of commands added in session 2
    pub added: usize,
    /// Number of commands removed from session 1
    pub removed: usize,
}

/// Produce a unified diff of commands between two sessions.
///
/// Compares the command text from `session1` and `session2` using
/// line-based diff via the `similar` crate.
///
/// # Output modes
/// - Human: unified diff with optional color (red for deletions,
///   green for insertions, cyan for hunk headers)
/// - JSON: structured change list with summary
///
/// # Edge cases
/// - Both sessions empty: prints "Both sessions have no commands"
/// - Identical sessions: prints "Sessions are identical"
///
/// # Errors
/// Returns an error only on I/O failures (unlikely for stdout).
pub fn diff_sessions(
    session1: &Session,
    session2: &Session,
    json: bool,
    output: &Output,
) -> Result<()> {
    let old_commands: Vec<&str> = session1
        .commands
        .iter()
        .map(|c| c.command.as_str())
        .collect();
    let new_commands: Vec<&str> = session2
        .commands
        .iter()
        .map(|c| c.command.as_str())
        .collect();

    let old_text = old_commands.join("\n");
    let new_text = new_commands.join("\n");

    // Handle both-empty edge case
    if old_commands.is_empty() && new_commands.is_empty() {
        if json {
            let output_json = serde_json::json!({
                "session1": { "name": session1.name(), "commands": 0 },
                "session2": { "name": session2.name(), "commands": 0 },
                "changes": [],
                "summary": { "equal": 0, "added": 0, "removed": 0 }
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&output_json).unwrap_or_else(|_| "{}".to_string())
            );
        } else {
            println!("Both sessions have no commands");
        }
        return Ok(());
    }

    // Add trailing newlines for proper diff behavior when non-empty
    let old_diffable = if old_text.is_empty() {
        old_text.clone()
    } else {
        format!("{old_text}\n")
    };
    let new_diffable = if new_text.is_empty() {
        new_text.clone()
    } else {
        format!("{new_text}\n")
    };

    let diff = TextDiff::from_lines(&old_diffable, &new_diffable);

    // Count changes
    let mut summary = DiffSummary {
        equal: 0,
        added: 0,
        removed: 0,
    };
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => summary.equal += 1,
            ChangeTag::Insert => summary.added += 1,
            ChangeTag::Delete => summary.removed += 1,
        }
    }

    if json {
        let mut changes: Vec<serde_json::Value> = Vec::new();
        for change in diff.iter_all_changes() {
            let change_type = match change.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            };
            let text = change.value().trim_end_matches('\n');
            if !text.is_empty() || !matches!(change.tag(), ChangeTag::Equal) {
                changes.push(serde_json::json!({
                    "type": change_type,
                    "command": text,
                }));
            }
        }

        let output_json = serde_json::json!({
            "session1": { "name": session1.name(), "commands": old_commands.len() },
            "session2": { "name": session2.name(), "commands": new_commands.len() },
            "changes": changes,
            "summary": {
                "equal": summary.equal,
                "added": summary.added,
                "removed": summary.removed,
            }
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output_json).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    // Check identical
    if summary.added == 0 && summary.removed == 0 {
        println!("Sessions are identical");
        println!("{} command(s) in common, 0 added, 0 removed", summary.equal);
        return Ok(());
    }

    // Human-readable unified diff output
    let unified = diff
        .unified_diff()
        .header(
            &format!("--- {}", session1.name()),
            &format!("+++ {}", session2.name()),
        )
        .context_radius(3)
        .to_string();

    if output.colors {
        // Post-process lines for color
        for line in unified.lines() {
            if line.starts_with("---") || line.starts_with("+++") {
                println!("\x1b[1m{line}\x1b[0m");
            } else if line.starts_with("@@") {
                println!("\x1b[36m{line}\x1b[0m");
            } else if line.starts_with('-') {
                println!("\x1b[31m{line}\x1b[0m");
            } else if line.starts_with('+') {
                println!("\x1b[32m{line}\x1b[0m");
            } else {
                println!("{line}");
            }
        }
    } else {
        print!("{unified}");
    }

    println!(
        "{} command(s) in common, {} added, {} removed",
        summary.equal, summary.added, summary.removed
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionStatus};
    use std::path::PathBuf;

    fn create_session(name: &str, commands: &[&str]) -> Session {
        let mut session = Session::new(name);
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

    #[test]
    fn test_diff_identical_sessions() {
        let s1 = create_session("session-a", &["echo hello", "ls -la", "pwd"]);
        let s2 = create_session("session-b", &["echo hello", "ls -la", "pwd"]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_completely_different() {
        let s1 = create_session("session-a", &["echo hello", "ls -la"]);
        let s2 = create_session("session-b", &["cargo build", "cargo test"]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_mixed_changes() {
        let s1 = create_session("session-a", &["echo hello", "ls -la", "pwd"]);
        let s2 = create_session("session-b", &["echo hello", "ls -la", "whoami"]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_empty_sessions() {
        let s1 = create_session("session-a", &[]);
        let s2 = create_session("session-b", &[]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_one_empty() {
        let s1 = create_session("session-a", &[]);
        let s2 = create_session("session-b", &["echo hello", "ls"]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_summary_counts() {
        let _s1 = create_session("session-a", &["echo hello", "ls -la", "pwd"]);
        let _s2 = create_session("session-b", &["echo hello", "ls -la", "whoami", "date"]);

        // We test DiffSummary logic directly
        let old_text = "echo hello\nls -la\npwd\n";
        let new_text = "echo hello\nls -la\nwhoami\ndate\n";
        let diff = TextDiff::from_lines(old_text, new_text);

        let mut summary = DiffSummary {
            equal: 0,
            added: 0,
            removed: 0,
        };
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => summary.equal += 1,
                ChangeTag::Insert => summary.added += 1,
                ChangeTag::Delete => summary.removed += 1,
            }
        }

        assert_eq!(summary.equal, 2); // "echo hello" and "ls -la"
        assert_eq!(summary.removed, 1); // "pwd"
        assert_eq!(summary.added, 2); // "whoami" and "date"
    }

    #[test]
    fn test_diff_json_output() {
        let s1 = create_session("session-a", &["echo hello", "ls"]);
        let s2 = create_session("session-b", &["echo hello", "pwd"]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: true,
        };

        let result = diff_sessions(&s1, &s2, true, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_colored_output() {
        let s1 = create_session("session-a", &["echo hello", "ls -la"]);
        let s2 = create_session("session-b", &["echo hello", "pwd"]);

        let output = Output {
            colors: true,
            symbols: crate::models::SymbolMode::Unicode,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_one_empty_reverse() {
        let s1 = create_session("session-a", &["echo hello", "ls"]);
        let s2 = create_session("session-b", &[]);

        let output = Output {
            colors: false,
            symbols: crate::models::SymbolMode::Ascii,
            verbosity: crate::models::Verbosity::Normal,
            json: false,
        };

        let result = diff_sessions(&s1, &s2, false, &output);
        assert!(result.is_ok());
    }
}
