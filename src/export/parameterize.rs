use regex::Regex;

use crate::models::Session;

/// A detected or manual parameter with its name, original value, and replacement value.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// Parameter name, e.g. "`HOME_DIR`", "USERNAME", "HOSTNAME", or user-defined
    pub name: String,
    /// Original value, e.g. "/home/alice", "alice", "workstation"
    pub original: String,
    /// Replacement value (None = use original)
    pub value: Option<String>,
    /// true if auto-detected, false if manual {{VAR}}
    pub auto_detected: bool,
}

/// Auto-detect environment-specific parameters from a session.
///
/// Extracts home directory, username, and hostname from session metadata,
/// but only includes them if the value actually appears in at least one
/// command string or cwd path.
#[must_use]
pub fn detect_parameters(session: &Session) -> Vec<Parameter> {
    let mut params = Vec::new();

    // Collect candidate values from session metadata
    let mut candidates: Vec<(&str, String)> = Vec::new();

    if let Some(home) = session.header.env.get("HOME") {
        if !home.is_empty() {
            candidates.push(("HOME_DIR", home.clone()));
        }
    }

    if let Some(user) = session.header.env.get("USER") {
        if !user.is_empty() {
            candidates.push(("USERNAME", user.clone()));
        }
    }

    if session.header.hostname != "unknown" && !session.header.hostname.is_empty() {
        candidates.push(("HOSTNAME", session.header.hostname.clone()));
    }

    // Only include candidates whose original value appears in at least one
    // command string or cwd path
    for (name, original) in candidates {
        let found = session.commands.iter().any(|cmd| {
            cmd.command.contains(&original) || cmd.cwd.to_string_lossy().contains(&original)
        });

        if found {
            // Deduplicate: skip if we already have this exact original value
            if !params.iter().any(|p: &Parameter| p.original == original) {
                params.push(Parameter {
                    name: name.to_string(),
                    original,
                    value: None,
                    auto_detected: true,
                });
            }
        }
    }

    params
}

/// Parse manual {{`VAR_NAME`}} placeholders from command strings.
///
/// Scans all commands for `{{VAR_NAME}}` patterns. Returns a Parameter
/// for each unique placeholder name found, skipping names that match
/// auto-detected parameter names (`HOME_DIR`, USERNAME, HOSTNAME).
///
/// # Panics
///
/// Panics if the internal regex pattern is invalid (should never happen).
#[must_use]
pub fn parse_manual_placeholders(session: &Session) -> Vec<Parameter> {
    let re = Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}").expect("valid regex");

    let auto_names: &[&str] = &["HOME_DIR", "USERNAME", "HOSTNAME"];
    let mut seen: Vec<String> = Vec::new();
    let mut params = Vec::new();

    for cmd in &session.commands {
        for cap in re.captures_iter(&cmd.command) {
            let var_name = cap[1].to_string();
            if auto_names.contains(&var_name.as_str()) {
                continue;
            }
            if seen.contains(&var_name) {
                continue;
            }
            seen.push(var_name.clone());
            params.push(Parameter {
                name: var_name.clone(),
                original: format!("{{{{{var_name}}}}}"),
                value: None,
                auto_detected: false,
            });
        }
    }

    params
}

/// Detect all parameters: auto-detected environment values and manual placeholders.
///
/// Auto-detected parameters appear first, followed by manual ones.
#[must_use]
pub fn detect_all_parameters(session: &Session) -> Vec<Parameter> {
    let mut params = detect_parameters(session);
    let manual = parse_manual_placeholders(session);
    params.extend(manual);
    params
}

/// Format type for rendering variables in format-native syntax.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormatType {
    /// Bash/YAML/Dockerfile: {{NAME}} → $NAME
    Shell,
    /// Makefile: {{NAME}} → $(NAME)
    Make,
    /// Markdown: {{NAME}} → {{NAME}} (passthrough)
    Markdown,
}

/// Replace `{{VAR}}` placeholders with format-native variable references.
///
/// # Panics
///
/// Panics if the internal regex pattern is invalid (should never happen).
#[must_use]
pub fn render_for_format(s: &str, format: FormatType) -> String {
    match format {
        FormatType::Shell => {
            // Replace {{NAME}} with $NAME
            let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}").expect("valid regex");
            re.replace_all(s, |caps: &regex::Captures| format!("${}", &caps[1]))
                .into_owned()
        }
        FormatType::Make => {
            // Replace {{NAME}} with $(NAME)
            let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}").expect("valid regex");
            re.replace_all(s, |caps: &regex::Captures| format!("$({})", &caps[1]))
                .into_owned()
        }
        FormatType::Markdown => {
            // Passthrough
            s.to_string()
        }
    }
}

/// Replace auto-detected parameter values in a command string with {{PLACEHOLDER}} syntax.
///
/// Applies replacements longest-first to avoid partial matches
/// (e.g., "/home/alice" is replaced before "alice").
/// Manual placeholders (already {{NAME}} in the string) are left as-is.
#[must_use]
pub fn apply_parameters(command: &str, params: &[Parameter]) -> String {
    // Only replace auto-detected parameters
    let mut auto_params: Vec<&Parameter> = params.iter().filter(|p| p.auto_detected).collect();

    // Sort by original length descending (longest first) to prevent partial matches
    auto_params.sort_by(|a, b| b.original.len().cmp(&a.original.len()));

    let mut result = command.to_string();
    for param in auto_params {
        let placeholder = format!("{{{{{}}}}}", param.name);
        result = result.replace(&param.original, &placeholder);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, Session, SessionHeader, SessionStatus};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    /// Helper to create a test session with given env vars, hostname, and commands.
    fn make_session(
        env: HashMap<String, String>,
        hostname: &str,
        commands: &[(&str, &str)], // (command_text, cwd)
    ) -> Session {
        let header = SessionHeader {
            version: 2,
            id: Uuid::new_v4(),
            name: "test-session".to_string(),
            shell: "bash".to_string(),
            os: "linux".to_string(),
            hostname: hostname.to_string(),
            env,
            tags: Vec::new(),
            recovered: None,
            started_at: 1700000000.0,
        };

        let cmds: Vec<Command> = commands
            .iter()
            .enumerate()
            .map(|(i, (cmd, cwd))| Command {
                index: i as u32,
                command: cmd.to_string(),
                cwd: PathBuf::from(cwd),
                started_at: 1700000000.0 + i as f64,
                ended_at: Some(1700000001.0 + i as f64),
                exit_code: Some(0),
                duration_ms: Some(1000),
            })
            .collect();

        let cmd_count = cmds.len() as u32;
        Session {
            header,
            commands: cmds,
            footer: Some(crate::models::SessionFooter {
                ended_at: 1700000010.0,
                command_count: cmd_count,
                status: SessionStatus::Completed,
            }),
        }
    }

    fn env_with(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_detect_home_dir() {
        let session = make_session(
            env_with(&[("HOME", "/home/alice")]),
            "unknown",
            &[("ls /home/alice/docs", "/home/alice")],
        );

        let params = detect_parameters(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "HOME_DIR");
        assert_eq!(params[0].original, "/home/alice");
        assert!(params[0].auto_detected);
    }

    #[test]
    fn test_detect_username() {
        let session = make_session(
            env_with(&[("USER", "alice")]),
            "unknown",
            &[("echo alice", "/tmp")],
        );

        let params = detect_parameters(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "USERNAME");
        assert_eq!(params[0].original, "alice");
    }

    #[test]
    fn test_detect_hostname() {
        let session = make_session(env_with(&[]), "myhost", &[("ssh myhost", "/tmp")]);

        let params = detect_parameters(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "HOSTNAME");
        assert_eq!(params[0].original, "myhost");
    }

    #[test]
    fn test_no_detection_when_absent() {
        // HOME is set but never appears in any command or cwd
        let session = make_session(
            env_with(&[("HOME", "/home/alice")]),
            "unknown",
            &[("echo hello", "/tmp")],
        );

        let params = detect_parameters(&session);
        assert!(params.is_empty());
    }

    #[test]
    fn test_manual_placeholder() {
        let session = make_session(
            env_with(&[]),
            "unknown",
            &[("curl {{DB_HOST}}:5432", "/tmp")],
        );

        let params = parse_manual_placeholders(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "DB_HOST");
        assert_eq!(params[0].original, "{{DB_HOST}}");
        assert!(!params[0].auto_detected);
    }

    #[test]
    fn test_apply_parameters() {
        let params = vec![Parameter {
            name: "HOME_DIR".to_string(),
            original: "/home/alice".to_string(),
            value: None,
            auto_detected: true,
        }];

        let result = apply_parameters("ls /home/alice/docs", &params);
        assert_eq!(result, "ls {{HOME_DIR}}/docs");
    }

    #[test]
    fn test_longest_first_replacement() {
        let params = vec![
            Parameter {
                name: "HOME_DIR".to_string(),
                original: "/home/alice".to_string(),
                value: None,
                auto_detected: true,
            },
            Parameter {
                name: "USERNAME".to_string(),
                original: "alice".to_string(),
                value: None,
                auto_detected: true,
            },
        ];

        // "/home/alice" (longer) should be replaced first, so "alice" is not
        // partially matched inside the path
        let result = apply_parameters("ls /home/alice && whoami | grep alice", &params);
        assert_eq!(result, "ls {{HOME_DIR}} && whoami | grep {{USERNAME}}");
    }

    #[test]
    fn test_detect_all_combines() {
        let session = make_session(
            env_with(&[("HOME", "/home/alice")]),
            "unknown",
            &[("ls /home/alice && curl {{DB_HOST}}", "/home/alice")],
        );

        let params = detect_all_parameters(&session);
        // Should have HOME_DIR (auto) and DB_HOST (manual)
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "HOME_DIR");
        assert!(params[0].auto_detected);
        assert_eq!(params[1].name, "DB_HOST");
        assert!(!params[1].auto_detected);
    }

    #[test]
    fn test_cwd_scanning() {
        // HOME appears only in cwd, not in command string
        let session = make_session(
            env_with(&[("HOME", "/home/alice")]),
            "unknown",
            &[("echo hello", "/home/alice/projects")],
        );

        let params = detect_parameters(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "HOME_DIR");
    }

    #[test]
    fn test_manual_placeholder_skips_auto_names() {
        // If someone writes {{HOME_DIR}} in a command, it should be skipped
        // by manual detection since it matches an auto-detected name
        let session = make_session(
            env_with(&[]),
            "unknown",
            &[("echo {{HOME_DIR}} {{CUSTOM_VAR}}", "/tmp")],
        );

        let params = parse_manual_placeholders(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "CUSTOM_VAR");
    }

    #[test]
    fn test_manual_placeholder_deduplication() {
        let session = make_session(
            env_with(&[]),
            "unknown",
            &[
                ("curl {{DB_HOST}}:5432", "/tmp"),
                ("ping {{DB_HOST}}", "/tmp"),
            ],
        );

        let params = parse_manual_placeholders(&session);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "DB_HOST");
    }

    #[test]
    fn test_apply_parameters_ignores_manual() {
        let params = vec![
            Parameter {
                name: "HOME_DIR".to_string(),
                original: "/home/alice".to_string(),
                value: None,
                auto_detected: true,
            },
            Parameter {
                name: "DB_HOST".to_string(),
                original: "{{DB_HOST}}".to_string(),
                value: None,
                auto_detected: false,
            },
        ];

        // Manual placeholder should stay as-is
        let result = apply_parameters("ls /home/alice && curl {{DB_HOST}}", &params);
        assert_eq!(result, "ls {{HOME_DIR}} && curl {{DB_HOST}}");
    }
}
