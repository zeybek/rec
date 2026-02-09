use crate::replay::DangerPolicy;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use strsim::levenshtein;

/// CLI Terminal Recorder - Record, replay, and export terminal sessions.
///
/// rec captures your terminal commands and lets you replay them later,
/// export them to scripts, or share them as documentation.
#[derive(Parser)]
#[command(name = "rec")]
#[command(
    author,
    version,
    about = "Record, replay, and export terminal sessions",
    after_help = "Exit codes: 0 success, 1 user error, 2 system error, 130 interrupted"
)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Increase output verbosity (show debug messages)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output in JSON format for scripting
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands for managing terminal recordings.
#[derive(Subcommand)]
pub enum Commands {
    /// Start recording a new session
    Start {
        /// Name for the session (auto-generated if not provided)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Stop the current recording
    Stop,

    /// Replay a recorded session
    #[command(name = "replay", alias = "play")]
    Replay {
        /// Session name or ID
        session: String,

        /// Preview commands without executing
        #[arg(long)]
        dry_run: bool,

        /// Execute one command at a time with confirmation
        #[arg(long)]
        step: bool,

        /// Skip specific command indices (comma-separated, 1-based)
        #[arg(long, value_delimiter = ',')]
        skip: Option<Vec<u32>>,

        /// Start from a specific command index (1-based)
        #[arg(long)]
        from: Option<u32>,

        /// Bypass destructive command prompts
        #[arg(long)]
        force: bool,

        /// Glob patterns to skip matching commands (comma-separated)
        #[arg(long, value_delimiter = ',')]
        skip_pattern: Option<Vec<String>>,

        /// Replay in each command's original working directory
        #[arg(long)]
        cwd: bool,

        /// Policy for handling dangerous commands (skip, abort, allow)
        #[arg(long, value_enum)]
        danger_policy: Option<DangerPolicy>,
    },

    /// List all recorded sessions
    List {
        /// Filter by tag (can be specified multiple times)
        #[arg(short, long)]
        tag: Vec<String>,

        /// Require all tags to match (default: any)
        #[arg(long)]
        tag_all: bool,
    },

    /// Show details of a session
    Show {
        /// Session name or ID
        session: String,

        /// Filter commands by regex pattern
        #[arg(long)]
        grep: Option<String>,
    },

    /// Delete a session
    Delete {
        /// Session name or ID (not required with --all)
        session: Option<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,

        /// Delete all sessions
        #[arg(long)]
        all: bool,
    },

    /// Run an interactive walkthrough of rec's features
    Demo,

    /// Diagnose installation and configuration issues
    Doctor,

    /// Copy a session with a new name
    Copy {
        /// Source session name or ID
        source: String,

        /// Name for the copy
        name: String,
    },

    /// Rename a session
    Rename {
        /// Current session name or ID
        old: String,

        /// New session name
        new: String,
    },

    /// Edit a session in $EDITOR
    Edit {
        /// Session name or ID
        session: String,
    },

    /// Add or remove tags on a session
    Tag {
        /// Session name or ID
        session: String,

        /// Tags to add
        #[arg(required = true)]
        tags: Vec<String>,
    },

    /// Manage tags across sessions
    Tags {
        #[command(subcommand)]
        action: Option<TagsAction>,
    },

    /// Export a session to another format
    Export {
        /// Session name or ID
        session: String,

        /// Output format
        #[arg(
            short,
            long,
            value_enum,
            long_help = "\
Output format for the exported session.

Available formats:
  bash            Bash shell script with set -e
  makefile        Makefile with command targets
  markdown        Markdown documentation with code blocks
  github-action   GitHub Actions workflow YAML
  gitlab-ci       GitLab CI/CD pipeline YAML
  dockerfile      Dockerfile with RUN commands
  circleci        CircleCI configuration YAML"
        )]
        format: ExportFormat,

        /// Output file (stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Enable auto-parameterization of values
        #[arg(long)]
        parameterize: bool,

        /// Set parameter values (format: KEY=VALUE, can be repeated)
        #[arg(long = "param", value_name = "KEY=VALUE")]
        params: Vec<String>,
    },

    /// Show current recording status
    Status,

    /// Internal: Handle shell hook events (preexec, precmd)
    ///
    /// This command is called by shell hooks to capture commands.
    /// Not intended for direct user invocation.
    #[command(name = "_hook", hide = true)]
    Hook {
        /// Hook type: preexec or precmd
        #[arg(value_enum)]
        hook_type: HookType,

        /// Argument: command text (preexec) or exit code (precmd)
        arg: String,
    },

    /// Import a session from a bash script or shell history file
    Import {
        /// Path to the file to import
        file: PathBuf,

        /// Session name (auto-generated from filename if not provided)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Initialize shell hooks for automatic recording
    Init {
        /// Shell to initialize (auto-detected if not provided)
        #[arg(value_enum)]
        shell: Option<Shell>,
    },

    /// Show or modify configuration
    Config {
        /// Get a specific config value
        #[arg(long)]
        get: Option<String>,

        /// Set a config value (KEY VALUE)
        #[arg(long, num_args = 2, value_names = ["KEY", "VALUE"])]
        set: Option<Vec<String>>,

        /// Open config file in $EDITOR
        #[arg(long)]
        edit: bool,

        /// Show config file path
        #[arg(long)]
        path: bool,

        /// List all config values with sources
        #[arg(long)]
        list: bool,
    },

    /// Search across all recorded sessions
    Search {
        /// Search pattern (substring or regex with --regex)
        pattern: String,

        /// Use regex pattern matching
        #[arg(long)]
        regex: bool,

        /// Filter by tag before searching
        #[arg(short, long)]
        tag: Vec<String>,
    },

    /// Compare commands between two sessions
    Diff {
        /// First session name or ID
        session1: String,

        /// Second session name or ID
        session2: String,
    },

    /// Show recording statistics
    Stats,

    /// Create or manage named aliases for sessions
    Alias {
        /// Alias name (for create/lookup)
        name: Option<String>,

        /// Target session name or ID (for create)
        session: Option<String>,

        /// List all aliases
        #[arg(long)]
        list: bool,

        /// Remove an alias by name
        #[arg(long)]
        remove: Option<String>,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Launch interactive terminal UI (requires --features tui)
    #[cfg(feature = "tui")]
    Ui,

    /// Launch interactive terminal UI (requires --features tui)
    #[cfg(not(feature = "tui"))]
    Ui,
}

/// Sub-actions for the `tags` command group.
#[derive(Subcommand)]
pub enum TagsAction {
    /// Normalize all existing tags (lowercase, trim, hyphenate)
    Normalize,
}

/// Available export formats for sessions.
#[derive(Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    /// Bash shell script with set -e
    Bash,
    /// Makefile with command targets
    Makefile,
    /// Markdown documentation with code blocks
    Markdown,
    /// GitHub Actions workflow YAML
    GithubAction,
    /// GitLab CI/CD pipeline YAML
    GitlabCi,
    /// Dockerfile with RUN commands
    Dockerfile,
    /// `CircleCI` configuration YAML
    Circleci,
}

impl ExportFormat {
    /// Returns a one-line description for this format.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            ExportFormat::Bash => "Bash shell script with set -e",
            ExportFormat::Makefile => "Makefile with command targets",
            ExportFormat::Markdown => "Markdown documentation with code blocks",
            ExportFormat::GithubAction => "GitHub Actions workflow YAML",
            ExportFormat::GitlabCi => "GitLab CI/CD pipeline YAML",
            ExportFormat::Dockerfile => "Dockerfile with RUN commands",
            ExportFormat::Circleci => "CircleCI configuration YAML",
        }
    }

    /// Returns the kebab-case name for this format as clap displays it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::Bash => "bash",
            ExportFormat::Makefile => "makefile",
            ExportFormat::Markdown => "markdown",
            ExportFormat::GithubAction => "github-action",
            ExportFormat::GitlabCi => "gitlab-ci",
            ExportFormat::Dockerfile => "dockerfile",
            ExportFormat::Circleci => "circleci",
        }
    }

    /// Returns all format names as clap would display them (kebab-case).
    ///
    /// Uses clap's `ValueEnum::value_variants()` to iterate all variants,
    /// ensuring the list stays in sync with the enum definition.
    #[must_use]
    pub fn all_names() -> Vec<&'static str> {
        Self::value_variants()
            .iter()
            .map(ExportFormat::name)
            .collect()
    }

    /// Returns `(name, description)` pairs for all formats.
    ///
    /// Useful for --help text and error messages.
    #[must_use]
    pub fn all_with_descriptions() -> Vec<(&'static str, &'static str)> {
        Self::value_variants()
            .iter()
            .map(|v| (v.name(), v.description()))
            .collect()
    }

    /// Suggests the closest format name for an invalid input using fuzzy matching.
    ///
    /// Returns `Some(name)` if a format name is within Levenshtein edit distance 2
    /// of the input, otherwise `None`.
    #[must_use]
    pub fn suggest(invalid: &str) -> Option<String> {
        let invalid_lower = invalid.to_lowercase();
        Self::all_names()
            .into_iter()
            .filter(|name| levenshtein(&invalid_lower, name) <= 2)
            .min_by_key(|name| levenshtein(&invalid_lower, name))
            .map(std::string::ToString::to_string)
    }
}

/// Supported shells for initialization.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Shell {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
}

/// Shell hook event types.
///
/// Used by the hidden `_hook` subcommand to distinguish between
/// pre-execution (captures command text) and post-execution
/// (captures exit code) hook events.
#[derive(Clone, Debug, ValueEnum)]
pub enum HookType {
    /// Before command execution - receives command text
    Preexec,
    /// After command execution - receives exit code
    Precmd,
}

impl Shell {
    /// Detect the current shell from the SHELL environment variable.
    ///
    /// Returns None if detection fails (unknown shell or SHELL not set).
    #[must_use]
    pub fn detect() -> Option<Self> {
        let shell_path = std::env::var("SHELL").ok()?;
        let shell_name = shell_path.rsplit('/').next()?;

        match shell_name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
    }

    /// Get the display name for the shell.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }

    /// Get the typical rc file path for this shell.
    #[must_use]
    pub fn rc_file(&self) -> &'static str {
        match self {
            Shell::Bash => "~/.bashrc",
            Shell::Zsh => "~/.zshrc",
            Shell::Fish => "~/.config/fish/config.fish",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_description_returns_static_str() {
        assert_eq!(
            ExportFormat::Bash.description(),
            "Bash shell script with set -e"
        );
        assert_eq!(
            ExportFormat::Makefile.description(),
            "Makefile with command targets"
        );
        assert_eq!(
            ExportFormat::Markdown.description(),
            "Markdown documentation with code blocks"
        );
        assert_eq!(
            ExportFormat::GithubAction.description(),
            "GitHub Actions workflow YAML"
        );
        assert_eq!(
            ExportFormat::GitlabCi.description(),
            "GitLab CI/CD pipeline YAML"
        );
        assert_eq!(
            ExportFormat::Dockerfile.description(),
            "Dockerfile with RUN commands"
        );
        assert_eq!(
            ExportFormat::Circleci.description(),
            "CircleCI configuration YAML"
        );
    }

    #[test]
    fn export_format_name_returns_kebab_case() {
        assert_eq!(ExportFormat::Bash.name(), "bash");
        assert_eq!(ExportFormat::GithubAction.name(), "github-action");
        assert_eq!(ExportFormat::GitlabCi.name(), "gitlab-ci");
        assert_eq!(ExportFormat::Circleci.name(), "circleci");
    }

    #[test]
    fn export_format_all_names_returns_all_variants() {
        let names = ExportFormat::all_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"bash"));
        assert!(names.contains(&"makefile"));
        assert!(names.contains(&"markdown"));
        assert!(names.contains(&"github-action"));
        assert!(names.contains(&"gitlab-ci"));
        assert!(names.contains(&"dockerfile"));
        assert!(names.contains(&"circleci"));
    }

    #[test]
    fn export_format_all_with_descriptions_pairs() {
        let pairs = ExportFormat::all_with_descriptions();
        assert_eq!(pairs.len(), 7);
        // Check first and last
        assert!(
            pairs
                .iter()
                .any(|(n, d)| *n == "bash" && d.contains("Bash"))
        );
        assert!(
            pairs
                .iter()
                .any(|(n, d)| *n == "circleci" && d.contains("CircleCI"))
        );
    }

    #[test]
    fn export_format_suggest_finds_close_match() {
        // "bassh" → "bash" (distance 1)
        assert_eq!(ExportFormat::suggest("bassh"), Some("bash".to_string()));
    }

    #[test]
    fn export_format_suggest_finds_github_action() {
        // "github-acton" → "github-action" (distance 1)
        assert_eq!(
            ExportFormat::suggest("github-acton"),
            Some("github-action".to_string())
        );
    }

    #[test]
    fn export_format_suggest_returns_none_for_distant_match() {
        // "zzzzz" is far from everything
        assert_eq!(ExportFormat::suggest("zzzzz"), None);
    }

    #[test]
    fn export_format_suggest_is_case_insensitive() {
        assert_eq!(ExportFormat::suggest("BASH"), Some("bash".to_string()));
        assert_eq!(ExportFormat::suggest("Bassh"), Some("bash".to_string()));
    }
}
