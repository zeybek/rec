//! Individual diagnostic check functions.
//!
//! Each function performs a single diagnostic check and returns a [`CheckResult`].

use crate::cli::Shell;
use crate::config::ConfigLoader;
use crate::storage::Paths;
use std::fs;
use std::path::PathBuf;

/// Status of a diagnostic check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    /// Check passed successfully.
    Pass,
    /// Check passed with a non-critical warning.
    Warn,
    /// Check failed — action needed.
    Fail,
}

impl CheckStatus {
    /// Return the status as a lowercase string for JSON output.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "fail",
        }
    }
}

/// Result of a single diagnostic check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Human-readable name of the check.
    pub name: &'static str,
    /// Pass, warn, or fail.
    pub status: CheckStatus,
    /// Descriptive message about the check outcome.
    pub message: String,
    /// Actionable fix hint shown on failure.
    pub fix_hint: Option<String>,
}

/// Report the rec binary version (always Pass, informational).
#[must_use]
pub fn check_rec_version() -> CheckResult {
    let version = env!("CARGO_PKG_VERSION");
    CheckResult {
        name: "rec version",
        status: CheckStatus::Pass,
        message: version.to_string(),
        fix_hint: None,
    }
}

/// Verify the `rec` binary is in a standard PATH location.
#[must_use]
pub fn check_rec_in_path() -> CheckResult {
    match std::env::current_exe() {
        Ok(exe) => {
            let exe_dir = exe.parent().map(std::path::Path::to_path_buf);
            let in_path = exe_dir
                .as_ref()
                .and_then(|dir| {
                    std::env::var("PATH")
                        .ok()
                        .map(|path_var| std::env::split_paths(&path_var).any(|p| p == *dir))
                })
                .unwrap_or(false);

            if in_path {
                CheckResult {
                    name: "rec in PATH",
                    status: CheckStatus::Pass,
                    message: exe.display().to_string(),
                    fix_hint: None,
                }
            } else {
                let dir_display = exe_dir
                    .as_ref()
                    .map_or_else(|| "(unknown)".to_string(), |d| d.display().to_string());
                CheckResult {
                    name: "rec in PATH",
                    status: CheckStatus::Warn,
                    message: format!("{} is not in a PATH directory", exe.display()),
                    fix_hint: Some(format!("Add {dir_display} to your PATH")),
                }
            }
        }
        Err(e) => CheckResult {
            name: "rec in PATH",
            status: CheckStatus::Fail,
            message: format!("Could not determine binary location: {e}"),
            fix_hint: Some("Ensure rec is installed correctly".to_string()),
        },
    }
}

/// Detect the current shell via `Shell::detect()`.
#[must_use]
pub fn check_shell_detected() -> CheckResult {
    if let Some(shell) = Shell::detect() {
        CheckResult {
            name: "Shell detected",
            status: CheckStatus::Pass,
            message: shell.name().to_string(),
            fix_hint: None,
        }
    } else {
        let shell_var = std::env::var("SHELL").unwrap_or_default();
        let message = if shell_var.is_empty() {
            "$SHELL not set".to_string()
        } else {
            format!("Unknown shell: {shell_var}")
        };
        CheckResult {
            name: "Shell detected",
            status: CheckStatus::Warn,
            message,
            fix_hint: Some("Set $SHELL to bash, zsh, or fish".to_string()),
        }
    }
}

/// Check if shell hooks are installed by scanning RC files for `rec init`.
#[must_use]
pub fn check_shell_hooks_installed() -> CheckResult {
    let Some(shell) = Shell::detect() else {
        return CheckResult {
            name: "Shell hooks",
            status: CheckStatus::Warn,
            message: "Cannot check hooks — shell not detected".to_string(),
            fix_hint: Some("Detect your shell first (set $SHELL)".to_string()),
        };
    };

    let home = match directories::BaseDirs::new() {
        Some(base) => base.home_dir().to_path_buf(),
        None => {
            return CheckResult {
                name: "Shell hooks",
                status: CheckStatus::Fail,
                message: "Cannot determine home directory".to_string(),
                fix_hint: Some("Set $HOME environment variable".to_string()),
            };
        }
    };

    // Candidate RC files per shell
    let rc_candidates: Vec<PathBuf> = match shell {
        Shell::Bash => vec![
            home.join(".bashrc"),
            home.join(".bash_profile"),
            home.join(".profile"),
        ],
        Shell::Zsh => vec![home.join(".zshrc"), home.join(".zprofile")],
        Shell::Fish => vec![home.join(".config/fish/config.fish")],
    };

    for rc_path in &rc_candidates {
        if let Ok(contents) = fs::read_to_string(rc_path) {
            if contents.contains("rec init") {
                return CheckResult {
                    name: "Shell hooks",
                    status: CheckStatus::Pass,
                    message: format!("found in {}", rc_path.display()),
                    fix_hint: None,
                };
            }
        }
    }

    let rc_file = shell.rc_file();
    CheckResult {
        name: "Shell hooks",
        status: CheckStatus::Fail,
        message: format!("not found in {rc_file}"),
        fix_hint: Some(format!(
            "Add to {}: eval \"$(rec init {})\"",
            rc_file,
            shell.name()
        )),
    }
}

/// Validate the configuration file via `ConfigLoader`.
#[must_use]
pub fn check_config_valid() -> CheckResult {
    let paths = Paths::new();
    let loader = ConfigLoader::new(paths.clone());

    if !paths.config_file.exists() {
        return CheckResult {
            name: "Config file",
            status: CheckStatus::Pass,
            message: "no config file (using defaults)".to_string(),
            fix_hint: None,
        };
    }

    match loader.load() {
        Ok(_) => CheckResult {
            name: "Config file",
            status: CheckStatus::Pass,
            message: "valid".to_string(),
            fix_hint: None,
        },
        Err(e) => CheckResult {
            name: "Config file",
            status: CheckStatus::Fail,
            message: format!("parse error: {e}"),
            fix_hint: Some(format!(
                "Fix {} or delete it to use defaults",
                paths.config_file.display()
            )),
        },
    }
}

/// Check if the data directory exists.
#[must_use]
pub fn check_storage_dir_exists() -> CheckResult {
    let paths = Paths::new();

    if paths.data_dir.exists() {
        CheckResult {
            name: "Storage directory",
            status: CheckStatus::Pass,
            message: paths.data_dir.display().to_string(),
            fix_hint: None,
        }
    } else {
        CheckResult {
            name: "Storage directory",
            status: CheckStatus::Warn,
            message: format!("{} does not exist yet", paths.data_dir.display()),
            fix_hint: Some(format!(
                "Will be created on first recording. Or run: mkdir -p {}",
                paths.data_dir.display()
            )),
        }
    }
}

/// Check if the data directory is writable by creating and removing a test file.
#[must_use]
pub fn check_storage_writable() -> CheckResult {
    let paths = Paths::new();

    if !paths.data_dir.exists() {
        // Try to create it
        if let Err(e) = fs::create_dir_all(&paths.data_dir) {
            return CheckResult {
                name: "Storage writable",
                status: CheckStatus::Fail,
                message: format!("Cannot create {}: {}", paths.data_dir.display(), e),
                fix_hint: Some(format!(
                    "Check permissions on parent directory of {}",
                    paths.data_dir.display()
                )),
            };
        }
    }

    let test_file = paths.data_dir.join(".write-test");
    match fs::write(&test_file, "test") {
        Ok(()) => {
            let _ = fs::remove_file(&test_file);
            CheckResult {
                name: "Storage writable",
                status: CheckStatus::Pass,
                message: "writable".to_string(),
                fix_hint: None,
            }
        }
        Err(e) => CheckResult {
            name: "Storage writable",
            status: CheckStatus::Fail,
            message: format!("Cannot write to {}: {}", paths.data_dir.display(), e),
            fix_hint: Some(format!(
                "Fix permissions: chmod u+w {}",
                paths.data_dir.display()
            )),
        },
    }
}

/// Check if the shell RC file is writable.
#[must_use]
pub fn check_rc_file_writable() -> CheckResult {
    let Some(shell) = Shell::detect() else {
        return CheckResult {
            name: "RC file writable",
            status: CheckStatus::Warn,
            message: "Cannot check — shell not detected".to_string(),
            fix_hint: Some("Detect your shell first (set $SHELL)".to_string()),
        };
    };

    let home = match directories::BaseDirs::new() {
        Some(base) => base.home_dir().to_path_buf(),
        None => {
            return CheckResult {
                name: "RC file writable",
                status: CheckStatus::Warn,
                message: "Cannot determine home directory".to_string(),
                fix_hint: Some("Set $HOME environment variable".to_string()),
            };
        }
    };

    // Resolve ~ in rc_file path
    let rc_path = match shell {
        Shell::Bash => home.join(".bashrc"),
        Shell::Zsh => home.join(".zshrc"),
        Shell::Fish => home.join(".config/fish/config.fish"),
    };

    if !rc_path.exists() {
        return CheckResult {
            name: "RC file writable",
            status: CheckStatus::Pass,
            message: format!("{} does not exist (will be created)", rc_path.display()),
            fix_hint: None,
        };
    }

    // Check write permission via metadata
    match fs::metadata(&rc_path) {
        Ok(meta) => {
            if meta.permissions().readonly() {
                CheckResult {
                    name: "RC file writable",
                    status: CheckStatus::Warn,
                    message: format!("{} is read-only", rc_path.display()),
                    fix_hint: Some(format!("Fix permissions: chmod u+w {}", rc_path.display())),
                }
            } else {
                CheckResult {
                    name: "RC file writable",
                    status: CheckStatus::Pass,
                    message: format!("{}", rc_path.display()),
                    fix_hint: None,
                }
            }
        }
        Err(e) => CheckResult {
            name: "RC file writable",
            status: CheckStatus::Warn,
            message: format!("Cannot read metadata for {}: {}", rc_path.display(), e),
            fix_hint: Some(format!("Check permissions on {}", rc_path.display())),
        },
    }
}

/// Verify the data directory has correct read+write permissions.
#[must_use]
pub fn check_data_dir_permissions() -> CheckResult {
    let paths = Paths::new();

    if !paths.data_dir.exists() {
        return CheckResult {
            name: "Data dir permissions",
            status: CheckStatus::Pass,
            message: "directory not yet created (OK)".to_string(),
            fix_hint: None,
        };
    }

    match fs::metadata(&paths.data_dir) {
        Ok(meta) => {
            if meta.permissions().readonly() {
                CheckResult {
                    name: "Data dir permissions",
                    status: CheckStatus::Fail,
                    message: format!("{} is read-only", paths.data_dir.display()),
                    fix_hint: Some(format!(
                        "Fix permissions: chmod u+rwx {}",
                        paths.data_dir.display()
                    )),
                }
            } else {
                CheckResult {
                    name: "Data dir permissions",
                    status: CheckStatus::Pass,
                    message: "read/write OK".to_string(),
                    fix_hint: None,
                }
            }
        }
        Err(e) => CheckResult {
            name: "Data dir permissions",
            status: CheckStatus::Fail,
            message: format!(
                "Cannot read metadata for {}: {}",
                paths.data_dir.display(),
                e
            ),
            fix_hint: Some(format!("Check permissions on {}", paths.data_dir.display())),
        },
    }
}
