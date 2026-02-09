//! Replay engine orchestrating the full replay loop.
//!
//! The `ReplayEngine` is a state machine that processes commands sequentially,
//! applying filters, safety checks, interactive prompts, and execution with
//! error handling and signal handling.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::Output;
use crate::models::Session;
use crate::models::config::Config;
use crate::replay::executor;
use crate::replay::prompt;
use crate::replay::safety::DestructiveDetector;
use crate::replay::{DangerPolicy, ReplayOptions, ReplaySummary};

/// Replay engine that orchestrates command filtering, safety checks,
/// interactive prompts, execution, error handling, and signal handling.
pub struct ReplayEngine {
    session: Session,
    options: ReplayOptions,
    detector: DestructiveDetector,
    output: Output,
    aborted: Arc<AtomicBool>,
    step_mode_active: bool,
    executed_count: usize,
    skipped_count: usize,
    failed_count: usize,
    skipped_dangerous: usize,
    /// Current working directory, updated when cd commands are executed
    current_cwd: Option<std::path::PathBuf>,
}

impl ReplayEngine {
    /// Create a new replay engine.
    ///
    /// # Arguments
    /// - `session` - The session to replay
    /// - `options` - Replay configuration (dry-run, step, skip, etc.)
    /// - `config` - Application config (safety settings)
    /// - `output` - Output handler for styled terminal output
    #[must_use]
    pub fn new(session: Session, options: ReplayOptions, config: &Config, output: Output) -> Self {
        let detector = DestructiveDetector::new(&config.safety);
        let aborted = Arc::new(AtomicBool::new(false));
        let step_mode_active = options.step;

        Self {
            session,
            options,
            detector,
            output,
            aborted,
            step_mode_active,
            executed_count: 0,
            skipped_count: 0,
            failed_count: 0,
            skipped_dangerous: 0,
            current_cwd: None,
        }
    }

    /// Create a new replay engine with a shared abort flag.
    ///
    /// Use this when a global Ctrl+C handler is already installed and you
    /// want the engine to check the same flag.
    ///
    /// # Arguments
    /// - `session` - The session to replay
    /// - `options` - Replay configuration (dry-run, step, skip, etc.)
    /// - `config` - Application config (safety settings)
    /// - `output` - Output handler for styled terminal output
    /// - `abort_flag` - Shared atomic flag set by the global Ctrl+C handler
    pub fn with_abort_flag(
        session: Session,
        options: ReplayOptions,
        config: &Config,
        output: Output,
        abort_flag: Arc<AtomicBool>,
    ) -> Self {
        let detector = DestructiveDetector::new(&config.safety);
        let step_mode_active = options.step;

        Self {
            session,
            options,
            detector,
            output,
            aborted: abort_flag,
            step_mode_active,
            executed_count: 0,
            skipped_count: 0,
            failed_count: 0,
            skipped_dangerous: 0,
            current_cwd: None,
        }
    }

    /// Run the replay engine, processing all commands in the session.
    ///
    /// Returns a `ReplaySummary` with execution statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if command execution or I/O fails.
    pub fn run(&mut self) -> crate::error::Result<ReplaySummary> {
        let total = self.session.commands.len();

        // Install Ctrl+C signal handler (may fail if a global handler is already set)
        let abort_flag = Arc::clone(&self.aborted);
        let _ = ctrlc::set_handler(move || {
            abort_flag.store(true, Ordering::SeqCst);
        });

        // Warn on incomplete session
        if self.session.footer.is_none() {
            self.output.warning(&format!(
                "This session was not completed. Playing {total} available commands."
            ));
        }

        // Non-interactive mode checks
        if self.options.step && !prompt::is_interactive() {
            return Err(crate::error::RecError::Config(
                "Step mode requires an interactive terminal".to_string(),
            ));
        }
        if self.options.danger_policy.is_none()
            && !self.options.force
            && !self.options.dry_run
            && !prompt::is_interactive()
        {
            self.output
                .warning("Non-interactive terminal: destructive command prompts will auto-deny");
        }

        // DangerPolicy::Abort — pre-scan ALL commands before executing any
        if matches!(self.options.danger_policy, Some(DangerPolicy::Abort)) {
            let dangerous: Vec<(usize, &str, String)> = self
                .session
                .commands
                .iter()
                .filter_map(|cmd| {
                    self.detector
                        .match_reason(&cmd.command)
                        .map(|reason| (cmd.index as usize, cmd.command.as_str(), reason))
                })
                .collect();

            if !dangerous.is_empty() {
                eprintln!("error: Found {} dangerous command(s):", dangerous.len());
                for (idx, cmd_text, reason) in &dangerous {
                    eprintln!("  [{}] $ {} — {}", idx + 1, cmd_text, reason);
                }
                eprintln!("\nAborting without executing any commands.");
                eprintln!("Use --danger-policy allow to execute all commands.");
                return Ok(ReplaySummary {
                    total,
                    executed: 0,
                    skipped: 0,
                    failed: 0,
                    aborted: true,
                });
            }
        }

        // DangerPolicy::Allow — print info notice before execution
        if matches!(self.options.danger_policy, Some(DangerPolicy::Allow)) {
            let dangerous_count = self
                .session
                .commands
                .iter()
                .filter(|cmd| self.detector.is_destructive(&cmd.command))
                .count();
            if dangerous_count > 0 {
                eprintln!(
                    "note: Executing {dangerous_count} dangerous command(s) (--danger-policy allow)"
                );
            }
        }

        // Process each command
        for cmd in &self.session.commands.clone() {
            // Check abort flag
            if self.aborted.load(Ordering::SeqCst) {
                break;
            }

            let index = cmd.index as usize;

            // --from filter: skip commands before start index (silently)
            if let Some(from_idx) = self.options.from_index {
                if index < from_idx {
                    continue;
                }
            }

            // --skip indices filter
            if self.options.skip_indices.contains(&index) {
                self.skipped_count += 1;
                self.output.info(&format!(
                    "[{}/{}] [skipped] $ {}",
                    index + 1,
                    total,
                    cmd.command
                ));
                continue;
            }

            // --skip-pattern filter
            if self
                .options
                .skip_patterns
                .iter()
                .any(|p| p.matches(&cmd.command))
            {
                self.skipped_count += 1;
                self.output.info(&format!(
                    "[{}/{}] [skipped] $ {}",
                    index + 1,
                    total,
                    cmd.command
                ));
                continue;
            }

            // Check destructive
            let is_destructive = self.detector.is_destructive(&cmd.command);

            // Dry-run mode: display command info without executing
            if self.options.dry_run {
                self.print_dry_run(cmd, index, total, is_destructive);
                continue;
            }

            // Step mode: prompt for action
            if self.step_mode_active {
                let action = prompt::prompt_step(&cmd.command, index, total, is_destructive);
                match action {
                    prompt::StepAction::Run => { /* proceed to execution */ }
                    prompt::StepAction::Skip => {
                        self.skipped_count += 1;
                        continue;
                    }
                    prompt::StepAction::Abort => {
                        self.aborted.store(true, Ordering::SeqCst);
                        break;
                    }
                    prompt::StepAction::RunAll => {
                        self.step_mode_active = false;
                        // proceed to execution
                    }
                }
            } else if is_destructive {
                // Destructive command handling based on danger_policy
                match self.options.danger_policy {
                    Some(DangerPolicy::Skip) => {
                        self.skipped_count += 1;
                        self.skipped_dangerous += 1;
                        eprintln!(
                            "warning: Skipped dangerous command [{}/{}]: {} (use --danger-policy allow to override)",
                            index + 1,
                            total,
                            cmd.command
                        );
                        continue;
                    }
                    Some(DangerPolicy::Allow) => {
                        // Proceed to execution (info notice printed once at start)
                    }
                    Some(DangerPolicy::Abort) => {
                        // Should not reach here — abort is handled in pre-scan above
                        unreachable!(
                            "DangerPolicy::Abort should be handled by pre-scan before the command loop"
                        );
                    }
                    None if self.options.force => {
                        // Legacy --force behavior: bypass prompts
                    }
                    None => {
                        if prompt::is_interactive() {
                            // Interactive: prompt for confirmation
                            let reason = self
                                .detector
                                .match_reason(&cmd.command)
                                .unwrap_or_else(|| "Matched destructive pattern".to_string());
                            if !prompt::prompt_destructive(&cmd.command, &reason) {
                                self.skipped_count += 1;
                                continue;
                            }
                        } else {
                            // Non-interactive without explicit policy: default to skip
                            self.skipped_count += 1;
                            self.output.warning(&format!(
                                "[{}/{}] Skipping destructive command (non-interactive): $ {}",
                                index + 1,
                                total,
                                cmd.command
                            ));
                            continue;
                        }
                    }
                }
            }

            // Execute command with retry loop
            loop {
                // Print command being executed
                if self.output.colors {
                    eprintln!("\x1b[1m$ {}\x1b[0m", cmd.command);
                } else {
                    eprintln!("$ {}", cmd.command);
                }

                // Determine cwd: --cwd uses original, otherwise use tracked current_cwd
                let cwd: Option<&Path> = if self.options.use_original_cwd {
                    Some(cmd.cwd.as_path())
                } else {
                    self.current_cwd.as_deref()
                };

                // Execute
                let result = executor::execute_command(&cmd.command, cwd);
                self.executed_count += 1;

                match result {
                    Ok(exec_result) => {
                        // Update current_cwd if this was a successful cd command
                        if let Some(new_cwd) = exec_result.new_cwd {
                            if !self.options.use_original_cwd {
                                self.current_cwd = Some(new_cwd);
                            }
                        }

                        if exec_result.status.success() {
                            break; // Command succeeded, move to next
                        }

                        // Command failed
                        self.failed_count += 1;
                        let exit_code = exec_result.status.code();

                        if prompt::is_interactive() && !self.aborted.load(Ordering::SeqCst) {
                            let action = prompt::prompt_error(&cmd.command, exit_code);
                            match action {
                                prompt::ErrorAction::Continue => break,
                                prompt::ErrorAction::Abort => {
                                    self.aborted.store(true, Ordering::SeqCst);
                                    break;
                                }
                                prompt::ErrorAction::Retry => {
                                    // Undo the executed_count increment for retry
                                    self.executed_count -= 1;
                                    self.failed_count -= 1;
                                    continue; // Retry the same command
                                }
                            }
                        }
                        // Non-interactive: print error and continue
                        eprintln!(
                            "  Command failed with exit code: {}",
                            exit_code.map_or_else(|| "unknown".to_string(), |c| c.to_string())
                        );
                        break;
                    }
                    Err(e) => {
                        self.failed_count += 1;
                        eprintln!("  Failed to execute command: {e}");
                        break;
                    }
                }
            }

            // Check abort after execution
            if self.aborted.load(Ordering::SeqCst) {
                break;
            }
        }

        // DangerPolicy::Skip end summary
        if matches!(self.options.danger_policy, Some(DangerPolicy::Skip))
            && self.skipped_dangerous > 0
        {
            eprintln!(
                "warning: Skipped {} of {} commands (dangerous). Re-run with --danger-policy allow to include them.",
                self.skipped_dangerous, total
            );
        }

        // Print summary
        let aborted = self.aborted.load(Ordering::SeqCst);
        self.print_summary(total, aborted);

        Ok(ReplaySummary {
            total,
            executed: self.executed_count,
            skipped: self.skipped_count,
            failed: self.failed_count,
            aborted,
        })
    }

    /// Print dry-run output for a single command.
    fn print_dry_run(
        &self,
        cmd: &crate::models::Command,
        index: usize,
        total: usize,
        is_destructive: bool,
    ) {
        let dangerous_marker = if is_destructive { " [DANGEROUS]" } else { "" };
        let warn_suffix = if is_destructive {
            format!(" {}", self.output.warning_symbol())
        } else {
            String::new()
        };

        println!(
            "[{}/{}] $ {}{}{}",
            index + 1,
            total,
            cmd.command,
            dangerous_marker,
            warn_suffix
        );
        println!("        cwd: {}", cmd.cwd.display());
        if let Some(exit_code) = cmd.exit_code {
            println!("        exit: {exit_code}");
        }
        if let Some(duration) = cmd.duration_ms {
            println!("        duration: {duration}ms");
        }
        if is_destructive {
            if self.output.colors {
                println!("        \x1b[31;1m\u{26a0} DESTRUCTIVE\x1b[0m");
            } else {
                println!("        [WARN] DESTRUCTIVE");
            }
        }
        println!();
    }

    /// Print replay completion summary.
    fn print_summary(&self, total: usize, aborted: bool) {
        println!();
        if aborted {
            self.output.warning("Replay aborted by user");
        }
        self.output.success(&format!(
            "Replay complete: {}/{} commands executed, {} skipped, {} failed",
            self.executed_count, total, self.skipped_count, self.failed_count
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session};
    use std::path::PathBuf;

    fn create_test_session(commands: &[&str]) -> Session {
        let mut session = Session::new("test-replay");
        for (i, cmd_text) in commands.iter().enumerate() {
            let mut cmd = Command::new(i as u32, cmd_text.to_string(), PathBuf::from("/tmp"));
            cmd.complete(0);
            session.add_command(cmd);
        }
        session
    }

    #[test]
    fn test_engine_creation() {
        let session = create_test_session(&["echo hello"]);
        let config = Config::default();
        let output = Output::new(false, false, false);
        let options = ReplayOptions::default();

        let engine = ReplayEngine::new(session, options, &config, output);
        assert_eq!(engine.executed_count, 0);
        assert_eq!(engine.skipped_count, 0);
        assert_eq!(engine.failed_count, 0);
        assert!(!engine.step_mode_active);
    }

    #[test]
    fn test_dry_run_does_not_execute() {
        let session = create_test_session(&["echo hello", "echo world"]);
        let config = Config::default();
        let output = Output::new(false, true, false); // quiet to reduce noise
        let options = ReplayOptions {
            dry_run: true,
            ..Default::default()
        };

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        assert_eq!(summary.total, 2);
        assert_eq!(summary.executed, 0);
        assert_eq!(summary.skipped, 0);
        assert!(!summary.aborted);
    }

    #[test]
    fn test_skip_indices() {
        let session = create_test_session(&["echo a", "echo b", "echo c"]);
        let config = Config::default();
        let output = Output::new(false, true, false);
        let mut options = ReplayOptions {
            dry_run: true,
            ..Default::default()
        };
        options.skip_indices.insert(1); // skip second command (0-based)

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        assert_eq!(summary.total, 3);
        assert_eq!(summary.skipped, 1);
    }

    #[test]
    fn test_from_index() {
        let session = create_test_session(&["echo a", "echo b", "echo c"]);
        let config = Config::default();
        let output = Output::new(false, true, false);
        let options = ReplayOptions {
            dry_run: true,
            from_index: Some(2), // start from third command (0-based index 2)
            ..Default::default()
        };

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        // Only the third command should be processed (not skipped, just displayed in dry-run)
        assert_eq!(summary.total, 3);
        assert_eq!(summary.executed, 0); // dry-run
        assert_eq!(summary.skipped, 0); // from_index doesn't count as skipped
    }

    #[test]
    fn test_skip_pattern() {
        let session = create_test_session(&["echo hello", "rm -rf /tmp/test", "ls -la"]);
        let config = Config::default();
        let output = Output::new(false, true, false);
        let mut options = ReplayOptions {
            dry_run: true,
            ..Default::default()
        };
        options
            .skip_patterns
            .push(glob::Pattern::new("rm*").unwrap());

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        assert_eq!(summary.total, 3);
        assert_eq!(summary.skipped, 1);
    }

    #[test]
    fn test_execute_safe_commands() {
        let session = create_test_session(&["echo hello", "echo world"]);
        let config = Config::default();
        let output = Output::new(false, true, false);
        let options = ReplayOptions::default();

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        assert_eq!(summary.total, 2);
        assert_eq!(summary.executed, 2);
        assert_eq!(summary.failed, 0);
        assert!(!summary.aborted);
    }

    #[test]
    fn test_incomplete_session_warns() {
        // Session without footer (incomplete)
        let mut session = Session::new("incomplete-test");
        let cmd = Command::new(0, "echo test".to_string(), PathBuf::from("/tmp"));
        session.add_command(cmd);
        // Don't call session.complete() — leave footer as None

        let config = Config::default();
        let output = Output::new(false, false, false);
        let options = ReplayOptions {
            dry_run: true,
            ..Default::default()
        };

        let mut engine = ReplayEngine::new(session, options, &config, output);
        let summary = engine.run().unwrap();

        // Should still work with available commands
        assert_eq!(summary.total, 1);
    }
}
