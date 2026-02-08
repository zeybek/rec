//! Replay module for executing recorded terminal sessions.
//!
//! Provides safety detection, command execution, interactive prompts,
//! and configuration for session replay.

pub mod engine;
pub mod executor;
pub mod prompt;
pub mod safety;

pub use engine::ReplayEngine;

use std::collections::HashSet;

/// Policy for handling dangerous (destructive) commands during replay.
///
/// Used by CI/CD pipelines and non-interactive environments to control
/// how destructive commands are handled without requiring TTY interaction.
///
/// When `None` is used in `ReplayOptions`, the existing interactive behavior
/// is preserved (prompt in TTY, auto-skip in non-interactive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DangerPolicy {
    /// Skip dangerous commands with warnings
    Skip,
    /// Scan for dangerous commands and abort without executing
    Abort,
    /// Execute all commands including dangerous ones
    Allow,
}

/// Options controlling replay behavior.
///
/// All indices are 0-based internally. Conversion from 1-based
/// user input happens at the CLI boundary.
#[derive(Debug, Clone, Default)]
pub struct ReplayOptions {
    /// Preview commands without executing
    pub dry_run: bool,
    /// Execute one command at a time with confirmation
    pub step: bool,
    /// Set of command indices to skip (0-based)
    pub skip_indices: HashSet<usize>,
    /// Glob patterns — commands matching any pattern are skipped
    pub skip_patterns: Vec<glob::Pattern>,
    /// Start execution from this command index (0-based)
    pub from_index: Option<usize>,
    /// Bypass destructive command prompts
    pub force: bool,
    /// Replay in each command's original working directory
    pub use_original_cwd: bool,
    /// Policy for handling dangerous commands (None = use existing interactive behavior)
    pub danger_policy: Option<DangerPolicy>,
}

/// Summary of a replay execution.
#[derive(Debug, Clone)]
pub struct ReplaySummary {
    /// Total number of commands in the session
    pub total: usize,
    /// Number of commands successfully executed
    pub executed: usize,
    /// Number of commands skipped
    pub skipped: usize,
    /// Number of commands that failed
    pub failed: usize,
    /// Whether the replay was aborted (Ctrl+C or user choice)
    pub aborted: bool,
}
