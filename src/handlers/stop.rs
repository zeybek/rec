use super::HandlerContext;
use super::common::{format_duration, handle_error_with_context, print_json, unix_timestamp};
use rec::error::EXIT_USER_ERROR;
use rec::models::{SessionFooter, SessionStatus};
use rec::recording::{CommandCapture, RecordingState};
use std::process::ExitCode;

/// Handle the `stop` command — finalize and save the current recording session.
///
/// Writes the session footer with command count and duration, then cleans up
/// the recording state files (lock and state).
///
/// # Errors
/// Returns exit code 1 if no recording is in progress.
/// Returns exit code 2 on I/O failures.
pub fn handle_stop(ctx: &HandlerContext) -> ExitCode {
    let recording_state = RecordingState::new(&ctx.paths.state_dir);

    // Check if recording
    if !recording_state.is_recording() {
        ctx.output.error(
            "Not recording",
            "No recording session is in progress",
            None,
            Some("Start a recording with: rec start [name]"),
        );
        return ExitCode::from(EXIT_USER_ERROR);
    }

    // Get active session info
    let active_session = match recording_state.current() {
        Ok(s) => s,
        Err(e) => {
            return handle_error_with_context(&e, ctx.output, "Failed to read session state", None);
        }
    };

    // Count commands by reading the NDJSON file directly
    let command_count = match std::fs::read_to_string(&active_session.session_path) {
        Ok(contents) => contents
            .lines()
            .filter(|line| line.contains("\"type\":\"command\""))
            .count() as u32,
        Err(_) => 0,
    };

    // Calculate duration
    let ended_at = unix_timestamp();
    let duration_secs = ended_at - active_session.started_at;

    // Write footer
    let footer = SessionFooter {
        ended_at,
        command_count,
        status: SessionStatus::Completed,
    };

    if let Err(e) = CommandCapture::write_footer(&active_session.session_path, &footer) {
        return handle_error_with_context(&e, ctx.output, "Failed to write session footer", None);
    }

    // Stop recording state (removes state and lock files)
    if let Err(e) = recording_state.stop() {
        ctx.output
            .warning(&format!("Failed to clean up recording state: {e}"));
        // Continue anyway - session is saved
    }

    // Format duration
    let duration_str = format_duration(duration_secs);

    // Output summary
    if ctx.output.json {
        let json = serde_json::json!({
            "status": "completed",
            "session": {
                "id": active_session.id.to_string(),
                "name": active_session.name,
                "path": active_session.session_path.to_string_lossy()
            },
            "summary": {
                "command_count": command_count,
                "duration_seconds": duration_secs,
                "duration_human": duration_str
            }
        });
        print_json(&json);
    } else {
        ctx.output.success(&format!(
            "Session '{}' saved ({} commands, {})",
            active_session.name, command_count, duration_str
        ));
    }

    ExitCode::SUCCESS
}
