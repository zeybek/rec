//! Recording statistics aggregation.
//!
//! Computes aggregate statistics across all recorded sessions using
//! efficient header-only loading. Supports both human-readable and
//! JSON output formats.

use std::collections::HashMap;

use serde::Serialize;

use crate::cli::Output;
use crate::error::Result;
use crate::storage::SessionStore;

/// Tag usage count for statistics display.
#[derive(Debug, Clone, Serialize)]
pub struct TagCount {
    /// Tag name
    pub tag: String,
    /// Number of sessions using this tag
    pub count: u64,
}

/// Aggregate recording statistics.
#[derive(Debug, Serialize)]
pub struct RecStats {
    /// Total number of recorded sessions
    pub total_sessions: u64,
    /// Total commands across all sessions
    pub total_commands: u64,
    /// Total recording duration in seconds
    pub total_duration_secs: f64,
    /// Total storage used in bytes
    pub storage_bytes: u64,
    /// Most-used tags, sorted by count descending
    pub tag_counts: Vec<TagCount>,
}

impl RecStats {
    /// Average session length in seconds.
    ///
    /// Returns 0.0 if there are no sessions.
    #[must_use]
    pub fn average_session_length_secs(&self) -> f64 {
        if self.total_sessions == 0 {
            0.0
        } else {
            self.total_duration_secs / self.total_sessions as f64
        }
    }

    /// Format storage bytes as a human-readable string.
    ///
    /// Examples: "512 B", "1.2 KB", "3.4 MB", "5.6 GB"
    #[must_use]
    pub fn storage_human(&self) -> String {
        format_bytes(self.storage_bytes)
    }
}

/// Compute aggregate statistics across all sessions.
///
/// Uses `load_header_and_footer()` for efficient scanning — skips
/// full command deserialization. Collects:
/// - Session count
/// - Total commands and duration (from footer)
/// - Storage size (from file metadata)
/// - Tag frequency counts (top 10)
///
/// # Errors
///
/// Returns an error if the session list cannot be read.
pub fn compute_stats(store: &SessionStore) -> Result<RecStats> {
    let ids = store.list()?;

    let mut total_sessions: u64 = 0;
    let mut total_commands: u64 = 0;
    let mut total_duration_secs: f64 = 0.0;
    let mut storage_bytes: u64 = 0;
    let mut tag_map: HashMap<String, u64> = HashMap::new();

    for id in &ids {
        let Ok((header, footer)) = store.load_header_and_footer(id) else {
            continue; // skip corrupt sessions
        };

        total_sessions += 1;

        if let Some(ref footer) = footer {
            total_commands += u64::from(footer.command_count);
            let duration = footer.ended_at - header.started_at;
            if duration > 0.0 {
                total_duration_secs += duration;
            }
        }

        for tag in &header.tags {
            *tag_map.entry(tag.clone()).or_insert(0) += 1;
        }

        // Get file size from disk
        let path = store.session_file_path(id);
        if let Ok(metadata) = std::fs::metadata(&path) {
            storage_bytes += metadata.len();
        }
    }

    // Sort tags by count descending, take top 10
    let mut tag_counts: Vec<TagCount> = tag_map
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    tag_counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    tag_counts.truncate(10);

    Ok(RecStats {
        total_sessions,
        total_commands,
        total_duration_secs,
        storage_bytes,
        tag_counts,
    })
}

/// Display recording statistics in human-readable or JSON format.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn format_stats(stats: &RecStats, json: bool, output: &Output) -> Result<()> {
    if json {
        let json_obj = serde_json::json!({
            "total_sessions": stats.total_sessions,
            "total_commands": stats.total_commands,
            "average_session_length_seconds": stats.average_session_length_secs(),
            "total_duration_seconds": stats.total_duration_secs,
            "storage_bytes": stats.storage_bytes,
            "storage_human": stats.storage_human(),
            "most_used_tags": stats.tag_counts.iter().map(|tc| {
                serde_json::json!({ "tag": tc.tag, "count": tc.count })
            }).collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json_obj).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(());
    }

    // Handle empty state
    if stats.total_sessions == 0 {
        output.info("No sessions recorded yet");
        return Ok(());
    }

    // Human-readable aligned key-value output
    println!();
    println!("  Recording Statistics");
    println!();
    println!("  Sessions:        {}", stats.total_sessions);
    println!("  Commands:        {}", stats.total_commands);
    println!(
        "  Avg length:      {}",
        format_duration_stats(stats.average_session_length_secs())
    );
    println!(
        "  Total duration:  {}",
        format_duration_stats(stats.total_duration_secs)
    );
    println!("  Storage:         {}", stats.storage_human());

    if !stats.tag_counts.is_empty() {
        println!();
        println!("  Most used tags:");
        for tc in &stats.tag_counts {
            println!("    {:<20} {}", tc.tag, tc.count);
        }
    }

    println!();
    Ok(())
}

/// Format a duration in seconds as a human-readable string for stats.
///
/// Examples: "5s", "3m 24s", "1h 5m 30s"
#[must_use]
pub fn format_duration_stats(seconds: f64) -> String {
    if seconds <= 0.0 {
        return "0s".to_string();
    }

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

/// Format a byte count as a human-readable string.
///
/// Examples: "0 B", "512 B", "1.2 KB", "3.4 MB", "5.6 GB"
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let bytes_f = bytes as f64;

    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionStatus};
    use crate::storage::{Paths, SessionStore};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    fn create_session_with_tags(name: &str, tags: Vec<&str>, cmd_count: usize) -> Session {
        let mut session = Session::new(name);
        session.header.tags = tags.into_iter().map(String::from).collect();
        for i in 0..cmd_count {
            session.commands.push(Command::new(
                i as u32,
                format!("cmd-{i}"),
                PathBuf::from("/tmp"),
            ));
        }
        session.complete(SessionStatus::Completed);
        session
    }

    #[test]
    fn test_compute_stats_empty_store() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let stats = compute_stats(&store).unwrap();
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.total_commands, 0);
        assert_eq!(stats.total_duration_secs, 0.0);
        assert_eq!(stats.storage_bytes, 0);
        assert!(stats.tag_counts.is_empty());
    }

    #[test]
    fn test_compute_stats_with_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let s1 = create_session_with_tags("session-1", vec!["deploy"], 3);
        let s2 = create_session_with_tags("session-2", vec!["setup"], 5);
        let s3 = create_session_with_tags("session-3", vec!["deploy"], 2);

        store.save(&s1).unwrap();
        store.save(&s2).unwrap();
        store.save(&s3).unwrap();

        let stats = compute_stats(&store).unwrap();
        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.total_commands, 10); // 3 + 5 + 2
        assert!(stats.storage_bytes > 0);
    }

    #[test]
    fn test_compute_stats_tag_counting() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let s1 = create_session_with_tags("s1", vec!["deploy", "docker"], 1);
        let s2 = create_session_with_tags("s2", vec!["deploy", "setup"], 1);
        let s3 = create_session_with_tags("s3", vec!["deploy"], 1);
        let s4 = create_session_with_tags("s4", vec!["setup"], 1);

        store.save(&s1).unwrap();
        store.save(&s2).unwrap();
        store.save(&s3).unwrap();
        store.save(&s4).unwrap();

        let stats = compute_stats(&store).unwrap();

        // deploy: 3 times, setup: 2 times, docker: 1 time
        assert_eq!(stats.tag_counts.len(), 3);
        assert_eq!(stats.tag_counts[0].tag, "deploy");
        assert_eq!(stats.tag_counts[0].count, 3);
        assert_eq!(stats.tag_counts[1].tag, "setup");
        assert_eq!(stats.tag_counts[1].count, 2);
        assert_eq!(stats.tag_counts[2].tag, "docker");
        assert_eq!(stats.tag_counts[2].count, 1);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1258291), "1.2 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
        assert_eq!(format_bytes(6006636544), "5.6 GB");
    }

    #[test]
    fn test_average_session_length() {
        let stats = RecStats {
            total_sessions: 3,
            total_commands: 10,
            total_duration_secs: 612.0, // 3 sessions × avg 204s
            storage_bytes: 1024,
            tag_counts: vec![],
        };

        let avg = stats.average_session_length_secs();
        assert!((avg - 204.0).abs() < 0.001);
    }

    #[test]
    fn test_average_session_length_zero_sessions() {
        let stats = RecStats {
            total_sessions: 0,
            total_commands: 0,
            total_duration_secs: 0.0,
            storage_bytes: 0,
            tag_counts: vec![],
        };

        assert_eq!(stats.average_session_length_secs(), 0.0);
    }

    #[test]
    fn test_storage_human_formatting() {
        let stats = RecStats {
            total_sessions: 1,
            total_commands: 5,
            total_duration_secs: 60.0,
            storage_bytes: 1258291,
            tag_counts: vec![],
        };

        assert_eq!(stats.storage_human(), "1.2 MB");

        let stats_small = RecStats {
            total_sessions: 1,
            total_commands: 1,
            total_duration_secs: 5.0,
            storage_bytes: 512,
            tag_counts: vec![],
        };

        assert_eq!(stats_small.storage_human(), "512 B");
    }

    #[test]
    fn test_format_duration_stats() {
        assert_eq!(format_duration_stats(0.0), "0s");
        assert_eq!(format_duration_stats(5.0), "5s");
        assert_eq!(format_duration_stats(65.0), "1m 5s");
        assert_eq!(format_duration_stats(204.0), "3m 24s");
        assert_eq!(format_duration_stats(3665.0), "1h 1m 5s");
        assert_eq!(format_duration_stats(-1.0), "0s");
    }
}
