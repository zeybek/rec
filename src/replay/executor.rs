//! Command execution for replay.
//!
//! Executes recorded commands through the shell via `sh -c` with
//! inherited stdio for real-time output.

use std::process::{Command, Stdio};

/// Execute a command string through the shell.
///
/// Uses the `SHELL` environment variable (fallback `/bin/sh`) to run
/// the command via `sh -c`. Stdin, stdout, and stderr are inherited so
/// the user sees output in real-time and can interact with prompts.
///
/// If `cwd` is provided and the directory exists, the command runs in
/// that directory. If the directory doesn't exist, a warning is printed
/// to stderr and the current directory is used instead.
///
/// # Errors
///
/// Returns an error if the shell process cannot be spawned or waited on.
pub fn execute_command(
    command_text: &str,
    cwd: Option<&std::path::Path>,
) -> std::io::Result<std::process::ExitStatus> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(command_text);

    if let Some(dir) = cwd {
        if dir.exists() {
            cmd.current_dir(dir);
        } else {
            eprintln!(
                "\x1b[33m  Warning: directory '{}' does not exist, using current directory\x1b[0m",
                dir.display()
            );
        }
    }

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.status()
}
