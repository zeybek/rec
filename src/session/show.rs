//! Session show command implementation.
//!
//! Displays session details including header info and numbered command list,
//! with optional grep filtering and JSON output.

use crate::cli::Output;
use crate::error::{RecError, Result};
use crate::models::Session;

/// Display session details with optional grep filtering and JSON output.
///
/// # Normal mode
/// Prints session header info (name, ID, date, shell, OS, hostname, etc.)
/// followed by a numbered command list. If `grep` is provided, only commands
/// matching the regex pattern are shown.
///
/// # JSON mode
/// Serializes the entire session to pretty-printed JSON.
///
/// # Errors
/// Returns an error if the grep pattern is an invalid regex.
pub fn show_session(
    session: &Session,
    grep: Option<&str>,
    json: bool,
    output: &Output,
) -> Result<()> {
    if json {
        let json_str = serde_json::to_string_pretty(session)
            .map_err(|e| RecError::InvalidSession(format!("Failed to serialize session: {e}")))?;
        println!("{json_str}");
        return Ok(());
    }

    // Compile grep regex if provided
    let re = match grep {
        Some(pattern) => {
            let compiled = regex::Regex::new(pattern).map_err(|e| {
                RecError::InvalidSession(format!("Invalid grep pattern '{pattern}': {e}"))
            })?;
            Some(compiled)
        }
        None => None,
    };

    // Print session header block
    println!();
    output.success(&format!("Session: {}", session.name()));
    println!();
    println!("  ID:        {}", session.id());

    // Format date from started_at timestamp
    if let Some(dt) = chrono::DateTime::from_timestamp(
        session.header.started_at as i64,
        ((session.header.started_at.fract()) * 1_000_000_000.0) as u32,
    ) {
        let local: chrono::DateTime<chrono::Local> = dt.into();
        println!("  Date:      {}", local.format("%Y-%m-%d %H:%M"));
    }

    println!("  Shell:     {}", session.header.shell);
    println!("  OS:        {}", session.header.os);
    println!("  Hostname:  {}", session.header.hostname);

    // Working directory from env
    if let Some(pwd) = session.header.env.get("PWD") {
        println!("  Directory: {pwd}");
    }

    // Duration (if footer exists)
    if let Some(ref footer) = session.footer {
        let duration_secs = footer.ended_at - session.header.started_at;
        println!("  Duration:  {}", format_duration(duration_secs));
        println!("  Commands:  {}", footer.command_count);
    } else {
        println!(
            "  Commands:  {} (session still recording)",
            session.commands.len()
        );
    }

    // Tags
    if !session.header.tags.is_empty() {
        println!("  Tags:      {}", session.header.tags.join(", "));
    }

    // Separator
    println!();
    println!("  {}", "-".repeat(60));
    println!();

    // Print commands
    if session.commands.is_empty() {
        output.info("  No commands recorded in this session.");
        return Ok(());
    }

    let mut shown_count = 0;
    for cmd in &session.commands {
        // Apply grep filter
        if let Some(ref re) = re {
            if !re.is_match(&cmd.command) {
                continue;
            }
        }

        shown_count += 1;
        let index = cmd.index + 1; // 1-based display
        let cwd_str = cmd.cwd.display().to_string();

        // Exit code with color hint
        let exit_str = match cmd.exit_code {
            Some(0) => output.style_success("exit:0"),
            Some(code) => output.style_error(&format!("exit:{code}")),
            None => "running".to_string(),
        };

        // Duration
        let dur_str = match cmd.duration_ms {
            Some(ms) if ms >= 1000 => format!("{:.1}s", ms as f64 / 1000.0),
            Some(ms) => format!("{ms}ms"),
            None => String::new(),
        };

        println!(
            "  {:>3}. {}  [{}]  {}  {}",
            index, cmd.command, cwd_str, exit_str, dur_str
        );
    }

    if re.is_some() {
        println!();
        output.info(&format!(
            "  Showing {} of {} commands (filtered by grep)",
            shown_count,
            session.commands.len()
        ));
    }

    println!();
    Ok(())
}

/// Format a duration in seconds as a human-readable string.
fn format_duration(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
