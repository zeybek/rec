use super::HandlerContext;
use super::common::{cleanup_stale_lock_with_warning, handle_error_with_context, print_json};
use rec::error::EXIT_USER_ERROR;
use rec::models::{Session, generate_session_name, validate_session_name};
use rec::recording::{CommandCapture, RecordingState};
use rec::storage::SessionStore;
use std::process::ExitCode;

/// Handle the `start` command — begin a new recording session.
///
/// Creates a new session file and activates recording state. If a session with
/// the given name already exists, auto-suffixes with a number (e.g., `foo-2`).
///
/// # Errors
/// Returns exit code 1 if already recording or name is invalid.
/// Returns exit code 2 on I/O failures.
pub fn handle_start(ctx: &HandlerContext, name: Option<&String>) -> ExitCode {
    let recording_state = RecordingState::new(&ctx.paths.state_dir);

    // Detect and clean up stale locks BEFORE checking is_recording
    let store_for_recovery = SessionStore::new(ctx.paths.clone());
    cleanup_stale_lock_with_warning(&recording_state, Some(&store_for_recovery));

    // Check if already recording
    if recording_state.is_recording() {
        if let Ok(current) = recording_state.current() {
            ctx.output.error(
                "Already recording",
                &format!("Session '{}' is in progress", current.name),
                None,
                Some("Stop current recording with: rec stop"),
            );
        } else {
            ctx.output.error(
                "Already recording",
                "A recording session is in progress",
                None,
                Some("Stop current recording with: rec stop"),
            );
        }
        return ExitCode::from(EXIT_USER_ERROR);
    }

    // Determine session name
    let mut session_name = match name {
        Some(n) => {
            if let Err(e) = validate_session_name(n) {
                return handle_error_with_context(&e, ctx.output, "Invalid session name", None);
            }
            n.clone()
        }
        None => generate_session_name(),
    };

    // Auto-suffix if name already exists
    let all_ids = store_for_recovery.list().unwrap_or_default();
    let mut existing_count = 0u32;
    for id in &all_ids {
        if let Ok(s) = store_for_recovery.load(id) {
            if s.name() == session_name {
                existing_count += 1;
            }
        }
    }
    if existing_count > 0 {
        let suffixed = format!("{}-{}", session_name, existing_count + 1);
        ctx.output.warning(&format!(
            "Session '{session_name}' already exists. Using '{suffixed}' instead"
        ));
        session_name = suffixed;
    }

    // Create session
    let session = Session::new(&session_name);
    let session_id = session.id().to_string();
    let session_path = ctx.paths.session_file(&session_id);

    // Write header to session file
    if let Err(e) = CommandCapture::write_header(&session_path, &session.header) {
        return handle_error_with_context(&e, ctx.output, "Failed to write session header", None);
    }

    // Activate recording state
    if let Err(e) = recording_state.start(&session_name, session_path.clone()) {
        // Clean up the session file we created
        let _ = std::fs::remove_file(&session_path);
        return handle_error_with_context(&e, ctx.output, "Failed to start recording", None);
    }

    // Output success
    if ctx.output.json {
        let json = serde_json::json!({
            "status": "recording",
            "session": {
                "id": session_id,
                "name": session_name,
                "path": session_path.to_string_lossy()
            },
            "instructions": {
                "env": "REC_RECORDING=1",
                "stop": "rec stop"
            }
        });
        print_json(&json);
    } else {
        ctx.output
            .success(&format!("Recording session '{session_name}'"));
        ctx.output
            .info("Set REC_RECORDING=1 in your shell to enable hooks");
        ctx.output.info("Stop recording with: rec stop");
    }

    ExitCode::SUCCESS
}
