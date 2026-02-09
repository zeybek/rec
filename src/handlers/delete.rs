use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result, print_json};
use rec::error::EXIT_USER_ERROR;
use std::process::ExitCode;

/// Handle the `delete` command — delete a session with confirmation.
pub fn handle_delete(
    ctx: &HandlerContext,
    identifier: Option<&str>,
    force: bool,
    all: bool,
) -> ExitCode {
    use rec::recording::RecordingState;
    use rec::session::resolve_session_with_alias;
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let recording_state = RecordingState::new(&ctx.paths.state_dir);

    // Handle --all flag
    if all {
        return handle_delete_all(ctx, &store, &recording_state, force, interactive);
    }

    // Require session identifier if not --all
    let identifier = match identifier {
        Some(id) => id,
        None => {
            ctx.output.error(
                "Missing session",
                "Provide a session name/ID or use --all to delete all sessions",
                None,
                Some("Usage: rec delete <SESSION> or rec delete --all"),
            );
            return ExitCode::from(EXIT_USER_ERROR);
        }
    };

    let session = match resolve_session_with_alias(&store, &alias_store, identifier, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Check if session is currently being recorded
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

/// Handle deletion of all sessions.
fn handle_delete_all(
    ctx: &HandlerContext,
    store: &rec::storage::SessionStore,
    recording_state: &rec::recording::RecordingState,
    force: bool,
    interactive: bool,
) -> ExitCode {
    // Get all session IDs
    let session_ids = match store.list() {
        Ok(s) => s,
        Err(e) => {
            ctx.output
                .error("Failed to list sessions", &e.to_string(), None, None);
            return ExitCode::from(EXIT_USER_ERROR);
        }
    };

    if session_ids.is_empty() {
        ctx.output.info("No sessions to delete");
        return ExitCode::SUCCESS;
    }

    // Check if any session is currently being recorded
    let active_session_id = if recording_state.is_recording() {
        recording_state.current().ok().map(|a| {
            a.session_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        })
    } else {
        None
    };

    // Filter out active session
    let deletable: Vec<_> = session_ids
        .iter()
        .filter(|id| {
            active_session_id
                .as_ref()
                .map_or(true, |active_id| *id != active_id)
        })
        .collect();

    let skipped = session_ids.len() - deletable.len();

    if deletable.is_empty() {
        ctx.output
            .warning("All sessions are currently being recorded, nothing to delete");
        return ExitCode::SUCCESS;
    }

    // Confirmation prompt
    if !force {
        if interactive {
            let prompt = if skipped > 0 {
                format!(
                    "Delete {} sessions? ({} active session will be skipped) This cannot be undone.",
                    deletable.len(),
                    skipped
                )
            } else {
                format!(
                    "Delete ALL {} sessions? This cannot be undone.",
                    deletable.len()
                )
            };

            let confirm = dialoguer::Confirm::new()
                .with_prompt(prompt)
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
                "Use --force to delete all in non-interactive mode",
                None,
                None,
            );
            return ExitCode::from(EXIT_USER_ERROR);
        }
    }

    // Delete all sessions
    let mut deleted_count = 0;
    let mut failed_count = 0;

    for session_id in &deletable {
        match store.delete(session_id) {
            Ok(()) => deleted_count += 1,
            Err(_) => failed_count += 1,
        }
    }

    // Output result
    if ctx.output.json {
        let json = serde_json::json!({
            "status": "deleted",
            "deleted": deleted_count,
            "failed": failed_count,
            "skipped": skipped
        });
        print_json(&json);
    } else if failed_count > 0 {
        ctx.output.warning(&format!(
            "Deleted {} sessions, {} failed, {} skipped",
            deleted_count, failed_count, skipped
        ));
    } else if skipped > 0 {
        ctx.output.success(&format!(
            "Deleted {} sessions ({} active session skipped)",
            deleted_count, skipped
        ));
    } else {
        ctx.output
            .success(&format!("Deleted all {} sessions", deleted_count));
    }

    ExitCode::SUCCESS
}
