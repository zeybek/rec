use rec::cli::Output;
use rec::error::RecError;
use rec::recording::RecordingState;
use rec::session::ResolveError;
use rec::storage::{Paths, SessionStore};
use std::process::ExitCode;

/// Get the current Unix timestamp in seconds, falling back to 0.0 if the system clock is before epoch.
pub fn unix_timestamp() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Print a `serde_json::Value` as pretty JSON to stdout.
///
/// `serde_json::to_string_pretty` on a `serde_json::Value` is infallible
/// in practice, but we use `expect` with a clear message rather than `unwrap`.
pub fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("JSON serialization of Value is infallible")
    );
}

/// Handle a generic `RecError` by printing and returning the appropriate exit code.
///
/// Uses "Error" as the default title and no help text. For customization,
/// use `handle_error_with_context` instead.
pub fn handle_error(e: &RecError, output: Output) -> ExitCode {
    output.error("Error", &e.to_string(), None, None);
    ExitCode::from(e.exit_code())
}

/// Handle a `RecError` with custom title and optional help text.
///
/// This provides flexibility for handlers that need specific error messaging
/// while still centralizing the exit code logic.
pub fn handle_error_with_context(
    e: &RecError,
    output: Output,
    title: &str,
    help: Option<&str>,
) -> ExitCode {
    output.error(title, &e.to_string(), None, help);
    ExitCode::from(e.exit_code())
}

/// Handle a session resolve error by printing fuzzy suggestions and the error.
///
/// Returns the appropriate `ExitCode` for the error.
pub fn handle_resolve_error(resolve_err: &ResolveError, output: Output) -> ExitCode {
    let help = match &resolve_err.error {
        RecError::SessionNotFound(_) => {
            if resolve_err.suggestions.is_empty() {
                Some("No similar names found. List all sessions with: rec list".to_string())
            } else {
                let suggestion_text =
                    rec::session::fuzzy::format_suggestions(&resolve_err.suggestions);
                Some(format!(
                    "{suggestion_text}\nList all sessions with: rec list"
                ))
            }
        }
        _ => None,
    };
    output.error(
        "Session not found",
        &resolve_err.error.to_string(),
        None,
        help.as_deref(),
    );
    ExitCode::from(resolve_err.error.exit_code())
}

/// Check for and clean up stale recording locks, printing warnings if found.
///
/// If `store` is provided, recovery will be attempted for incomplete sessions.
/// Pass `None` if you only want to clean up the lock without recovery.
pub fn cleanup_stale_lock_with_warning(
    recording_state: &RecordingState,
    store: Option<&SessionStore>,
) {
    if let Ok(Some(recovery)) = recording_state.cleanup_stale_lock(store) {
        eprintln!(
            "warning: Cleaned up stale lock from process {} (no longer running)",
            recovery.dead_pid
        );
        if let Some(ref name) = recovery.recovered_name {
            eprintln!("warning: Recovered incomplete session as '{name}'");
        }
    }
}

/// Pending command state passed from preexec to precmd via file.
///
/// Written to `{state_dir}/pending.json` during preexec, read and
/// deleted during precmd. Contains the command text, working directory,
/// and start timestamp needed to complete the command record.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PendingCommand {
    pub command: String,
    pub cwd: std::path::PathBuf,
    pub started_at: f64,
}

/// Format a duration in seconds as a human-readable string.
pub fn format_duration(seconds: f64) -> String {
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

/// Convert a command handler result to an `ExitCode`, printing contextual error messages.
///
/// Maps `Ok(())` to `ExitCode::SUCCESS` and `Err(e)` to the appropriate exit code
/// with user-friendly error messages and help text.
pub fn handle_result(result: Result<(), RecError>, output: Output, paths: &Paths) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let (error_type, help) = match &e {
                RecError::SessionNotFound(name) => (
                    "Session not found",
                    Some(format!(
                        "Run 'rec list' to see available sessions. No session named '{name}'"
                    )),
                ),
                RecError::SessionExists(name) => (
                    "Session already exists",
                    Some(format!(
                        "A session named '{name}' already exists. Use a different name or delete it first"
                    )),
                ),
                RecError::Io(_) => (
                    "IO error",
                    Some("Check file permissions and disk space".to_string()),
                ),
                RecError::Json(_) => (
                    "Serialization error",
                    Some("Session file may be corrupted".to_string()),
                ),
                RecError::Toml(_) => (
                    "Config syntax error",
                    Some(format!(
                        "Check config file syntax at {}",
                        paths.config_file.display()
                    )),
                ),
                RecError::InvalidSession(_) => (
                    "Invalid session",
                    Some("Session file is malformed or corrupted".to_string()),
                ),
                RecError::Config(_) => ("Config error", None),
                RecError::RecordingInProgress => (
                    "Recording in progress",
                    Some("Run 'rec stop' to stop the current recording first".to_string()),
                ),
                RecError::NoActiveRecording => (
                    "No active recording",
                    Some("Run 'rec start' to begin a new recording".to_string()),
                ),
                RecError::StaleLock(pid) => (
                    "Stale lock detected",
                    Some(format!(
                        "Lock from process {pid} (no longer running). Run 'rec start' to auto-clean"
                    )),
                ),
                RecError::InvalidSessionName(_) => (
                    "Invalid session name",
                    Some(
                        "Session names can only contain letters, numbers, dashes, and underscores"
                            .to_string(),
                    ),
                ),
                RecError::InvalidAliasName(_) => (
                    "Invalid alias name",
                    Some(
                        "Alias names can only contain letters, numbers, dashes, and underscores"
                            .to_string(),
                    ),
                ),
                RecError::InvalidTagName(_) => (
                    "Invalid tag name",
                    Some(
                        "Tag names can only contain letters, numbers, dashes, and underscores"
                            .to_string(),
                    ),
                ),
            };

            output.error(error_type, &e.to_string(), None, help.as_deref());

            // Differentiated exit codes: 1 for user errors, 2 for system errors
            ExitCode::from(e.exit_code())
        }
    }
}
