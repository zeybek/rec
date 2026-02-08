use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result};
use rec::error::EXIT_USER_ERROR;
use rec::replay::{DangerPolicy, ReplayEngine, ReplayOptions as EngineReplayOptions};
use rec::storage::SessionStore;
use std::collections::HashSet;
use std::process::ExitCode;

/// Options for the replay command.
///
/// Groups all replay-related CLI arguments into a single struct to reduce
/// the number of parameters passed to `handle_replay`.
pub struct ReplayOptions<'a> {
    /// Preview commands without executing
    pub dry_run: bool,
    /// Execute one command at a time with confirmation
    pub step: bool,
    /// Command indices to skip (1-based, from CLI)
    pub skip: Option<&'a Vec<u32>>,
    /// Start from a specific command index (1-based, from CLI)
    pub from: Option<u32>,
    /// Bypass destructive command prompts
    pub force: bool,
    /// Glob patterns to skip matching commands
    pub skip_pattern: Option<&'a Vec<String>>,
    /// Replay in each command's original working directory
    pub cwd: bool,
    /// Policy for handling dangerous commands
    pub danger_policy: Option<DangerPolicy>,
}

/// Handle the `replay` command — re-execute commands from a recorded session.
///
/// Supports dry-run mode, step-by-step execution, skipping specific commands,
/// and dangerous command policies. Commands can be filtered by index or pattern.
///
/// # Errors
/// Returns exit code 1 on user errors (session not found, invalid patterns, aborted).
/// Returns exit code 2 on system errors.
pub fn handle_replay(ctx: &HandlerContext, identifier: &str, opts: &ReplayOptions<'_>) -> ExitCode {
    // Detect deprecated "play" alias usage
    if let Some(first_arg) = std::env::args().nth(1) {
        if first_arg == "play" {
            eprintln!("warning: 'rec play' is deprecated, use 'rec replay' instead");
        }
    }

    // Parse skip patterns into glob::Pattern
    let skip_patterns: Vec<glob::Pattern> = match opts.skip_pattern {
        Some(patterns) => {
            let mut parsed = Vec::new();
            for p in patterns {
                match glob::Pattern::new(p) {
                    Ok(pat) => parsed.push(pat),
                    Err(e) => {
                        ctx.output.error(
                            "Invalid skip pattern",
                            &format!("'{p}': {e}"),
                            None,
                            Some("Use Unix glob syntax, e.g. 'rm*' or 'docker rm*'"),
                        );
                        return ExitCode::from(EXIT_USER_ERROR);
                    }
                }
            }
            parsed
        }
        None => Vec::new(),
    };

    // Convert 1-based user indices to 0-based
    let skip_indices: HashSet<usize> = match opts.skip {
        Some(indices) => indices
            .iter()
            .map(|i| (*i as usize).saturating_sub(1))
            .collect(),
        None => HashSet::new(),
    };

    let from_index: Option<usize> = opts.from.map(|f| (f as usize).saturating_sub(1));

    // Resolve danger_policy: --danger-policy takes precedence, --force alone maps to Allow
    let resolved_danger_policy = if let Some(dp) = opts.danger_policy {
        if opts.force {
            eprintln!("note: --danger-policy takes precedence over --force");
        }
        Some(dp)
    } else if opts.force {
        eprintln!("note: --force is equivalent to --danger-policy allow");
        Some(DangerPolicy::Allow)
    } else {
        None
    };

    // Build EngineReplayOptions for the replay engine
    let options = EngineReplayOptions {
        dry_run: opts.dry_run,
        step: opts.step,
        skip_indices,
        skip_patterns,
        from_index,
        force: opts.force,
        use_original_cwd: opts.cwd,
        danger_policy: resolved_danger_policy,
    };

    // Session resolution: alias → name → UUID
    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = rec::storage::AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match rec::session::resolve_session_with_alias(
        &store,
        &alias_store,
        identifier,
        interactive,
    ) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Create and run engine with shared abort flag
    let mut engine = ReplayEngine::with_abort_flag(
        session,
        options,
        &ctx.config,
        ctx.output,
        std::sync::Arc::clone(&ctx.interrupted),
    );
    let result = match engine.run() {
        Ok(summary) => {
            // Abort policy with dangerous commands → exit 1
            if summary.aborted && matches!(resolved_danger_policy, Some(DangerPolicy::Abort)) {
                return ExitCode::from(EXIT_USER_ERROR);
            }
            Ok(())
        }
        Err(e) => Err(e),
    };
    handle_result(result, ctx.output, &ctx.paths)
}
