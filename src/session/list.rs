//! Session list command implementation.
//!
//! Lists all recorded sessions with formatting, tag filtering, pagination,
//! and JSON output support. Uses header-only loading for efficiency.

use std::io::IsTerminal;

use crate::cli::Output;
use crate::error::Result;
use crate::models::SessionFooter;
use crate::storage::SessionStore;

/// Summary info for a session in the list view.
struct SessionSummary {
    id: String,
    name: String,
    started_at: f64,
    tags: Vec<String>,
    footer: Option<SessionFooter>,
}

/// List sessions with optional tag filtering, pagination, and JSON output.
///
/// Loads only headers and footers for efficiency. Sessions are sorted by
/// `started_at` descending (most recent first).
///
/// # Tag filtering
/// - If `tags` is non-empty and `tag_all` is false: sessions matching ANY tag are shown
/// - If `tags` is non-empty and `tag_all` is true: sessions matching ALL tags are shown
///
/// # Pagination
/// In interactive terminals, shows 20 sessions at a time with a "Show more?" prompt.
///
/// # Errors
/// Returns an error if reading sessions from the store fails.
pub fn list_sessions(
    store: &SessionStore,
    tags: &[String],
    tag_all: bool,
    json: bool,
    output: &Output,
) -> Result<()> {
    let ids = store.list()?;

    // Load header+footer for all sessions
    let mut summaries: Vec<SessionSummary> = Vec::new();
    for id in &ids {
        if let Ok((header, footer)) = store.load_header_and_footer(id) {
            summaries.push(SessionSummary {
                id: id.clone(),
                name: header.name.clone(),
                started_at: header.started_at,
                tags: header.tags.clone(),
                footer,
            });
        }
    }

    // Sort by started_at descending (most recent first)
    summaries.sort_by(|a, b| {
        b.started_at
            .partial_cmp(&a.started_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply tag filter (normalized comparison for backward compat)
    if !tags.is_empty() {
        use crate::session::normalize_tag;
        let normalized_filter_tags: Vec<String> = tags.iter().map(|t| normalize_tag(t)).collect();
        summaries.retain(|s| {
            let session_normalized: Vec<String> = s.tags.iter().map(|t| normalize_tag(t)).collect();
            if tag_all {
                // ALL specified tags must be present (compare normalized forms)
                normalized_filter_tags
                    .iter()
                    .all(|ft| session_normalized.iter().any(|st| st == ft))
            } else {
                // ANY specified tag must be present (compare normalized forms)
                session_normalized
                    .iter()
                    .any(|st| normalized_filter_tags.iter().any(|ft| ft == st))
            }
        });
    }

    // Handle empty state
    if summaries.is_empty() {
        if json {
            println!("[]");
        } else if !tags.is_empty() {
            output.info("No sessions match tag filter");
        } else {
            output.info("No sessions found");
        }
        return Ok(());
    }

    // JSON output
    if json {
        let json_entries: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                let mut obj = serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "date": format_date(s.started_at),
                    "tags": s.tags,
                });

                if let Some(ref footer) = s.footer {
                    obj["command_count"] = serde_json::json!(footer.command_count);
                    let duration_secs = footer.ended_at - s.started_at;
                    obj["duration"] = serde_json::json!(format_duration(duration_secs));
                    obj["duration_seconds"] = serde_json::json!(duration_secs);
                } else {
                    obj["command_count"] = serde_json::json!(null);
                    obj["duration"] = serde_json::json!("active");
                }

                obj
            })
            .collect();

        println!(
            "{}",
            serde_json::to_string_pretty(&json_entries).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    // Table output with pagination
    let page_size = 20;
    let total = summaries.len();
    let interactive = std::io::stdout().is_terminal();

    // Print header line
    println!();
    println!(
        "  {:<30} {:<18} {:>5}  {:>10}  TAGS",
        "NAME", "DATE", "CMDS", "DURATION"
    );
    println!("  {}", "-".repeat(80));

    for (i, s) in summaries.iter().enumerate() {
        // Pagination: pause every page_size in interactive mode
        if interactive && i > 0 && i % page_size == 0 {
            let remaining = total - i;
            let show_more = dialoguer::Confirm::new()
                .with_prompt(format!("Show more? ({remaining} remaining)"))
                .default(true)
                .interact()
                .unwrap_or(false);
            if !show_more {
                break;
            }
        }

        let name = truncate(&s.name, 30);
        let date = format_date(s.started_at);

        let cmds = match &s.footer {
            Some(f) => format!("{}", f.command_count),
            None => "?".to_string(),
        };

        let duration = match &s.footer {
            Some(f) => format_duration(f.ended_at - s.started_at),
            None => "active".to_string(),
        };

        let tags = if s.tags.is_empty() {
            String::new()
        } else {
            truncate(&s.tags.join(", "), 20)
        };

        println!("  {name:<30} {date:<18} {cmds:>5}  {duration:>10}  {tags}");
    }

    println!();
    output.info(&format!("{total} session(s)"));
    println!();

    Ok(())
}

/// Format a Unix timestamp as YYYY-MM-DD HH:MM.
fn format_date(timestamp: f64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0).map_or_else(
        || "unknown".to_string(),
        |dt| {
            let local: chrono::DateTime<chrono::Local> = dt.into();
            local.format("%Y-%m-%d %H:%M").to_string()
        },
    )
}

/// Format a duration in seconds as a human-readable string.
fn format_duration(seconds: f64) -> String {
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

/// Truncate a string to `max_len`, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date() {
        // 2026-01-26 at some time
        let ts = 1737878400.0; // 2025-01-26T08:00:00Z
        let date = format_date(ts);
        assert!(date.starts_with("2025-01-26"));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(5.0), "5s");
        assert_eq!(format_duration(65.0), "1m 5s");
        assert_eq!(format_duration(3665.0), "1h 1m 5s");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world!", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn test_tag_filter_case_insensitive() {
        use crate::models::{Command, Session, SessionStatus};
        use crate::storage::{Paths, SessionStore};
        use std::path::PathBuf;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let paths = Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        };
        let store = SessionStore::new(paths);

        // Create session with mixed-case tags
        let mut s1 = Session::new("deploy-session");
        s1.header.tags = vec!["Deploy".to_string(), "SETUP".to_string()];
        s1.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            PathBuf::from("/tmp"),
        ));
        s1.complete(SessionStatus::Completed);
        store.save(&s1).unwrap();

        // Create session with lowercase tags
        let mut s2 = Session::new("other-session");
        s2.header.tags = vec!["rust".to_string()];
        s2.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            PathBuf::from("/tmp"),
        ));
        s2.complete(SessionStatus::Completed);
        store.save(&s2).unwrap();

        // Load summaries and apply tag filter — simulating list_sessions logic
        let ids = store.list().unwrap();
        let mut summaries: Vec<SessionSummary> = Vec::new();
        for id in &ids {
            if let Ok((header, footer)) = store.load_header_and_footer(id) {
                summaries.push(SessionSummary {
                    id: id.clone(),
                    name: header.name.clone(),
                    started_at: header.started_at,
                    tags: header.tags.clone(),
                    footer,
                });
            }
        }

        // Filter with lowercase "deploy" — should match "Deploy"
        let filter_tags = ["deploy".to_string()];
        {
            use crate::session::normalize_tag;
            let normalized_filter_tags: Vec<String> =
                filter_tags.iter().map(|t| normalize_tag(t)).collect();
            let filtered: Vec<&SessionSummary> = summaries
                .iter()
                .filter(|s| {
                    let session_normalized: Vec<String> =
                        s.tags.iter().map(|t| normalize_tag(t)).collect();
                    session_normalized
                        .iter()
                        .any(|st| normalized_filter_tags.iter().any(|ft| ft == st))
                })
                .collect();

            assert_eq!(filtered.len(), 1);
            assert_eq!(filtered[0].name, "deploy-session");
        }

        // Filter with uppercase "SETUP" — should match "SETUP"
        let filter_tags2 = ["setup".to_string()];
        {
            use crate::session::normalize_tag;
            let normalized_filter_tags: Vec<String> =
                filter_tags2.iter().map(|t| normalize_tag(t)).collect();
            let filtered: Vec<&SessionSummary> = summaries
                .iter()
                .filter(|s| {
                    let session_normalized: Vec<String> =
                        s.tags.iter().map(|t| normalize_tag(t)).collect();
                    session_normalized
                        .iter()
                        .any(|st| normalized_filter_tags.iter().any(|ft| ft == st))
                })
                .collect();

            assert_eq!(filtered.len(), 1);
            assert_eq!(filtered[0].name, "deploy-session");
        }
    }
}
