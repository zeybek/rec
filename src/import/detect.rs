//! Format detection and session name generation for imports.

use crate::error::{RecError, Result};
use crate::models::session::validate_session_name;
use std::fmt;
use std::path::Path;

/// Supported import file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    BashScript,
    BashHistory,
    ZshHistory,
    FishHistory,
}

impl fmt::Display for ImportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportFormat::BashScript => write!(f, "bash-script"),
            ImportFormat::BashHistory => write!(f, "bash-history"),
            ImportFormat::ZshHistory => write!(f, "zsh-history"),
            ImportFormat::FishHistory => write!(f, "fish-history"),
        }
    }
}

/// Detect the import format from file extension and content.
///
/// Detection order:
/// 1. `.cast` extension → error (asciinema not supported)
/// 2. `.sh` or `.bash` extension → `BashScript`
/// 3. Shebang lines (`#!/bin/bash`, `#!/bin/sh`, `#!/usr/bin/env bash`) → `BashScript`
/// 4. Lines matching `: TIMESTAMP:DURATION;CMD` → `ZshHistory`
/// 5. Lines starting with `- cmd: ` → `FishHistory`
/// 6. Default fallback → `BashHistory`
///
/// # Errors
///
/// Returns an error if the file is an unsupported format (e.g., `.cast` files).
///
/// # Panics
///
/// Panics if the internal regex pattern is invalid (should never happen).
pub fn detect_format(path: &str, content: &str) -> Result<ImportFormat> {
    use std::sync::OnceLock;
    static ZSH_RE: OnceLock<regex::Regex> = OnceLock::new();
    let path = Path::new(path);

    // Check extension first
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext {
            "cast" => {
                return Err(RecError::InvalidSession(
                    "Asciinema .cast files are not currently supported. \
                     Use bash scripts or shell history files instead."
                        .to_string(),
                ));
            }
            "sh" | "bash" => return Ok(ImportFormat::BashScript),
            _ => {}
        }
    }

    // Sniff content (first 10 lines)
    let first_lines: Vec<&str> = content.lines().take(10).collect();

    // Check for shebang
    if let Some(first) = first_lines.first() {
        if first.starts_with("#!") && (first.contains("bash") || first.contains("/sh")) {
            return Ok(ImportFormat::BashScript);
        }
    }

    // Check for zsh extended history format
    let zsh_re = ZSH_RE.get_or_init(|| regex::Regex::new(r"^: \d+:\d+;").unwrap());
    for line in &first_lines {
        if zsh_re.is_match(line) {
            return Ok(ImportFormat::ZshHistory);
        }
    }

    // Check for fish history format
    for line in &first_lines {
        if line.starts_with("- cmd: ") {
            return Ok(ImportFormat::FishHistory);
        }
    }

    // Default: bash history
    Ok(ImportFormat::BashHistory)
}

/// Generate a session name from a file path.
///
/// Rules:
/// - Uses the filename (not the full path)
/// - Strips leading dot (hidden files like `.bash_history` → `bash_history`)
/// - Replaces non-alphanumeric characters (except `-` and `_`) with `-`
/// - Collapses consecutive hyphens
/// - Trims leading/trailing hyphens
/// - Lowercases the result
/// - Falls back to `imported-session` if result is empty or invalid
#[must_use]
pub fn session_name_from_path(path: &Path) -> String {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("imported");

    // Strip leading dot
    let filename = filename.strip_prefix('.').unwrap_or(filename);

    // Replace non-alphanumeric (except - and _) with hyphen
    let sanitized: String = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim leading/trailing hyphens and lowercase
    let result = result.trim_matches('-').to_lowercase();

    // Validate; fall back if empty or invalid
    if result.is_empty() || validate_session_name(&result).is_err() {
        return "imported-session".to_string();
    }

    result
}
