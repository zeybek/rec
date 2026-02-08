use super::HandlerContext;
use super::common::handle_result;
use std::process::ExitCode;

/// Handle the `search` command — searches across all sessions.
pub fn handle_search(ctx: &HandlerContext, pattern: &str, regex: bool, tag: &[String]) -> ExitCode {
    use rec::session::search_sessions;
    use rec::storage::SessionStore;

    let store = SessionStore::new(ctx.paths.clone());
    let result = search_sessions(&store, pattern, regex, tag, ctx.output.json, &ctx.output);
    handle_result(result, ctx.output, &ctx.paths)
}
