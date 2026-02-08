//! Tags listing command implementation.
//!
//! Lists all tags across all sessions with session counts, sorted by
//! count descending (most used first), then alphabetically for ties.

use crate::cli::Output;
use crate::error::Result;
use crate::storage::SessionStore;
use std::collections::HashMap;

/// List all tags with session counts.
///
/// Loads headers for all sessions and collects tag counts. Output is sorted
/// by count descending, then alphabetically for ties.
///
/// # JSON mode
/// Outputs a JSON array of `{tag, count}` objects.
///
/// # Normal mode
/// Prints each tag with its session count.
///
/// # Errors
/// Returns an error if reading sessions from the store fails.
pub fn list_tags(store: &SessionStore, json: bool, output: &Output) -> Result<()> {
    let ids = store.list()?;

    // Collect tag counts
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for id in &ids {
        if let Ok((header, _footer)) = store.load_header_and_footer(id) {
            for tag in &header.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }

    // Sort by count descending, then alphabetically for ties
    let mut sorted_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
    sorted_tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Handle empty state
    if sorted_tags.is_empty() {
        if json {
            println!("[]");
        } else {
            output.info("No tags found. Add tags with: rec tag <session> <tag>");
        }
        return Ok(());
    }

    // JSON output
    if json {
        let json_entries: Vec<serde_json::Value> = sorted_tags
            .iter()
            .map(|(tag, count)| {
                serde_json::json!({
                    "tag": tag,
                    "count": count,
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::to_string_pretty(&json_entries).unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    // Normal output
    println!();
    for (tag, count) in &sorted_tags {
        let label = if *count == 1 { "session" } else { "sessions" };
        println!("  {tag:<20} ({count} {label})");
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionStatus};
    use crate::storage::Paths;
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

    fn create_tagged_session(name: &str, tags: Vec<String>) -> Session {
        let mut session = Session::new(name);
        session.header.tags = tags;
        session.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            PathBuf::from("/tmp"),
        ));
        session.complete(SessionStatus::Completed);
        session
    }

    #[test]
    fn test_tag_counting() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Create sessions with tags
        let s1 = create_tagged_session("s1", vec!["deploy".into(), "rust".into()]);
        let s2 = create_tagged_session("s2", vec!["deploy".into(), "setup".into()]);
        let s3 = create_tagged_session("s3", vec!["rust".into()]);
        store.save(&s1).unwrap();
        store.save(&s2).unwrap();
        store.save(&s3).unwrap();

        // Collect counts manually
        let ids = store.list().unwrap();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for id in &ids {
            if let Ok((header, _)) = store.load_header_and_footer(id) {
                for tag in &header.tags {
                    *tag_counts.entry(tag.clone()).or_insert(0) += 1;
                }
            }
        }

        assert_eq!(tag_counts.get("deploy"), Some(&2));
        assert_eq!(tag_counts.get("rust"), Some(&2));
        assert_eq!(tag_counts.get("setup"), Some(&1));
    }

    #[test]
    fn test_empty_tags() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Session with no tags
        let s = create_tagged_session("no-tags", vec![]);
        store.save(&s).unwrap();

        let ids = store.list().unwrap();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for id in &ids {
            if let Ok((header, _)) = store.load_header_and_footer(id) {
                for tag in &header.tags {
                    *tag_counts.entry(tag.clone()).or_insert(0) += 1;
                }
            }
        }

        assert!(tag_counts.is_empty());
    }
}
