//! Interactive demo session generation.
//!
//! Provides a guided walkthrough of `rec` features using simulated
//! terminal output with typing effects.

use std::thread;
use std::time::Duration;

/// Run a simulated interactive walkthrough of the core rec workflow.
///
/// Prints a complete demonstration of the start -> record -> stop -> show ->
/// replay --dry-run -> export flow using styled text. No real sessions are
/// created, no state files are touched.
///
/// When `is_tty` is true, brief pauses (500ms) are added between sections
/// for readability. When false (piped output, CI), everything is printed
/// immediately.
pub fn run_demo(is_tty: bool) {
    let pause = if is_tty {
        Duration::from_millis(500)
    } else {
        Duration::ZERO
    };

    // Step 1: Welcome banner
    println!("Welcome to rec! Let's walk through the core workflow.");
    println!("rec records your terminal commands and lets you replay, export, and share them.");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 2: Starting a recording
    println!();
    println!("$ rec start --name deploy-demo");
    println!("\u{2713} Recording started: deploy-demo");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 3: Show example commands being "captured"
    println!();
    println!("$ echo \"Hello from rec!\"");
    println!("$ git status");
    println!("$ docker compose up -d");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 4: Stopping the recording
    println!();
    println!("$ rec stop");
    println!("\u{2713} Recording stopped: deploy-demo (3 commands)");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 5: Viewing the session
    println!();
    println!("$ rec show deploy-demo");
    println!("Session: deploy-demo");
    println!("Commands: 3");
    println!("  1. echo \"Hello from rec!\"");
    println!("  2. git status");
    println!("  3. docker compose up -d");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 6: Replay with dry-run
    println!();
    println!("$ rec replay deploy-demo --dry-run");
    println!("[dry-run] Would execute: echo \"Hello from rec!\"");
    println!("[dry-run] Would execute: git status");
    println!("[dry-run] Would execute: docker compose up -d");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 7: Export to bash script
    println!();
    println!("$ rec export deploy-demo --format bash");
    println!("#!/usr/bin/env bash");
    println!("set -euo pipefail");
    println!("echo \"Hello from rec!\"");
    println!("git status");
    println!("docker compose up -d");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 8: v2 feature highlights
    println!();
    println!("New in v2:");
    println!("  \u{2022} rec search \"docker\"     \u{2014} Search across all sessions");
    println!("  \u{2022} rec stats               \u{2014} View recording statistics");
    println!("  \u{2022} rec doctor              \u{2014} Diagnose installation issues");
    println!("  \u{2022} rec import deploy.sh    \u{2014} Import from scripts or history");
    if !pause.is_zero() {
        thread::sleep(pause);
    }

    // Step 9: Closing
    println!();
    println!("That's rec! Get started: rec start --name my-first-session");
    println!("Docs: https://github.com/zeybek/rec");
}

#[cfg(test)]
mod tests {
    // run_demo prints to stdout — we just verify it doesn't panic.

    #[test]
    fn run_demo_non_tty_does_not_panic() {
        // Non-TTY mode: no pauses, fast completion
        super::run_demo(false);
    }
}
