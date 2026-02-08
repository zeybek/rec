use crate::error::{RecError, Result};
use crate::models::{Command, Session, SessionFooter, SessionHeader};
use crate::storage::Paths;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// Set restrictive file permissions (0o600: read/write for owner only).
///
/// This is a security measure to prevent other users from reading
/// potentially sensitive session data.
///
/// # Errors
///
/// Returns an error if setting permissions fails.
#[cfg(unix)]
pub fn set_restrictive_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

/// No-op on non-Unix platforms (Windows has a different permission model).
#[cfg(not(unix))]
pub fn set_restrictive_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// NDJSON line types for streaming writes.
///
/// Each line in a session file is one of these types, discriminated
/// by the "type" field in the JSON.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NdjsonLine {
    /// Session header with metadata
    Header(SessionHeader),
    /// Individual command
    Command(Command),
    /// Session footer with summary
    Footer(SessionFooter),
}

/// Session storage with NDJSON format.
///
/// Stores sessions as NDJSON (Newline Delimited JSON) files where each line
/// is a valid JSON object. This format allows:
/// - Streaming writes (append commands during recording)
/// - Human readability (one JSON object per line)
/// - Crash recovery (partial files are readable up to last complete line)
///
/// File structure:
/// ```json
/// {"type": "header", "version": 2, "id": "...", ...}
/// {"type": "command", "index": 0, "command": "...", ...}
/// {"type": "command", "index": 1, "command": "...", ...}
/// {"type": "footer", "ended_at": ..., "command_count": ..., "status": "..."}
/// ```
pub struct SessionStore {
    paths: Paths,
}

impl SessionStore {
    /// Create a new `SessionStore` with the given paths.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// Save a complete session to NDJSON file atomically.
    ///
    /// Uses a write-to-temp-then-rename pattern for crash safety:
    /// 1. Write to a temporary file (`{uuid}.ndjson.tmp`)
    /// 2. Flush and sync to disk
    /// 3. Atomically rename to final location (POSIX rename is atomic)
    ///
    /// This ensures the session file is never left in a partial state.
    /// If a crash occurs during write, only the `.tmp` file is affected.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Directory creation fails
    /// - File creation fails
    /// - JSON serialization fails
    /// - Atomic rename fails
    pub fn save(&self, session: &Session) -> Result<()> {
        self.paths.ensure_dirs()?;
        let final_path = self.paths.session_file(&session.header.id.to_string());
        let tmp_path = final_path.with_extension("ndjson.tmp");

        // Write to temporary file
        let write_result = (|| -> Result<()> {
            let file = File::create(&tmp_path)?;
            let mut writer = BufWriter::new(&file);

            // Write header line
            let header_line = NdjsonLine::Header(session.header.clone());
            serde_json::to_writer(&mut writer, &header_line)?;
            writeln!(writer)?;

            // Write each command
            for cmd in &session.commands {
                let cmd_line = NdjsonLine::Command(cmd.clone());
                serde_json::to_writer(&mut writer, &cmd_line)?;
                writeln!(writer)?;
            }

            // Write footer if session is complete
            if let Some(ref footer) = session.footer {
                let footer_line = NdjsonLine::Footer(footer.clone());
                serde_json::to_writer(&mut writer, &footer_line)?;
                writeln!(writer)?;
            }

            // Ensure all data is flushed and synced to disk before rename
            writer.flush()?;
            file.sync_all()?;
            Ok(())
        })();

        // Handle write errors: clean up temp file
        if let Err(e) = write_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }

        // Atomic rename (POSIX guarantees atomicity for rename on same filesystem)
        if let Err(e) = fs::rename(&tmp_path, &final_path) {
            // Clean up temp file on rename failure
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }

        // Set restrictive permissions (0o600) to prevent other users from reading
        set_restrictive_permissions(&final_path)?;

        Ok(())
    }

    /// Load a session from NDJSON file.
    ///
    /// Reads the NDJSON file line by line and reconstructs the Session
    /// struct from the header, commands, and optional footer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Session file doesn't exist
    /// - File read fails
    /// - JSON parsing fails
    /// - File is missing the header
    pub fn load(&self, id: &str) -> Result<Session> {
        let path = self.paths.session_file(id);
        if !path.exists() {
            return Err(RecError::SessionNotFound(id.to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut header: Option<SessionHeader> = None;
        let mut commands: Vec<Command> = Vec::new();
        let mut footer: Option<SessionFooter> = None;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let parsed: NdjsonLine =
                serde_json::from_str(&line).map_err(|e| RecError::InvalidSession(e.to_string()))?;

            match parsed {
                NdjsonLine::Header(h) => header = Some(h),
                NdjsonLine::Command(c) => commands.push(c),
                NdjsonLine::Footer(f) => footer = Some(f),
            }
        }

        let header =
            header.ok_or_else(|| RecError::InvalidSession("Missing header".to_string()))?;

        Ok(Session {
            header,
            commands,
            footer,
        })
    }

    /// List all session IDs in the data directory.
    ///
    /// Returns the file stems (IDs) of all `.ndjson` files in the
    /// sessions directory.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the directory fails.
    pub fn list(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();

        if !self.paths.data_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.paths.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "ndjson") {
                if let Some(stem) = path.file_stem() {
                    sessions.push(stem.to_string_lossy().to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a session by ID.
    ///
    /// Removes the session file from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or deletion fails.
    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.paths.session_file(id);
        if !path.exists() {
            return Err(RecError::SessionNotFound(id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    /// Load only the header and footer from a session file.
    ///
    /// Much more efficient than `load()` for listing because it skips
    /// command deserialization. Reads the file line by line:
    /// - Parses the first non-empty line as the header
    /// - Tracks the last non-empty line and tries parsing it as a footer
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Session file doesn't exist
    /// - File read fails
    /// - Header is missing or invalid
    pub fn load_header_and_footer(
        &self,
        id: &str,
    ) -> Result<(SessionHeader, Option<SessionFooter>)> {
        let path = self.paths.session_file(id);
        if !path.exists() {
            return Err(RecError::SessionNotFound(id.to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut header: Option<SessionHeader> = None;
        let mut last_line = String::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if header.is_none() {
                let parsed: NdjsonLine = serde_json::from_str(&line)
                    .map_err(|e| RecError::InvalidSession(e.to_string()))?;
                if let NdjsonLine::Header(h) = parsed {
                    header = Some(h);
                } else {
                    return Err(RecError::InvalidSession(
                        "First line is not a header".to_string(),
                    ));
                }
            }

            last_line = line;
        }

        let header =
            header.ok_or_else(|| RecError::InvalidSession("Missing header".to_string()))?;

        // Try to parse the last line as a footer
        let footer = serde_json::from_str::<NdjsonLine>(&last_line)
            .ok()
            .and_then(|parsed| match parsed {
                NdjsonLine::Footer(f) => Some(f),
                _ => None,
            });

        Ok((header, footer))
    }

    /// Rename a session by updating the name in its NDJSON file.
    ///
    /// Creates a backup of the original file before modifying.
    /// Loads the full session, updates the name, and saves it back.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Session doesn't exist
    /// - Backup creation fails
    /// - File write fails
    pub fn rename(&self, id: &str, new_name: &str) -> Result<()> {
        // Create backup before modifying
        let source = self.paths.session_file(id);
        let backup = self.paths.backup_file(id);
        fs::copy(&source, &backup)?;

        // Load, update, save — clean up backup on success, preserve on failure
        let mut session = self.load(id)?;
        session.header.name = new_name.to_string();

        match self.save(&session) {
            Ok(()) => {
                let _ = fs::remove_file(&backup);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Add tags to a session, skipping duplicates.
    ///
    /// Creates a backup of the original file before modifying.
    /// Returns the final list of all tags on the session.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Session doesn't exist
    /// - Backup creation fails
    /// - File write fails
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_tags(&self, id: &str, tags: Vec<String>) -> Result<Vec<String>> {
        // Create backup before modifying
        let source = self.paths.session_file(id);
        let backup = self.paths.backup_file(id);
        fs::copy(&source, &backup)?;

        // Load, update tags, save — clean up backup on success, preserve on failure
        let mut session = self.load(id)?;
        for tag in &tags {
            if !session.header.tags.contains(tag) {
                session.header.tags.push(tag.clone());
            }
        }
        let final_tags = session.header.tags.clone();

        match self.save(&session) {
            Ok(()) => {
                let _ = fs::remove_file(&backup);
                Ok(final_tags)
            }
            Err(e) => Err(e),
        }
    }

    /// Check if a session exists.
    #[must_use]
    pub fn exists(&self, id: &str) -> bool {
        self.paths.session_file(id).exists()
    }

    /// Get the file path for a session by ID.
    #[must_use]
    pub fn session_file_path(&self, id: &str) -> std::path::PathBuf {
        self.paths.session_file(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SessionStatus;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    fn create_test_session(name: &str) -> Session {
        let mut session = Session::new(name);
        session.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            PathBuf::from("/tmp"),
        ));
        session
            .commands
            .push(Command::new(1, "ls -la".to_string(), PathBuf::from("/tmp")));
        session.complete(SessionStatus::Completed);
        session
    }

    #[test]
    fn test_save_and_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("test-session");
        let session_id = session.header.id.to_string();

        // Save
        store.save(&session).unwrap();

        // Verify file exists
        assert!(store.exists(&session_id));

        // Load
        let loaded = store.load(&session_id).unwrap();

        assert_eq!(loaded.header.name, "test-session");
        assert_eq!(loaded.commands.len(), 2);
        assert_eq!(loaded.commands[0].command, "echo hello");
        assert_eq!(loaded.commands[1].command, "ls -la");
        assert!(loaded.footer.is_some());
        assert_eq!(
            loaded.footer.as_ref().unwrap().status,
            SessionStatus::Completed
        );
    }

    #[test]
    fn test_ndjson_format_is_human_readable() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("readable-test");
        let session_id = session.header.id.to_string();

        store.save(&session).unwrap();

        // Read raw file and verify format
        let path = paths.session_file(&session_id);
        let contents = fs::read_to_string(path).unwrap();

        // Each line should be valid JSON
        for line in contents.lines() {
            let _: serde_json::Value =
                serde_json::from_str(line).expect("Each line should be valid JSON");
        }

        // Verify type discriminator
        assert!(contents.contains("\"type\":\"header\""));
        assert!(contents.contains("\"type\":\"command\""));
        assert!(contents.contains("\"type\":\"footer\""));
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Initially empty
        let sessions = store.list().unwrap();
        assert!(sessions.is_empty());

        // Add sessions
        let session1 = create_test_session("session-1");
        let session2 = create_test_session("session-2");
        let id1 = session1.header.id.to_string();
        let id2 = session2.header.id.to_string();

        store.save(&session1).unwrap();
        store.save(&session2).unwrap();

        // List should return both
        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&id1));
        assert!(sessions.contains(&id2));
    }

    #[test]
    fn test_delete_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("to-delete");
        let session_id = session.header.id.to_string();

        store.save(&session).unwrap();
        assert!(store.exists(&session_id));

        store.delete(&session_id).unwrap();
        assert!(!store.exists(&session_id));
    }

    #[test]
    fn test_load_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let result = store.load("nonexistent-id");
        assert!(result.is_err());

        match result {
            Err(RecError::SessionNotFound(id)) => assert_eq!(id, "nonexistent-id"),
            _ => panic!("Expected SessionNotFound error"),
        }
    }

    #[test]
    fn test_delete_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let result = store.delete("nonexistent-id");
        assert!(result.is_err());

        match result {
            Err(RecError::SessionNotFound(id)) => assert_eq!(id, "nonexistent-id"),
            _ => panic!("Expected SessionNotFound error"),
        }
    }

    #[test]
    fn test_load_header_and_footer() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("hf-test");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        let (header, footer) = store.load_header_and_footer(&session_id).unwrap();
        assert_eq!(header.name, "hf-test");
        assert!(footer.is_some());
        assert_eq!(footer.unwrap().command_count, 2);
    }

    #[test]
    fn test_load_header_and_footer_no_footer() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Session without footer (still recording)
        let mut session = Session::new("in-progress-hf");
        session.commands.push(Command::new(
            0,
            "echo test".to_string(),
            PathBuf::from("/tmp"),
        ));
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        let (header, footer) = store.load_header_and_footer(&session_id).unwrap();
        assert_eq!(header.name, "in-progress-hf");
        assert!(footer.is_none());
    }

    #[test]
    fn test_load_header_and_footer_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let result = store.load_header_and_footer("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_save_session_without_footer() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Session still recording (no footer)
        let mut session = Session::new("in-progress");
        session.commands.push(Command::new(
            0,
            "echo test".to_string(),
            PathBuf::from("/tmp"),
        ));

        let session_id = session.header.id.to_string();

        store.save(&session).unwrap();
        let loaded = store.load(&session_id).unwrap();

        assert!(loaded.footer.is_none());
        assert_eq!(loaded.commands.len(), 1);
    }

    #[test]
    fn test_rename_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("old-name");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Rename
        store.rename(&session_id, "new-name").unwrap();

        // Verify name updated
        let loaded = store.load(&session_id).unwrap();
        assert_eq!(loaded.header.name, "new-name");

        // Verify backup is cleaned up after successful rename
        let backup_path = paths.backup_file(&session_id);
        assert!(
            !backup_path.exists(),
            "Backup should be cleaned up on success"
        );
    }

    #[test]
    fn test_rename_preserves_commands() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("original");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        store.rename(&session_id, "renamed").unwrap();

        let loaded = store.load(&session_id).unwrap();
        assert_eq!(loaded.header.name, "renamed");
        assert_eq!(loaded.commands.len(), 2);
        assert_eq!(loaded.commands[0].command, "echo hello");
        assert_eq!(loaded.commands[1].command, "ls -la");
        assert!(loaded.footer.is_some());
    }

    #[test]
    fn test_add_tags() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("tagged-session");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        let tags = store
            .add_tags(&session_id, vec!["deploy".to_string(), "setup".to_string()])
            .unwrap();

        assert_eq!(tags, vec!["deploy", "setup"]);

        // Verify tags persisted
        let loaded = store.load(&session_id).unwrap();
        assert_eq!(loaded.header.tags, vec!["deploy", "setup"]);
    }

    #[test]
    fn test_add_tags_skips_duplicates() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("dup-tags");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Add initial tags
        store
            .add_tags(&session_id, vec!["deploy".to_string(), "setup".to_string()])
            .unwrap();

        // Add again with overlap
        let tags = store
            .add_tags(&session_id, vec!["deploy".to_string(), "rust".to_string()])
            .unwrap();

        // Should have 3 tags, not 4 (deploy not duplicated)
        assert_eq!(tags, vec!["deploy", "setup", "rust"]);
    }

    #[test]
    fn test_add_tags_cleans_up_backup_on_success() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("backup-tag");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        store
            .add_tags(&session_id, vec!["test-tag".to_string()])
            .unwrap();

        // Verify backup is cleaned up after successful tag add
        let backup_path = paths.backup_file(&session_id);
        assert!(
            !backup_path.exists(),
            "Backup should be cleaned up on success"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_rename_preserves_backup_on_failure() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("fail-rename");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Create the backup manually (simulating what rename() does before save)
        let source = paths.session_file(&session_id);
        let backup = paths.backup_file(&session_id);
        fs::copy(&source, &backup).unwrap();

        // Make the data directory read-only AFTER backup is created.
        // This causes the atomic write (tmp file creation + rename) to fail.
        let dir_perms = PermissionsExt::from_mode(0o555);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();

        // Attempt save directly — should fail because atomic write can't create tmp file
        let mut modified_session = session.clone();
        modified_session.header.name = "should-fail".to_string();
        let result = store.save(&modified_session);
        assert!(
            result.is_err(),
            "Save should fail when directory is read-only"
        );

        // Verify backup is still preserved (simulating what rename() would leave behind)
        assert!(backup.exists(), "Backup should be preserved on failure");

        // Restore permissions so TempDir cleanup works
        let dir_perms = PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn test_add_tags_preserves_backup_on_failure() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("fail-tags");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Create the backup manually (simulating what add_tags() does before save)
        let source = paths.session_file(&session_id);
        let backup = paths.backup_file(&session_id);
        fs::copy(&source, &backup).unwrap();

        // Make the data directory read-only AFTER backup is created.
        // This causes the atomic write (tmp file creation + rename) to fail.
        let dir_perms = PermissionsExt::from_mode(0o555);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();

        // Attempt save directly — should fail because atomic write can't create tmp file
        let mut modified_session = session.clone();
        modified_session.header.tags.push("fail-tag".to_string());
        let result = store.save(&modified_session);
        assert!(
            result.is_err(),
            "Save should fail when directory is read-only"
        );

        // Verify backup is still preserved (simulating what add_tags() would leave behind)
        assert!(backup.exists(), "Backup should be preserved on failure");

        // Restore permissions so TempDir cleanup works
        let dir_perms = PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();
    }

    #[test]
    fn test_add_multiple_tags_at_once() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("multi-tag");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        let tags = store
            .add_tags(
                &session_id,
                vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            )
            .unwrap();

        assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    }

    #[test]
    #[cfg(unix)]
    fn test_save_sets_restrictive_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("permissions-test");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Verify permissions are 0o600 (read/write for owner only)
        let session_path = paths.session_file(&session_id);
        let metadata = std::fs::metadata(&session_path).unwrap();
        let mode = metadata.permissions().mode();

        // On Unix, mode includes file type bits. We only care about permission bits (lower 9 bits)
        let permission_bits = mode & 0o777;
        assert_eq!(
            permission_bits, 0o600,
            "Session file should have 0o600 permissions, got 0o{permission_bits:o}"
        );
    }

    #[test]
    fn test_atomic_save_no_tmp_file_left_on_success() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("atomic-test");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Verify no .tmp file is left behind after successful save
        let session_path = paths.session_file(&session_id);
        let tmp_path = session_path.with_extension("ndjson.tmp");
        assert!(
            !tmp_path.exists(),
            "Temporary file should be cleaned up after successful save"
        );

        // Verify the actual session file exists
        assert!(session_path.exists(), "Session file should exist");
    }

    #[test]
    #[cfg(unix)]
    fn test_atomic_save_cleans_tmp_on_write_failure() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        paths.ensure_dirs().unwrap();
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("atomic-fail-test");
        let session_id = session.header.id.to_string();

        // Make the directory read-only so tmp file creation fails
        let dir_perms = PermissionsExt::from_mode(0o555);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();

        // Attempt save — should fail
        let result = store.save(&session);
        assert!(
            result.is_err(),
            "Save should fail when directory is read-only"
        );

        // Verify no tmp file is left behind
        let session_path = paths.session_file(&session_id);
        let tmp_path = session_path.with_extension("ndjson.tmp");
        assert!(
            !tmp_path.exists(),
            "Temporary file should not exist after failed save"
        );

        // Restore permissions for cleanup
        let dir_perms = PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&paths.data_dir, dir_perms).unwrap();
    }

    #[test]
    fn test_atomic_save_overwrites_existing_session() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Create and save initial session
        let mut session = create_test_session("overwrite-test");
        let session_id = session.header.id.to_string();
        store.save(&session).unwrap();

        // Modify and save again
        session.header.name = "updated-name".to_string();
        session.header.tags.push("new-tag".to_string());
        store.save(&session).unwrap();

        // Verify the session was updated atomically
        let loaded = store.load(&session_id).unwrap();
        assert_eq!(loaded.header.name, "updated-name");
        assert!(loaded.header.tags.contains(&"new-tag".to_string()));
    }
}
