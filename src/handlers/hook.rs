use super::HandlerContext;
use super::common::{PendingCommand, unix_timestamp};
use rec::cli::HookType;
use rec::recording::{CommandCapture, RecordingState};
use rec::storage::set_restrictive_permissions;
use std::process::ExitCode;

pub fn handle_hook(ctx: &HandlerContext, hook_type: &HookType, arg: &str) -> ExitCode {
    let recording_state = RecordingState::new(&ctx.paths.state_dir);

    // Quick check - if not recording, exit silently
    if !recording_state.is_recording() {
        return ExitCode::SUCCESS;
    }

    let Ok(active_session) = recording_state.current() else {
        return ExitCode::SUCCESS; // Silent exit if state unavailable
    };

    match hook_type {
        HookType::Preexec => {
            // Store pending command info for precmd to complete
            let started_at = unix_timestamp();

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

            let pending = PendingCommand {
                command: arg.to_owned(),
                cwd,
                started_at,
            };

            // Write pending command to state directory
            let pending_path = ctx.paths.state_dir.join("pending.json");
            if let Ok(file) = std::fs::File::create(&pending_path) {
                let _ = serde_json::to_writer(file, &pending);
                // Set restrictive permissions (0o600) to prevent other users from reading
                let _ = set_restrictive_permissions(&pending_path);
            }
        }
        HookType::Precmd => {
            // Read pending command and complete it
            let pending_path = ctx.paths.state_dir.join("pending.json");

            let pending: Option<PendingCommand> = std::fs::File::open(&pending_path)
                .ok()
                .and_then(|f| serde_json::from_reader(f).ok());

            // Clean up pending file
            let _ = std::fs::remove_file(&pending_path);

            if let Some(pending) = pending {
                let ended_at = unix_timestamp();

                let exit_code: i32 = arg.parse().unwrap_or(0);
                let duration_ms = ((ended_at - pending.started_at) * 1000.0) as u64;

                // Count existing commands by reading NDJSON file directly
                let command_index = match std::fs::read_to_string(&active_session.session_path) {
                    Ok(contents) => contents
                        .lines()
                        .filter(|line| line.contains("\"type\":\"command\""))
                        .count() as u32,
                    Err(_) => 0,
                };

                // Create command with all metadata
                let cmd = rec::models::Command {
                    index: command_index,
                    command: pending.command,
                    cwd: pending.cwd,
                    started_at: pending.started_at,
                    ended_at: Some(ended_at),
                    exit_code: Some(exit_code),
                    duration_ms: Some(duration_ms),
                };

                let _ = CommandCapture::append_command(&active_session.session_path, &cmd);
            }
        }
    }

    ExitCode::SUCCESS
}
