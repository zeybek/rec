//! Handler for the `ui` command.

use super::HandlerContext;
use std::process::ExitCode;

/// Handle the `ui` command — launch interactive TUI.
#[cfg(feature = "tui")]
pub fn handle_ui(ctx: &HandlerContext) -> ExitCode {
    match rec::tui::run(&ctx.paths) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("TUI error: {e}");
            ExitCode::from(2)
        }
    }
}

/// Handle the `ui` command when TUI feature is not enabled.
#[cfg(not(feature = "tui"))]
pub fn handle_ui(_ctx: &HandlerContext) -> ExitCode {
    eprintln!("TUI is not available in this build.");
    eprintln!();
    eprintln!("To enable TUI, reinstall with the tui feature:");
    eprintln!("  cargo install rec-cli --features tui");
    ExitCode::from(1)
}
