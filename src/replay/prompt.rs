//! Interactive prompts for replay.
//!
//! Provides step mode, destructive command confirmation, and error
//! recovery prompts using dialoguer. All styling goes to stderr so
//! it doesn't interfere with command output.

use std::io::IsTerminal;

use dialoguer::{Confirm, Select};

/// User action for step mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
    /// Execute this command
    Run,
    /// Skip this command
    Skip,
    /// Abort the entire replay
    Abort,
    /// Run all remaining commands without prompting
    RunAll,
}

/// User action for error recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorAction {
    /// Continue to the next command
    Continue,
    /// Abort the entire replay
    Abort,
    /// Retry the failed command
    Retry,
}

/// Prompt the user for a step-mode action.
///
/// Shows the command with its index and total count, with a warning
/// prefix if the command is destructive. Returns the user's chosen
/// action.
///
/// On error or interrupt, returns `StepAction::Abort`.
#[must_use]
pub fn prompt_step(command: &str, index: usize, total: usize, is_destructive: bool) -> StepAction {
    let prefix = if is_destructive {
        "\x1b[31;1m\u{26a0}\x1b[0m "
    } else {
        ""
    };

    let prompt = format!(
        "{}[{}/{}] \x1b[1m$ {}\x1b[0m",
        prefix,
        index + 1,
        total,
        command
    );
    eprintln!("{prompt}");

    let items = &["Run", "Skip", "Abort", "Run all remaining"];
    let selection = Select::new()
        .with_prompt("Action")
        .items(items)
        .default(0)
        .interact();

    match selection {
        Ok(0) => StepAction::Run,
        Ok(1) => StepAction::Skip,
        Ok(3) => StepAction::RunAll,
        _ => StepAction::Abort,
    }
}

/// Prompt for confirmation before executing a destructive command.
///
/// Prints a styled warning block to stderr with the command and
/// reason for flagging. Returns `true` to execute, `false` to skip.
///
/// On error, returns `false` (safe default).
#[must_use]
pub fn prompt_destructive(command: &str, reason: &str) -> bool {
    eprintln!();
    eprintln!("  \x1b[31;1m\u{26a0} Destructive command detected\x1b[0m");
    eprintln!("  Command: \x1b[1m{command}\x1b[0m");
    eprintln!("  Reason:  {reason}");
    eprintln!();

    Confirm::new()
        .with_prompt("Execute this command?")
        .default(false)
        .interact()
        .unwrap_or(false)
}

/// Prompt for error recovery after a command fails.
///
/// Prints a styled error block to stderr with the exit code and
/// command text. Returns the user's chosen recovery action.
///
/// On error, returns `ErrorAction::Abort`.
#[must_use]
pub fn prompt_error(command: &str, exit_code: Option<i32>) -> ErrorAction {
    let code_str = exit_code.map_or_else(|| "unknown".to_string(), |c| c.to_string());

    eprintln!();
    eprintln!("  \x1b[31m\u{2717} Command failed\x1b[0m (exit code: {code_str})");
    eprintln!("  $ {command}");
    eprintln!();

    let items = &["Continue", "Abort", "Retry"];
    let selection = Select::new()
        .with_prompt("What would you like to do?")
        .items(items)
        .default(0)
        .interact();

    match selection {
        Ok(0) => ErrorAction::Continue,
        Ok(2) => ErrorAction::Retry,
        _ => ErrorAction::Abort,
    }
}

/// Check if stdin is an interactive terminal.
///
/// Used by the replay engine to decide whether interactive prompts
/// are available. In non-interactive mode (piped input), prompts
/// should be skipped or the operation should error.
#[must_use]
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
}
