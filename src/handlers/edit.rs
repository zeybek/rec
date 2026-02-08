use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result};
use rec::error::EXIT_USER_ERROR;
use std::process::ExitCode;

/// Handle the `edit` command — open a session in `$EDITOR`.
pub fn handle_edit(ctx: &HandlerContext, identifier: &str) -> ExitCode {
    use rec::recording::RecordingState;
    use rec::session::{edit_session, resolve_session_with_alias};
    use rec::storage::{AliasStore, SessionStore};

    if ctx.output.json {
        ctx.output.error(
            "Not supported",
            "Edit requires an interactive terminal and cannot run in JSON mode",
            None,
            Some("Run without --json flag"),
        );
        return ExitCode::from(EXIT_USER_ERROR);
    }

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

    let edit_result = edit_session(&store, &session, &ctx.output);
    if edit_result.is_err() {
        let session_id = session.id().to_string();
        let source = store.session_file_path(&session_id);
        let backup = source.with_extension("ndjson.bak");
        if backup.exists() {
            ctx.output
                .info(&format!("note: Backup preserved at {}", backup.display()));
        }
    }
    handle_result(edit_result, ctx.output, &ctx.paths)
}
