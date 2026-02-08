//! Core import logic: read file, detect format, parse, and save session.

use crate::error::{RecError, Result};
use crate::models::{Command, Session, SessionStatus};
use crate::storage::SessionStore;
use std::path::Path;

use super::detect::{ImportFormat, detect_format, session_name_from_path};
use super::{bash_history, bash_script, fish_history, zsh_history};

/// Result of a successful import operation.
#[derive(Debug)]
pub struct ImportResult {
    /// Name of the created session
    pub session_name: String,
    /// Number of commands imported
    pub command_count: usize,
    /// Detected import format
    pub format: ImportFormat,
    /// Preview of first 5 commands
    pub preview_commands: Vec<String>,
}

/// Import a file into a rec session.
///
/// Detects format, parses commands, creates a completed Session,
/// saves it via `SessionStore`, and returns an `ImportResult` with preview.
///
/// # Errors
///
/// Returns an error if the file cannot be read, the format is unsupported,
/// or the session cannot be saved to the store.
pub fn import_file(
    path: &Path,
    name_override: Option<&str>,
    store: &SessionStore,
) -> Result<ImportResult> {
    let content = std::fs::read_to_string(path)?;
    let path_str = path.to_string_lossy();

    let format = detect_format(&path_str, &content)?;

    let commands: Vec<String> = match format {
        ImportFormat::BashScript => bash_script::parse_bash_script(&content),
        ImportFormat::BashHistory => bash_history::parse_bash_history(&content),
        ImportFormat::ZshHistory => zsh_history::parse_zsh_history(&content),
        ImportFormat::FishHistory => fish_history::parse_fish_history(&content),
    };

    if commands.is_empty() {
        return Err(RecError::InvalidSession(
            "No commands found in file".to_string(),
        ));
    }

    let session_name = match name_override {
        Some(name) => name.to_string(),
        None => session_name_from_path(path),
    };

    let command_count = commands.len();
    let preview_commands: Vec<String> = commands.iter().take(5).cloned().collect();

    // Create session with commands
    let mut session = Session::new(&session_name);
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

    for (i, cmd_text) in commands.iter().enumerate() {
        let cmd = Command::new(i as u32, cmd_text.clone(), cwd.clone());
        session.add_command(cmd);
    }
    session.complete(SessionStatus::Completed);

    store.save(&session)?;

    Ok(ImportResult {
        session_name,
        command_count,
        format,
        preview_commands,
    })
}
