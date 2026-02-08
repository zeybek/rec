//! Shared test infrastructure for integration tests.
//!
//! Provides `TestEnv` for filesystem-isolated test environments and
//! output factory functions for constructing `Output` without touching
//! real XDG paths or loading config files.
//!
//! **Safety invariants:**
//! - NEVER call `get_paths()` -- poisons the `OnceLock` singleton with real XDG paths
//! - NEVER call `Output::new()` -- calls `load_config()` which calls `Paths::new()`
//! - NEVER call `Paths::new()` -- uses real ProjectDirs/XDG directories
//! - ALL construction uses struct literals with explicit field values

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use uuid::Uuid;

use rec::cli::Output;
use rec::models::config::{SymbolMode, Verbosity};
use rec::models::{Command, Session, SessionFooter, SessionHeader, SessionStatus};
use rec::recording::RecordingState;
use rec::storage::{AliasStore, Paths, SessionStore};

/// Isolated test environment with temp directory, paths, and stores.
///
/// The `_temp_dir` field keeps the `TempDir` alive for the lifetime of the
/// `TestEnv`. When `TestEnv` is dropped, the temp directory and all its
/// contents are cleaned up automatically.
#[allow(dead_code)]
pub struct TestEnv {
    /// Held for side effects (cleanup on drop). Underscore prefix is intentional.
    _temp_dir: TempDir,
    /// XDG-style paths pointing into the temp directory.
    pub paths: Paths,
    /// Session store backed by the temp directory.
    pub store: SessionStore,
    /// Alias store backed by the temp directory.
    pub alias_store: AliasStore,
}

impl TestEnv {
    /// Create a new isolated test environment.
    ///
    /// Constructs `Paths` via struct literal (never `Paths::new()`) so that
    /// all paths point into a fresh temp directory. Calls `ensure_dirs()`
    /// eagerly so tests never need to create directories manually.
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let paths = Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        };
        paths
            .ensure_dirs()
            .expect("Failed to create test directories");
        let store = SessionStore::new(paths.clone());
        let alias_store = AliasStore::new(&paths);
        Self {
            _temp_dir: temp_dir,
            paths,
            store,
            alias_store,
        }
    }

    /// Create a minimal completed session with one command.
    ///
    /// The session has a single `echo hello` command (exit code 0, cwd `/tmp`)
    /// and is marked as `Completed`. Does NOT save to store.
    #[allow(dead_code, clippy::unused_self)]
    pub fn create_session(&self, name: &str) -> Session {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let cmd = Command {
            index: 0,
            command: "echo hello".to_string(),
            cwd: PathBuf::from("/tmp"),
            started_at: now,
            ended_at: Some(now + 0.001),
            exit_code: Some(0),
            duration_ms: Some(1),
        };

        Session {
            header: SessionHeader {
                version: 2,
                id: Uuid::new_v4(),
                name: name.to_string(),
                shell: "bash".to_string(),
                os: "test".to_string(),
                hostname: "test-host".to_string(),
                env: HashMap::new(),
                tags: Vec::new(),
                recovered: None,
                started_at: now,
            },
            commands: vec![cmd],
            footer: Some(SessionFooter {
                ended_at: now + 0.001,
                command_count: 1,
                status: SessionStatus::Completed,
            }),
        }
    }

    /// Create a completed session and persist it to the store.
    ///
    /// Returns the saved session (same as `create_session` but also written to disk).
    #[allow(dead_code)]
    pub fn create_and_save_session(&self, name: &str) -> Session {
        let session = self.create_session(name);
        self.store.save(&session).expect("Failed to save session");
        session
    }

    /// Create a completed session with the given command strings.
    ///
    /// All commands get exit code 0 and cwd `/tmp`. Does NOT save to store.
    #[allow(dead_code, clippy::unused_self)]
    pub fn create_session_with_commands(&self, name: &str, commands: &[&str]) -> Session {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let cmds: Vec<Command> = commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| Command {
                #[allow(clippy::cast_possible_truncation)]
                index: i as u32,
                command: cmd.to_string(),
                cwd: PathBuf::from("/tmp"),
                #[allow(clippy::cast_precision_loss)]
                started_at: now + (i as f64 * 0.001),
                #[allow(clippy::cast_precision_loss)]
                ended_at: Some(now + ((i as f64 + 1.0) * 0.001)),
                exit_code: Some(0),
                duration_ms: Some(1),
            })
            .collect();

        #[allow(clippy::cast_possible_truncation)]
        let count = cmds.len() as u32;

        Session {
            header: SessionHeader {
                version: 2,
                id: Uuid::new_v4(),
                name: name.to_string(),
                shell: "bash".to_string(),
                os: "test".to_string(),
                hostname: "test-host".to_string(),
                env: HashMap::new(),
                tags: Vec::new(),
                recovered: None,
                started_at: now,
            },
            commands: cmds,
            footer: Some(SessionFooter {
                ended_at: now + (f64::from(count) * 0.001),
                command_count: count,
                status: SessionStatus::Completed,
            }),
        }
    }

    /// Create a completed session with tags set on the header.
    ///
    /// Has one default command. Does NOT save to store.
    #[allow(dead_code)]
    pub fn create_session_with_tags(&self, name: &str, tags: &[&str]) -> Session {
        let mut session = self.create_session(name);
        session.header.tags = tags.iter().map(std::string::ToString::to_string).collect();
        session
    }

    /// Create a `RecordingState` backed by this environment's state directory.
    ///
    /// The `RecordingState` manages recording.lock and recording.json files
    /// in the test environment's `state_dir`, isolated from real XDG paths.
    #[allow(dead_code)]
    pub fn recording_state(&self) -> RecordingState {
        RecordingState::new(&self.paths.state_dir)
    }
}

// ---------------------------------------------------------------------------
// Output factory functions
//
// These are free functions (not on TestEnv) because Output has no filesystem
// relationship. Tests that don't need TestEnv can still use these.
//
// All construct Output via struct literal -- NEVER via Output::new() which
// calls load_config() and touches real XDG paths.
// ---------------------------------------------------------------------------

/// Create an Output with quiet verbosity (only errors shown).
#[allow(dead_code)]
pub fn quiet_output() -> Output {
    Output {
        colors: false,
        symbols: SymbolMode::Ascii,
        verbosity: Verbosity::Quiet,
        json: false,
    }
}

/// Create an Output with verbose verbosity (debug messages shown).
#[allow(dead_code)]
pub fn verbose_output() -> Output {
    Output {
        colors: false,
        symbols: SymbolMode::Ascii,
        verbosity: Verbosity::Verbose,
        json: false,
    }
}

/// Create an Output with JSON mode enabled.
#[allow(dead_code)]
pub fn json_output() -> Output {
    Output {
        colors: false,
        symbols: SymbolMode::Ascii,
        verbosity: Verbosity::Normal,
        json: true,
    }
}

/// Create an Output with normal verbosity and no JSON.
#[allow(dead_code)]
pub fn normal_output() -> Output {
    Output {
        colors: false,
        symbols: SymbolMode::Ascii,
        verbosity: Verbosity::Normal,
        json: false,
    }
}
