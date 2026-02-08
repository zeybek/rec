use super::HandlerContext;
use super::common::{format_duration, print_json, unix_timestamp};
use rec::error::EXIT_SYSTEM_ERROR;
use std::process::ExitCode;

/// Handle the `status` command — shows current recording status.
pub fn handle_status(ctx: &HandlerContext) -> ExitCode {
    use rec::recording::RecordingState;

    let recording_state = RecordingState::new(&ctx.paths.state_dir);

    if !recording_state.is_recording() {
        if ctx.output.json {
            let json = serde_json::json!({
                "recording": false
            });
            print_json(&json);
        } else {
            ctx.output.info("Not currently recording");
            ctx.output.info(&format!(
                "Start a recording with: {}",
                ctx.output.style_command("rec start [name]")
            ));
        }
        return ExitCode::SUCCESS;
    }

    let active_session = match recording_state.current() {
        Ok(s) => s,
        Err(e) => {
            ctx.output
                .error("Failed to read recording state", &e.to_string(), None, None);
            return ExitCode::from(EXIT_SYSTEM_ERROR);
        }
    };

    // Count commands by reading NDJSON file directly
    let command_count = match std::fs::read_to_string(&active_session.session_path) {
        Ok(contents) => contents
            .lines()
            .filter(|line| line.contains("\"type\":\"command\""))
            .count(),
        Err(_) => 0,
    };

    // Calculate duration
    let now = unix_timestamp();
    let duration_secs = now - active_session.started_at;
    let duration_str = format_duration(duration_secs);

    if ctx.output.json {
        let json = serde_json::json!({
            "recording": true,
            "session": {
                "id": active_session.id.to_string(),
                "name": active_session.name,
                "path": active_session.session_path.to_string_lossy()
            },
            "status": {
                "command_count": command_count,
                "duration_seconds": duration_secs,
                "duration_human": duration_str,
                "pid": active_session.pid
            }
        });
        print_json(&json);
    } else {
        ctx.output
            .success(&format!("Recording: {}", active_session.name));
        println!();
        println!("  Session ID:  {}", active_session.id);
        println!("  Commands:    {command_count}");
        println!("  Duration:    {duration_str}");
        println!("  Path:        {}", active_session.session_path.display());
        println!();
        ctx.output.info(&format!(
            "Stop recording with: {}",
            ctx.output.style_command("rec stop")
        ));
    }

    ExitCode::SUCCESS
}
