use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result, print_json};
use rec::error::EXIT_USER_ERROR;
use std::process::ExitCode;

/// Handle the `delete` command — delete a session with confirmation.
pub fn handle_delete(ctx: &HandlerContext, identifier: &str, force: bool) -> ExitCode {
    use rec::recording::RecordingState;
    use rec::session::resolve_session_with_alias;
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match resolve_session_with_alias(&store, &alias_store, identifier, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Check if session is currently being recorded
    let recording_state = RecordingState::new(&ctx.paths.state_dir);
    if recording_state.is_recording() {
        if let Ok(active) = recording_state.current() {
            let session_path = ctx.paths.session_file(&session.id().to_string());
            if active.session_path == session_path {
                ctx.output.error(
                    "Cannot modify session",
                    "Cannot modify session while recording is in progress",
                    None,
                    Some("Stop recording first with: rec stop"),
                );
                return ExitCode::from(EXIT_USER_ERROR);
            }
        }
    }

    // Confirmation prompt
    if !force {
        if interactive {
            let confirm = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "Delete session '{}'? This cannot be undone.",
                    session.name()
                ))
                .default(false)
                .interact()
                .unwrap_or(false);

            if !confirm {
                ctx.output.info("Delete cancelled");
                return ExitCode::SUCCESS;
            }
        } else {
            ctx.output.error(
                "Cannot confirm deletion",
                "Use --force to delete in non-interactive mode",
                None,
                None,
            );
            return ExitCode::from(EXIT_USER_ERROR);
        }
    }

    // Delete the session
    let session_id = session.id().to_string();
    let session_name = session.name().to_string();

    let result = match store.delete(&session_id) {
        Ok(()) => {
            if ctx.output.json {
                let json = serde_json::json!({
                    "status": "deleted",
                    "session": {
                        "id": session_id,
                        "name": session_name
                    }
                });
                print_json(&json);
            } else {
                ctx.output
                    .success(&format!("Deleted session '{session_name}'"));
            }
            Ok(())
        }
        Err(e) => Err(e),
    };
    handle_result(result, ctx.output, &ctx.paths)
}
