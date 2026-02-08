use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result};
use std::process::ExitCode;

/// Handle the `diff` command — compares commands between two sessions.
pub fn handle_diff(ctx: &HandlerContext, session1: &str, session2: &str) -> ExitCode {
    use rec::session::{diff_sessions, resolve_session_with_alias};
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();

    let s1 = match resolve_session_with_alias(&store, &alias_store, session1, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };
    let s2 = match resolve_session_with_alias(&store, &alias_store, session2, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    let result = diff_sessions(&s1, &s2, ctx.output.json, &ctx.output);
    handle_result(result, ctx.output, &ctx.paths)
}
