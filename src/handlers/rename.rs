use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result, print_json};
use rec::error::{EXIT_SYSTEM_ERROR, EXIT_USER_ERROR};
use std::process::ExitCode;

/// Handle the `rename` command — rename a session with validation and collision detection.
pub fn handle_rename(ctx: &HandlerContext, old: &str, new: &str) -> ExitCode {
    use rec::models::validate_session_name;
    use rec::recording::RecordingState;
    use rec::session::resolve_session_with_alias;
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match resolve_session_with_alias(&store, &alias_store, old, interactive) {
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

    // Validate new name
    if let Err(e) = validate_session_name(new) {
        ctx.output
            .error("Invalid session name", &e.to_string(), None, None);
        return ExitCode::from(EXIT_USER_ERROR);
    }

    // Check for name collision
    let all_ids = match store.list() {
        Ok(ids) => ids,
        Err(e) => {
            ctx.output
                .error("Failed to list sessions", &e.to_string(), None, None);
            return ExitCode::from(EXIT_SYSTEM_ERROR);
        }
    };
    for id in &all_ids {
        if let Ok(s) = store.load(id) {
            if s.name() == new && s.id() != session.id() {
                ctx.output.error(
                    "Name already exists",
                    &format!("A session named '{new}' already exists. Choose a different name."),
                    None,
                    None,
                );
                return ExitCode::from(EXIT_USER_ERROR);
            }
        }
    }

    // Rename the session
    let session_id = session.id().to_string();
    let old_name = session.name().to_string();

    let result = match store.rename(&session_id, new) {
        Ok(()) => {
            if ctx.output.json {
                let json = serde_json::json!({
                    "status": "renamed",
                    "session": {
                        "id": session_id,
                        "old_name": old_name,
                        "new_name": new
                    }
                });
                print_json(&json);
            } else {
                ctx.output
                    .success(&format!("Renamed '{old_name}' to '{new}'"));
            }
            Ok(())
        }
        Err(e) => {
            let backup_path = ctx.paths.backup_file(&session_id);
            if backup_path.exists() {
                ctx.output.info(&format!(
                    "note: Backup preserved at {}",
                    backup_path.display()
                ));
            }
            Err(e)
        }
    };
    handle_result(result, ctx.output, &ctx.paths)
}
