pub mod alias;
pub mod common;
pub mod completions;
pub mod config;
pub mod copy;
pub mod delete;
pub mod demo;
pub mod diff;
pub mod edit;
pub mod export;
pub mod help;
pub mod hook;
pub mod import;
pub mod init;
pub mod list;
pub mod rename;
pub mod replay;
pub mod search;
pub mod show;
pub mod start;
pub mod stats;
pub mod status;
pub mod stop;
pub mod tag;
pub mod tags;
pub mod ui;

/// Shared context bundling all state needed by command handlers.
///
/// Access output flags via `ctx.output.json`, `ctx.output.is_verbose()`, `ctx.output.is_quiet()`.
pub struct HandlerContext {
    pub paths: rec::storage::Paths,
    pub config: rec::models::Config,
    pub output: rec::cli::Output,
    pub interrupted: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
