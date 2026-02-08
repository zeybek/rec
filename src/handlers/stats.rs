use super::HandlerContext;
use super::common::handle_result;
use std::process::ExitCode;

/// Handle the `stats` command — shows recording statistics.
pub fn handle_stats(ctx: &HandlerContext) -> ExitCode {
    use rec::session::{compute_stats, format_stats};
    use rec::storage::SessionStore;

    let store = SessionStore::new(ctx.paths.clone());
    let result = match compute_stats(&store) {
        Ok(stats) => format_stats(&stats, ctx.output.json, &ctx.output),
        Err(e) => Err(e),
    };
    handle_result(result, ctx.output, &ctx.paths)
}
