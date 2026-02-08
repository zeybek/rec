use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result};
use std::process::ExitCode;

/// Handle the `show` command — display details of a session.
pub fn handle_show(ctx: &HandlerContext, identifier: &str, grep: Option<&String>) -> ExitCode {
    use rec::session::{resolve_session_with_alias, show_session};
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match resolve_session_with_alias(&store, &alias_store, identifier, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    let result = show_session(
        &session,
        grep.map(String::as_str),
        ctx.output.json,
        &ctx.output,
    );
    handle_result(result, ctx.output, &ctx.paths)
}
