//! Formatting of diagnostic check results.
//!
//! Supports human-readable (with optional ANSI color) and JSON output.

use serde_json::json;

use super::checks::{CheckResult, CheckStatus};

/// Format check results as a human-readable checklist.
///
/// When `use_color` is true, uses ANSI escape codes:
/// - Green for pass
/// - Yellow for warn
/// - Red for fail
#[must_use]
pub fn format_report(results: &[CheckResult], use_color: bool) -> String {
    let mut lines = Vec::new();
    lines.push("rec doctor".to_string());

    for result in results {
        let (symbol, color_start, color_end) = match result.status {
            CheckStatus::Pass => {
                if use_color {
                    ("\u{2713}", "\x1b[32m", "\x1b[0m")
                } else {
                    ("\u{2713}", "", "")
                }
            }
            CheckStatus::Warn => {
                if use_color {
                    ("\u{26a0}", "\x1b[33m", "\x1b[0m")
                } else {
                    ("\u{26a0}", "", "")
                }
            }
            CheckStatus::Fail => {
                if use_color {
                    ("\u{2717}", "\x1b[31m", "\x1b[0m")
                } else {
                    ("\u{2717}", "", "")
                }
            }
        };

        lines.push(format!(
            "{}{} {}: {}{}",
            color_start, symbol, result.name, result.message, color_end
        ));

        if let Some(ref hint) = result.fix_hint {
            lines.push(format!("  Fix: {hint}"));
        }
    }

    // Summary line
    let pass_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Pass)
        .count();
    let warn_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Warn)
        .count();
    let fail_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Fail)
        .count();

    lines.push(String::new());
    lines.push(format!(
        "Result: {pass_count} passed, {warn_count} warnings, {fail_count} failed"
    ));

    lines.join("\n")
}

/// Format check results as a JSON value.
///
/// Structure:
/// ```json
/// {
///   "checks": [
///     { "name": "...", "status": "pass|warn|fail", "message": "...", "fix_hint": null|"..." }
///   ],
///   "summary": { "pass": N, "warn": N, "fail": N }
/// }
/// ```
#[must_use]
pub fn format_report_json(results: &[CheckResult]) -> serde_json::Value {
    let checks: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "status": r.status.as_str(),
                "message": r.message,
                "fix_hint": r.fix_hint,
            })
        })
        .collect();

    let pass_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Pass)
        .count();
    let warn_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Warn)
        .count();
    let fail_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Fail)
        .count();

    json!({
        "checks": checks,
        "summary": {
            "pass": pass_count,
            "warn": warn_count,
            "fail": fail_count,
        }
    })
}
