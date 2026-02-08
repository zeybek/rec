//! TOML round-trip editing for sessions.
//!
//! Converts NDJSON sessions to human-readable TOML for editing in $EDITOR,
//! then parses changes back while preserving non-editable fields (id, version,
//! timestamps).

use crate::cli::Output;
use crate::error::{RecError, Result};
use crate::models::Session;
use crate::storage::{SessionStore, set_restrictive_permissions};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

/// Editable representation of a session for TOML serialization.
///
/// Contains only the fields that users should be able to modify.
/// Non-editable fields (id, version, `started_at`, `ended_at`, env) are
/// preserved from the original session during round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableSession {
    /// Human-readable session name
    pub name: String,
    /// User-defined tags
    pub tags: Vec<String>,
    /// Shell type
    pub shell: String,
    /// Operating system info
    pub os: String,
    /// Machine hostname
    pub hostname: String,
    /// Editable commands
    pub commands: Vec<EditableCommand>,
}

/// Editable representation of a single command.
///
/// Uses String for cwd instead of `PathBuf` for TOML friendliness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableCommand {
    /// The command text
    pub command: String,
    /// Working directory (as string for TOML)
    pub cwd: String,
    /// Exit code (None if command was still running)
    pub exit_code: Option<i32>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

/// Convert a Session to a TOML string for editing.
///
/// Prepends a comment block explaining what's editable.
///
/// # Errors
///
/// Returns an error if TOML serialization fails.
pub fn session_to_toml(session: &Session) -> Result<String> {
    let editable = EditableSession {
        name: session.header.name.clone(),
        tags: session.header.tags.clone(),
        shell: session.header.shell.clone(),
        os: session.header.os.clone(),
        hostname: session.header.hostname.clone(),
        commands: session
            .commands
            .iter()
            .map(|cmd| EditableCommand {
                command: cmd.command.clone(),
                cwd: cmd.cwd.to_string_lossy().to_string(),
                exit_code: cmd.exit_code,
                duration_ms: cmd.duration_ms,
            })
            .collect(),
    };

    let toml_str =
        toml::to_string_pretty(&editable).map_err(|e| RecError::Config(e.to_string()))?;

    Ok(format!(
        "# Edit this file to modify the session.\n\
         # Fields: name, tags, shell, os, hostname, and commands are editable.\n\
         # Session ID, timestamps, and version are preserved automatically.\n\
         # Save and close to apply changes. Quit without saving to cancel.\n\
         \n\
         {toml_str}"
    ))
}

/// Parse a TOML string back into a Session, merging with the original.
///
/// Preserves non-editable fields from the original session:
/// - header.version, header.id, `header.started_at`, header.env
/// - footer (`ended_at`, status)
/// - command timestamps (`started_at`, `ended_at`) for existing indices
///
/// Updates from the editable content:
/// - header.name, header.tags, header.shell, header.os, header.hostname
/// - commands (text, cwd, `exit_code`, `duration_ms`)
///
/// # Errors
///
/// Returns an error if TOML parsing fails.
pub fn toml_to_session(toml_str: &str, original: &Session) -> Result<Session> {
    let editable: EditableSession =
        toml::from_str(toml_str).map_err(|e| RecError::Config(e.to_string()))?;

    let mut merged = original.clone();
    merged.header.name = editable.name;
    merged.header.tags = editable.tags;
    merged.header.shell = editable.shell;
    merged.header.os = editable.os;
    merged.header.hostname = editable.hostname;

    // Rebuild commands, preserving timestamps from originals where possible
    merged.commands = editable
        .commands
        .iter()
        .enumerate()
        .map(|(i, ec)| {
            let orig = original.commands.get(i);
            crate::models::Command {
                index: i as u32,
                command: ec.command.clone(),
                cwd: PathBuf::from(&ec.cwd),
                started_at: orig.map_or(0.0, |o| o.started_at),
                ended_at: orig.and_then(|o| o.ended_at),
                exit_code: ec.exit_code,
                duration_ms: ec.duration_ms,
            }
        })
        .collect();

    // Update footer command count if footer exists
    if let Some(ref mut footer) = merged.footer {
        footer.command_count = merged.commands.len() as u32;
    }

    Ok(merged)
}

/// Launch an editor with the given content and return the edited result.
///
/// Returns `Ok(None)` if the user quit without saving (content unchanged or empty).
/// Returns `Ok(Some(content))` with the new content if changes were made.
///
/// Editor preference: $VISUAL -> $EDITOR -> vi
///
/// # Errors
///
/// Returns an error if the temp file cannot be created/read or the editor fails to launch.
pub fn launch_editor(content: &str) -> Result<Option<String>> {
    // Create temp file with .toml extension
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("rec-edit-{}.toml", std::process::id()));

    // Write content to temp file
    {
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
    }

    // Set restrictive permissions (0o600) to prevent other users from reading
    set_restrictive_permissions(&temp_path)?;

    // Detect editor
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    // Spawn editor
    let status = std::process::Command::new(&editor)
        .arg(&temp_path)
        .status()
        .map_err(|e| {
            // Clean up on error
            let _ = std::fs::remove_file(&temp_path);
            RecError::Io(e)
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_path);
        return Ok(None);
    }

    // Read result
    let new_content = std::fs::read_to_string(&temp_path)?;

    // Clean up
    let _ = std::fs::remove_file(&temp_path);

    // If content is empty or unchanged, treat as cancel
    if new_content.trim().is_empty() || new_content == content {
        return Ok(None);
    }

    Ok(Some(new_content))
}

/// Edit a session interactively using $EDITOR with TOML format.
///
/// Workflow:
/// 1. Create backup of the session file
/// 2. Convert session to TOML
/// 3. Open in editor
/// 4. Parse changes back, re-opening on syntax errors
/// 5. Save the updated session
///
/// # Errors
///
/// Returns an error if backup creation, TOML conversion, or session save fails.
pub fn edit_session(store: &SessionStore, session: &Session, output: &Output) -> Result<()> {
    // Create backup
    let session_id = session.id().to_string();
    let source = store.session_file_path(&session_id);
    let backup = source.with_extension("ndjson.bak");
    std::fs::copy(&source, &backup)?;

    // Convert to TOML
    let mut toml_content = session_to_toml(session)?;

    // Edit loop
    loop {
        let edited = launch_editor(&toml_content)?;

        match edited {
            None => {
                // No changes made — clean up backup (non-fatal if removal fails)
                let _ = std::fs::remove_file(&backup);
                output.info("Edit cancelled");
                return Ok(());
            }
            Some(new_content) => {
                // Try parsing
                match toml_to_session(&new_content, session) {
                    Ok(updated) => {
                        // Save may fail — if so, backup is preserved via ? propagation
                        store.save(&updated)?;
                        // Success — clean up backup (non-fatal if removal fails)
                        let _ = std::fs::remove_file(&backup);
                        output.success(&format!("Session '{}' updated", updated.name()));
                        return Ok(());
                    }
                    Err(e) => {
                        output.error(
                            "TOML parse error",
                            &e.to_string(),
                            None,
                            Some("Fix the syntax error and save again"),
                        );

                        // Ask to re-edit
                        let re_edit = dialoguer::Confirm::new()
                            .with_prompt("Re-edit?")
                            .default(true)
                            .interact()
                            .unwrap_or(false);

                        if !re_edit {
                            // User gave up — clean up backup (non-fatal if removal fails)
                            let _ = std::fs::remove_file(&backup);
                            output.info("Edit cancelled, original preserved");
                            return Ok(());
                        }
                        // Re-open with the broken content so user can fix
                        toml_content = new_content;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionStatus};
    use std::path::PathBuf;

    fn create_test_session() -> Session {
        let mut session = Session::new("test-edit");
        session.header.shell = "bash".to_string();
        session.header.os = "linux".to_string();
        session.header.hostname = "myhost".to_string();
        session.header.tags = vec!["setup".to_string(), "docker".to_string()];

        let mut cmd0 = Command::new(0, "echo hello".to_string(), PathBuf::from("/home/user"));
        cmd0.exit_code = Some(0);
        cmd0.duration_ms = Some(50);
        cmd0.ended_at = Some(cmd0.started_at + 0.05);
        session.commands.push(cmd0);

        let mut cmd1 = Command::new(1, "ls -la".to_string(), PathBuf::from("/tmp"));
        cmd1.exit_code = Some(0);
        cmd1.duration_ms = Some(120);
        cmd1.ended_at = Some(cmd1.started_at + 0.12);
        session.commands.push(cmd1);

        session.complete(SessionStatus::Completed);
        session
    }

    #[test]
    fn test_session_to_toml_produces_valid_toml() {
        let session = create_test_session();
        let toml_str = session_to_toml(&session).unwrap();

        // Should contain the comment header
        assert!(toml_str.contains("# Edit this file"));

        // Should be parseable as TOML
        let parsed: EditableSession = toml::from_str(
            toml_str
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
        )
        .expect("TOML should be valid");

        assert_eq!(parsed.name, "test-edit");
        assert_eq!(parsed.tags, vec!["setup", "docker"]);
        assert_eq!(parsed.commands.len(), 2);
        assert_eq!(parsed.commands[0].command, "echo hello");
    }

    #[test]
    fn test_toml_to_session_round_trip() {
        let session = create_test_session();
        let original_id = session.id();
        let original_version = session.header.version;
        let original_started_at = session.header.started_at;

        let toml_str = session_to_toml(&session).unwrap();
        let restored = toml_to_session(&toml_str, &session).unwrap();

        // Non-editable fields preserved
        assert_eq!(restored.id(), original_id);
        assert_eq!(restored.header.version, original_version);
        assert_eq!(restored.header.started_at, original_started_at);

        // Editable fields preserved
        assert_eq!(restored.name(), "test-edit");
        assert_eq!(restored.header.tags, vec!["setup", "docker"]);
        assert_eq!(restored.commands.len(), 2);
        assert_eq!(restored.commands[0].command, "echo hello");
        assert_eq!(restored.commands[0].cwd, PathBuf::from("/home/user"));
    }

    #[test]
    fn test_toml_to_session_with_modified_name() {
        let session = create_test_session();
        let original_id = session.id();

        let toml_str = session_to_toml(&session).unwrap();
        let modified = toml_str.replace("test-edit", "new-name");
        let restored = toml_to_session(&modified, &session).unwrap();

        assert_eq!(restored.name(), "new-name");
        assert_eq!(restored.id(), original_id); // ID preserved
    }

    #[test]
    fn test_toml_to_session_with_added_command() {
        let session = create_test_session();
        let mut toml_str = session_to_toml(&session).unwrap();

        // Add a new command
        toml_str.push_str(
            "\n\n[[commands]]\ncommand = \"pwd\"\ncwd = \"/var\"\nexit_code = 0\nduration_ms = 10\n",
        );

        let restored = toml_to_session(&toml_str, &session).unwrap();
        assert_eq!(restored.commands.len(), 3);
        assert_eq!(restored.commands[2].command, "pwd");
        assert_eq!(restored.commands[2].cwd, PathBuf::from("/var"));

        // Footer command count updated
        assert_eq!(restored.footer.as_ref().unwrap().command_count, 3);
    }

    #[test]
    fn test_toml_to_session_with_invalid_toml() {
        let session = create_test_session();
        let result = toml_to_session("this is not valid toml {{{{", &session);
        assert!(result.is_err());
    }

    #[test]
    fn test_toml_to_session_preserves_timestamps() {
        let session = create_test_session();
        let cmd0_started = session.commands[0].started_at;
        let cmd0_ended = session.commands[0].ended_at;

        let toml_str = session_to_toml(&session).unwrap();
        let restored = toml_to_session(&toml_str, &session).unwrap();

        // Command timestamps preserved from original
        assert_eq!(restored.commands[0].started_at, cmd0_started);
        assert_eq!(restored.commands[0].ended_at, cmd0_ended);
    }

    #[test]
    fn test_toml_to_session_preserves_env() {
        let session = create_test_session();
        let original_env = session.header.env.clone();

        let toml_str = session_to_toml(&session).unwrap();
        let restored = toml_to_session(&toml_str, &session).unwrap();

        assert_eq!(restored.header.env, original_env);
    }

    #[test]
    fn test_toml_to_session_preserves_footer() {
        let session = create_test_session();
        let original_ended_at = session.footer.as_ref().unwrap().ended_at;
        let original_status = session.footer.as_ref().unwrap().status;

        let toml_str = session_to_toml(&session).unwrap();
        let restored = toml_to_session(&toml_str, &session).unwrap();

        assert_eq!(
            restored.footer.as_ref().unwrap().ended_at,
            original_ended_at
        );
        assert_eq!(restored.footer.as_ref().unwrap().status, original_status);
    }
}
