//! Command execution for replay.
//!
//! Executes recorded commands through the shell via `sh -c` with
//! inherited stdio for real-time output.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Result of command execution, including whether it was a cd command.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Exit status of the command
    pub status: std::process::ExitStatus,
    /// If this was a cd command, the new directory to change to
    pub new_cwd: Option<PathBuf>,
}

/// Check if a command is a `cd` command and extract the target directory.
///
/// Returns `Some(path)` if the command is a simple `cd <path>` command,
/// `None` otherwise (e.g., `cd && ls`, `cd; echo`, complex commands).
fn parse_cd_command(command_text: &str) -> Option<&str> {
    let trimmed = command_text.trim();

    // Must start with "cd "
    if !trimmed.starts_with("cd ") {
        return None;
    }

    // Check for command chaining that would make this not a simple cd
    // e.g., "cd foo && ls", "cd foo; echo", "cd foo | cat"
    if trimmed.contains("&&")
        || trimmed.contains("||")
        || trimmed.contains(';')
        || trimmed.contains('|')
    {
        return None;
    }

    // Extract the path after "cd "
    let path = trimmed.strip_prefix("cd ")?.trim();

    // Handle quoted paths
    let path = path
        .strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .or_else(|| path.strip_prefix('\'').and_then(|p| p.strip_suffix('\'')))
        .unwrap_or(path);

    if path.is_empty() {
        return None;
    }

    Some(path)
}

/// Resolve a cd target path to an absolute path.
///
/// Handles:
/// - `~` → home directory
/// - `-` → previous directory (not supported, returns None)
/// - Relative paths → resolved against current directory
/// - Absolute paths → used as-is
fn resolve_cd_path(target: &str, current_dir: &Path) -> Option<PathBuf> {
    if target == "-" {
        // OLDPWD not tracked, let shell handle it
        return None;
    }

    let expanded = if target.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            if target == "~" {
                home_path
            } else if let Some(rest) = target.strip_prefix("~/") {
                home_path.join(rest)
            } else {
                // ~user syntax - let shell handle it
                return None;
            }
        } else {
            return None;
        }
    } else if Path::new(target).is_absolute() {
        PathBuf::from(target)
    } else {
        current_dir.join(target)
    };

    // Canonicalize to resolve .. and symlinks
    expanded.canonicalize().ok()
}

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
/// For simple `cd <path>` commands, returns the resolved new directory
/// in `ExecutionResult::new_cwd` so the caller can update its working
/// directory for subsequent commands.
///
/// # Errors
///
/// Returns an error if the shell process cannot be spawned or waited on.
pub fn execute_command(command_text: &str, cwd: Option<&Path>) -> std::io::Result<ExecutionResult> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(command_text);

    let effective_cwd = if let Some(dir) = cwd {
        if dir.exists() {
            cmd.current_dir(dir);
            dir.to_path_buf()
        } else {
            eprintln!(
                "\x1b[33m  Warning: directory '{}' does not exist, using current directory\x1b[0m",
                dir.display()
            );
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status()?;

    // If command succeeded and it's a simple cd, resolve the new directory
    let new_cwd = if status.success() {
        if let Some(target) = parse_cd_command(command_text) {
            resolve_cd_path(target, &effective_cwd)
        } else {
            None
        }
    } else {
        None
    };

    Ok(ExecutionResult { status, new_cwd })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cd_simple() {
        assert_eq!(parse_cd_command("cd foo"), Some("foo"));
        assert_eq!(parse_cd_command("cd /tmp"), Some("/tmp"));
        assert_eq!(parse_cd_command("cd ~/Code"), Some("~/Code"));
        assert_eq!(parse_cd_command("cd .."), Some(".."));
    }

    #[test]
    fn test_parse_cd_quoted() {
        assert_eq!(parse_cd_command("cd \"my folder\""), Some("my folder"));
        assert_eq!(parse_cd_command("cd 'my folder'"), Some("my folder"));
    }

    #[test]
    fn test_parse_cd_chained_returns_none() {
        assert_eq!(parse_cd_command("cd foo && ls"), None);
        assert_eq!(parse_cd_command("cd foo; echo"), None);
        assert_eq!(parse_cd_command("cd foo || true"), None);
        assert_eq!(parse_cd_command("cd foo | cat"), None);
    }

    #[test]
    fn test_parse_cd_not_cd() {
        assert_eq!(parse_cd_command("echo cd foo"), None);
        assert_eq!(parse_cd_command("ls"), None);
        assert_eq!(parse_cd_command("cdfoo"), None);
    }

    #[test]
    fn test_resolve_cd_path_absolute() {
        let current = PathBuf::from("/home/user");
        // /tmp should exist on most systems
        let resolved = resolve_cd_path("/tmp", &current);
        assert!(resolved.is_some());
        assert!(resolved.unwrap().is_absolute());
    }

    #[test]
    fn test_resolve_cd_path_home() {
        let current = PathBuf::from("/tmp");
        if std::env::var("HOME").is_ok() {
            let resolved = resolve_cd_path("~", &current);
            assert!(resolved.is_some());
        }
    }

    #[test]
    fn test_resolve_cd_path_dash_returns_none() {
        let current = PathBuf::from("/tmp");
        assert_eq!(resolve_cd_path("-", &current), None);
    }

    #[test]
    fn test_execute_echo_command() {
        let result = execute_command("echo hello", None).unwrap();
        assert!(result.status.success());
        assert!(result.new_cwd.is_none());
    }

    #[test]
    fn test_execute_cd_returns_new_cwd() {
        let result = execute_command("cd /tmp", None).unwrap();
        assert!(result.status.success());
        assert!(result.new_cwd.is_some());
        // On macOS, /tmp is a symlink to /private/tmp, so we canonicalize for comparison
        let expected = std::fs::canonicalize("/tmp").unwrap_or_else(|_| PathBuf::from("/tmp"));
        assert_eq!(result.new_cwd.unwrap(), expected);
    }

    #[test]
    fn test_execute_cd_nonexistent_no_new_cwd() {
        let result = execute_command("cd /nonexistent_dir_12345", None).unwrap();
        assert!(!result.status.success());
        assert!(result.new_cwd.is_none());
    }
}
