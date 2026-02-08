use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use fd_lock::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{RecError, Result};
use crate::storage::SessionStore;

/// Information about a recovered stale session.
#[derive(Debug)]
pub struct RecoveryInfo {
    /// PID of the dead process
    pub dead_pid: u32,
    /// Name of the recovered session (if any)
    pub recovered_name: Option<String>,
    /// Number of commands in the recovered session
    pub command_count: usize,
}

/// Metadata for an active recording session.
///
/// Stored as JSON in the state directory while a recording is in progress.
/// Contains enough information to resume or clean up after a crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSession {
    /// Unique session identifier
    pub id: Uuid,

    /// Human-readable session name
    pub name: String,

    /// Path to the session NDJSON file
    pub session_path: PathBuf,

    /// Unix timestamp when recording started
    pub started_at: f64,

    /// PID of the recording process
    pub pid: u32,
}

/// Manages recording lifecycle with file-based locking.
///
/// Uses `fd-lock` to ensure only one recording can be active at a time.
/// State is persisted to disk so it survives process restarts and can
/// detect stale locks from crashed processes.
///
/// # File Layout
///
/// - `{state_dir}/recording.lock` - Lock file for mutual exclusion
/// - `{state_dir}/recording.json` - Active session metadata
pub struct RecordingState {
    /// Path to the lock file
    lock_path: PathBuf,

    /// Path to the state JSON file
    state_path: PathBuf,
}

impl RecordingState {
    /// Create a new `RecordingState` using the given state directory.
    ///
    /// Does not create the directory; caller should ensure it exists
    /// (e.g., via `Paths::ensure_dirs()`).
    #[must_use]
    pub fn new(state_dir: &Path) -> Self {
        Self {
            lock_path: state_dir.join("recording.lock"),
            state_path: state_dir.join("recording.json"),
        }
    }

    /// Start a new recording session.
    ///
    /// Acquires an exclusive file lock, writes session metadata, and
    /// returns the `ActiveSession`. If another recording is in progress
    /// (lock held by live process), returns `RecError::RecordingInProgress`.
    /// If a stale lock is detected (process no longer alive), cleans it up
    /// first and then starts.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable session name
    /// * `session_path` - Path where the session NDJSON file will be written
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired or file I/O fails.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    pub fn start(&self, name: &str, session_path: PathBuf) -> Result<ActiveSession> {
        // Clean up stale lock if needed (no recovery in start — caller handles that)
        self.cleanup_stale_lock(None)?;

        // Try to acquire lock
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)?;

        let mut lock = RwLock::new(lock_file);
        let Ok(mut guard) = lock.try_write() else {
            return Err(RecError::RecordingInProgress);
        };

        let pid = std::process::id();
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let session = ActiveSession {
            id: Uuid::new_v4(),
            name: name.to_string(),
            session_path,
            started_at,
            pid,
        };

        // Write PID to lock file
        guard.set_len(0)?;
        write!(guard, "{pid}")?;
        guard.flush()?;

        // Write session state
        let state_json = serde_json::to_string_pretty(&session)?;
        fs::write(&self.state_path, state_json)?;

        // Drop the guard explicitly - we keep the lock file with PID written,
        // but release the fd-lock. The PID-based check handles crash detection.
        drop(guard);
        drop(lock);

        Ok(session)
    }

    /// Stop the current recording session.
    ///
    /// Reads the active session metadata, removes the state and lock files,
    /// and returns the session info. Returns `RecError::NoActiveRecording`
    /// if no recording is in progress.
    ///
    /// # Errors
    ///
    /// Returns an error if no recording is active or file removal fails.
    pub fn stop(&self) -> Result<ActiveSession> {
        let session = self.current()?;

        // Remove state file first, then lock file
        if self.state_path.exists() {
            fs::remove_file(&self.state_path)?;
        }
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path)?;
        }

        Ok(session)
    }

    /// Check if a recording is currently in progress.
    ///
    /// Returns `true` if the state file exists and contains valid session data.
    #[must_use]
    pub fn is_recording(&self) -> bool {
        self.current().is_ok()
    }

    /// Get the current active session metadata.
    ///
    /// Returns `RecError::NoActiveRecording` if no recording is active.
    ///
    /// # Errors
    ///
    /// Returns an error if no recording is active or the state file is corrupted.
    pub fn current(&self) -> Result<ActiveSession> {
        if !self.state_path.exists() {
            return Err(RecError::NoActiveRecording);
        }

        let contents = fs::read_to_string(&self.state_path)?;
        let session: ActiveSession = serde_json::from_str(&contents).map_err(|e| {
            RecError::InvalidSession(format!("Failed to parse recording state: {e}"))
        })?;

        Ok(session)
    }

    /// Clean up stale lock from a crashed process, optionally recovering the session.
    ///
    /// Reads the PID from the lock file and checks if that process is still
    /// alive using `libc::kill(pid, 0)` on Unix. If the process is dead,
    /// attempts recovery (if `store` is provided) and then removes lock/state files.
    ///
    /// # Recovery behavior
    ///
    /// When `store` is `Some`:
    /// - Sessions with ≥1 command are renamed to `recovered-YYYY-MM-DD-HHMMSS`
    ///   and flagged with `recovered: Some(true)`.
    /// - Sessions with 0 commands are silently deleted.
    /// - Recovery is best-effort: errors loading/saving are logged and cleanup proceeds.
    ///
    /// When `store` is `None`:
    /// - Existing behavior: just remove lock and state files.
    ///
    /// # Safety
    ///
    /// - We only clean up if the process is definitively dead
    /// - PIDs can be reused, but the window is very small for a recording tool
    /// - The lock file contains only a PID, so worst case we clean up an
    ///   unrelated process's "lock" (but they wouldn't be using our lock file)
    ///
    /// # Errors
    ///
    /// Returns an error if file I/O operations fail unexpectedly.
    pub fn cleanup_stale_lock(&self, store: Option<&SessionStore>) -> Result<Option<RecoveryInfo>> {
        if !self.lock_path.exists() {
            return Ok(None);
        }

        // Read PID from lock file
        let Ok(mut lock_file) = File::open(&self.lock_path) else {
            return Ok(None); // Lock file disappeared, that's fine
        };

        let mut pid_str = String::new();
        if lock_file.read_to_string(&mut pid_str).is_err() {
            // Can't read lock file, remove it
            let _ = fs::remove_file(&self.lock_path);
            let _ = fs::remove_file(&self.state_path);
            return Ok(None);
        }

        let pid_str = pid_str.trim();
        if pid_str.is_empty() {
            // Empty lock file, remove it
            let _ = fs::remove_file(&self.lock_path);
            let _ = fs::remove_file(&self.state_path);
            return Ok(None);
        }

        let pid: u32 = if let Ok(p) = pid_str.parse() {
            p
        } else {
            // Invalid PID in lock file, remove it
            let _ = fs::remove_file(&self.lock_path);
            let _ = fs::remove_file(&self.state_path);
            return Ok(None);
        };

        // Check if process is still alive
        if is_process_alive(pid) {
            return Ok(None);
        }

        // Process is dead — stale lock detected
        let recovery_info = if let Some(store) = store {
            Some(self.attempt_recovery(pid, store))
        } else {
            Some(RecoveryInfo {
                dead_pid: pid,
                recovered_name: None,
                command_count: 0,
            })
        };

        // Always clean up lock and state files
        let _ = fs::remove_file(&self.lock_path);
        let _ = fs::remove_file(&self.state_path);

        Ok(recovery_info)
    }

    /// Attempt to recover a session from a dead recording process.
    ///
    /// Reads the active session metadata, loads the NDJSON file, and either
    /// saves it as a recovered session or deletes it if empty.
    /// Returns `None` only if recovery completely fails (errors are best-effort).
    fn attempt_recovery(&self, pid: u32, store: &SessionStore) -> RecoveryInfo {
        // Read active session metadata before cleaning up
        let Ok(active) = self.current() else {
            // Can't read state — no recovery possible
            return RecoveryInfo {
                dead_pid: pid,
                recovered_name: None,
                command_count: 0,
            };
        };

        // Try to load the session from the NDJSON file
        let Ok(mut session) = store.load(&active.id.to_string()) else {
            // Can't load session — no recovery possible
            return RecoveryInfo {
                dead_pid: pid,
                recovered_name: None,
                command_count: 0,
            };
        };

        let command_count = session.commands.len();

        if command_count >= 1 {
            // Recover: rename and flag
            let recovered_name = Local::now().format("recovered-%Y-%m-%d-%H%M%S").to_string();
            session.header.name.clone_from(&recovered_name);
            session.header.recovered = Some(true);

            match store.save(&session) {
                Ok(()) => RecoveryInfo {
                    dead_pid: pid,
                    recovered_name: Some(recovered_name),
                    command_count,
                },
                Err(_) => {
                    // Save failed — best-effort, report what we can
                    RecoveryInfo {
                        dead_pid: pid,
                        recovered_name: None,
                        command_count,
                    }
                }
            }
        } else {
            // Empty session — discard
            let _ = store.delete(&active.id.to_string());
            RecoveryInfo {
                dead_pid: pid,
                recovered_name: None,
                command_count: 0,
            }
        }
    }
}

/// Check if a process with the given PID is still alive.
///
/// On Unix, uses `libc::kill(pid, 0)` which checks process existence
/// without sending a signal. Returns `true` if the process exists.
#[cfg(unix)]
#[allow(unsafe_code)]
fn is_process_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) is a standard POSIX operation that checks
    // if a process exists without sending any signal. No memory is
    // accessed; the syscall only queries the kernel process table.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Fallback for non-Unix platforms.
///
/// Always returns `true` (assumes process is alive) to avoid
/// accidentally cleaning up active locks.
#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    true // Conservative: assume alive on non-Unix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session};
    use crate::storage::Paths;
    use tempfile::TempDir;

    fn setup() -> (TempDir, RecordingState) {
        let tmp = TempDir::new().unwrap();
        let state = RecordingState::new(tmp.path());
        (tmp, state)
    }

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    #[test]
    fn test_start_creates_state_and_lock_files() {
        let (tmp, state) = setup();
        let session_path = tmp.path().join("test.ndjson");

        let session = state.start("test-session", session_path.clone()).unwrap();

        assert_eq!(session.name, "test-session");
        assert_eq!(session.session_path, session_path);
        assert!(session.started_at > 0.0);
        assert_eq!(session.pid, std::process::id());

        // State file should exist
        assert!(state.state_path.exists());
        // Lock file should exist
        assert!(state.lock_path.exists());
    }

    #[test]
    fn test_stop_removes_files() {
        let (tmp, state) = setup();
        let session_path = tmp.path().join("test.ndjson");

        state.start("test-session", session_path).unwrap();
        let session = state.stop().unwrap();

        assert_eq!(session.name, "test-session");
        assert!(!state.state_path.exists());
        assert!(!state.lock_path.exists());
    }

    #[test]
    fn test_stop_without_recording_returns_error() {
        let (_tmp, state) = setup();

        let result = state.stop();
        assert!(result.is_err());
        match result.unwrap_err() {
            RecError::NoActiveRecording => {}
            e => panic!("Expected NoActiveRecording, got {e:?}"),
        }
    }

    #[test]
    fn test_is_recording() {
        let (tmp, state) = setup();

        assert!(!state.is_recording());

        let session_path = tmp.path().join("test.ndjson");
        state.start("test-session", session_path).unwrap();

        assert!(state.is_recording());

        state.stop().unwrap();

        assert!(!state.is_recording());
    }

    #[test]
    fn test_current_returns_session_info() {
        let (tmp, state) = setup();
        let session_path = tmp.path().join("test.ndjson");

        let started = state.start("test-session", session_path.clone()).unwrap();
        let current = state.current().unwrap();

        assert_eq!(current.id, started.id);
        assert_eq!(current.name, "test-session");
        assert_eq!(current.session_path, session_path);
    }

    #[test]
    fn test_current_without_recording_returns_error() {
        let (_tmp, state) = setup();

        let result = state.current();
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_stale_lock_removes_dead_process_lock() {
        let (tmp, state) = setup();

        // Write a lock file with a PID that almost certainly doesn't exist
        // Using a very high PID that's unlikely to be a real process
        let fake_pid = 4_000_000_000u32;
        fs::write(&state.lock_path, fake_pid.to_string()).unwrap();

        // Write a fake state file
        let fake_session = ActiveSession {
            id: Uuid::new_v4(),
            name: "stale-session".to_string(),
            session_path: tmp.path().join("stale.ndjson"),
            started_at: 0.0,
            pid: fake_pid,
        };
        let state_json = serde_json::to_string(&fake_session).unwrap();
        fs::write(&state.state_path, state_json).unwrap();

        // Cleanup should remove both files
        let result = state.cleanup_stale_lock(None).unwrap();

        assert!(!state.lock_path.exists());
        assert!(!state.state_path.exists());
        // Should return RecoveryInfo with dead_pid
        let info = result.expect("Should return RecoveryInfo for dead process");
        assert_eq!(info.dead_pid, fake_pid);
    }

    #[test]
    fn test_cleanup_stale_lock_preserves_live_process() {
        let (tmp, state) = setup();

        // Write lock file with current process PID (definitely alive)
        let our_pid = std::process::id();
        fs::write(&state.lock_path, our_pid.to_string()).unwrap();

        let fake_session = ActiveSession {
            id: Uuid::new_v4(),
            name: "live-session".to_string(),
            session_path: tmp.path().join("live.ndjson"),
            started_at: 0.0,
            pid: our_pid,
        };
        let state_json = serde_json::to_string(&fake_session).unwrap();
        fs::write(&state.state_path, state_json).unwrap();

        // Cleanup should preserve both files (our process is alive)
        let result = state.cleanup_stale_lock(None).unwrap();

        assert!(state.lock_path.exists());
        assert!(state.state_path.exists());
        assert!(result.is_none(), "Should return None for live process");
    }

    #[test]
    fn test_cleanup_empty_lock_file() {
        let (_tmp, state) = setup();

        // Write an empty lock file
        fs::write(&state.lock_path, "").unwrap();

        let result = state.cleanup_stale_lock(None).unwrap();

        assert!(!state.lock_path.exists());
        assert!(result.is_none());
    }

    #[test]
    fn test_cleanup_invalid_pid_in_lock() {
        let (_tmp, state) = setup();

        // Write an invalid PID
        fs::write(&state.lock_path, "not-a-pid").unwrap();

        let result = state.cleanup_stale_lock(None).unwrap();

        assert!(!state.lock_path.exists());
        assert!(result.is_none());
    }

    #[test]
    fn test_start_after_stale_cleanup() {
        let (tmp, state) = setup();

        // Create a stale lock with a dead PID
        let fake_pid = 4_000_000_000u32;
        fs::write(&state.lock_path, fake_pid.to_string()).unwrap();
        let fake_session = ActiveSession {
            id: Uuid::new_v4(),
            name: "stale".to_string(),
            session_path: tmp.path().join("stale.ndjson"),
            started_at: 0.0,
            pid: fake_pid,
        };
        fs::write(
            &state.state_path,
            serde_json::to_string(&fake_session).unwrap(),
        )
        .unwrap();

        // start() should auto-clean stale lock and succeed
        let session_path = tmp.path().join("new.ndjson");
        let session = state.start("new-session", session_path).unwrap();

        assert_eq!(session.name, "new-session");
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_current_process() {
        assert!(is_process_alive(std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_dead_process() {
        // PID 4 billion is almost certainly not running
        assert!(!is_process_alive(4_000_000_000));
    }

    #[test]
    fn test_cleanup_stale_lock_recovers_session_with_commands() {
        let tmp = TempDir::new().unwrap();
        let paths = create_test_paths(&tmp);
        let store = SessionStore::new(paths.clone());

        // Create state dir for RecordingState
        let state_dir = tmp.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let rec_state = RecordingState::new(&state_dir);

        // Create a session with 2 commands and save it via the store
        let mut session = Session::new("original-name");
        session.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            std::path::PathBuf::from("/tmp"),
        ));
        session.commands.push(Command::new(
            1,
            "ls -la".to_string(),
            std::path::PathBuf::from("/tmp"),
        ));
        let session_id = session.header.id;
        store.save(&session).unwrap();

        // Create stale lock and state files pointing to this session
        let fake_pid = 4_000_000_000u32;
        fs::write(&rec_state.lock_path, fake_pid.to_string()).unwrap();
        let active = ActiveSession {
            id: session_id,
            name: "original-name".to_string(),
            session_path: paths.session_file(&session_id.to_string()),
            started_at: 0.0,
            pid: fake_pid,
        };
        fs::write(
            &rec_state.state_path,
            serde_json::to_string(&active).unwrap(),
        )
        .unwrap();

        // Call cleanup with store — should recover
        let result = rec_state.cleanup_stale_lock(Some(&store)).unwrap();

        // Verify lock and state files removed
        assert!(!rec_state.lock_path.exists());
        assert!(!rec_state.state_path.exists());

        // Verify recovery info
        let info = result.expect("Should return RecoveryInfo");
        assert_eq!(info.dead_pid, fake_pid);
        assert_eq!(info.command_count, 2);
        assert!(
            info.recovered_name.is_some(),
            "Should have recovered_name for session with commands"
        );
        let recovered_name = info.recovered_name.unwrap();
        assert!(
            recovered_name.starts_with("recovered-"),
            "Name should start with 'recovered-': {recovered_name}"
        );

        // Verify the session file was updated
        let loaded = store.load(&session_id.to_string()).unwrap();
        assert!(
            loaded.header.name.starts_with("recovered-"),
            "Session name should be updated"
        );
        assert_eq!(loaded.header.recovered, Some(true));
        assert_eq!(loaded.commands.len(), 2);
    }

    #[test]
    fn test_cleanup_stale_lock_discards_empty_session() {
        let tmp = TempDir::new().unwrap();
        let paths = create_test_paths(&tmp);
        let store = SessionStore::new(paths.clone());

        // Create state dir for RecordingState
        let state_dir = tmp.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let rec_state = RecordingState::new(&state_dir);

        // Create a session with 0 commands
        let session = Session::new("empty-session");
        let session_id = session.header.id;
        store.save(&session).unwrap();

        // Verify session file exists
        assert!(store.exists(&session_id.to_string()));

        // Create stale lock and state files
        let fake_pid = 4_000_000_000u32;
        fs::write(&rec_state.lock_path, fake_pid.to_string()).unwrap();
        let active = ActiveSession {
            id: session_id,
            name: "empty-session".to_string(),
            session_path: paths.session_file(&session_id.to_string()),
            started_at: 0.0,
            pid: fake_pid,
        };
        fs::write(
            &rec_state.state_path,
            serde_json::to_string(&active).unwrap(),
        )
        .unwrap();

        // Call cleanup with store — should discard
        let result = rec_state.cleanup_stale_lock(Some(&store)).unwrap();

        // Verify lock and state files removed
        assert!(!rec_state.lock_path.exists());
        assert!(!rec_state.state_path.exists());

        // Verify recovery info
        let info = result.expect("Should return RecoveryInfo");
        assert_eq!(info.dead_pid, fake_pid);
        assert_eq!(info.command_count, 0);
        assert!(
            info.recovered_name.is_none(),
            "Should not have recovered_name for empty session"
        );

        // Verify the session file was deleted
        assert!(
            !store.exists(&session_id.to_string()),
            "Empty session file should be deleted"
        );
    }

    #[test]
    fn test_cleanup_stale_lock_without_store_backward_compat() {
        let (tmp, state) = setup();

        // Create stale lock with dead PID
        let fake_pid = 4_000_000_000u32;
        fs::write(&state.lock_path, fake_pid.to_string()).unwrap();

        let fake_session = ActiveSession {
            id: Uuid::new_v4(),
            name: "stale-compat".to_string(),
            session_path: tmp.path().join("stale.ndjson"),
            started_at: 0.0,
            pid: fake_pid,
        };
        fs::write(
            &state.state_path,
            serde_json::to_string(&fake_session).unwrap(),
        )
        .unwrap();

        // Call cleanup without store (backward compat)
        let result = state.cleanup_stale_lock(None).unwrap();

        // Verify lock and state files removed
        assert!(!state.lock_path.exists());
        assert!(!state.state_path.exists());

        // Verify recovery info — dead_pid only, no recovery
        let info = result.expect("Should return RecoveryInfo");
        assert_eq!(info.dead_pid, fake_pid);
        assert!(info.recovered_name.is_none());
        assert_eq!(info.command_count, 0);
    }
}
