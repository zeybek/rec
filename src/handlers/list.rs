use super::HandlerContext;
use super::common::{cleanup_stale_lock_with_warning, handle_result};
use std::process::ExitCode;

/// Handle the `list` command — lists all recorded sessions.
pub fn handle_list(ctx: &HandlerContext, tag: &[String], tag_all: bool) -> ExitCode {
    use rec::recording::RecordingState;
    use rec::session::list_sessions;
    use rec::storage::SessionStore;

    // Detect and warn about stale locks (cleanup only, no recovery)
    let recording_state = RecordingState::new(&ctx.paths.state_dir);
    cleanup_stale_lock_with_warning(&recording_state, None);

    let store = SessionStore::new(ctx.paths.clone());
    let result = list_sessions(&store, tag, tag_all, ctx.output.json, &ctx.output);
    handle_result(result, ctx.output, &ctx.paths)
}
