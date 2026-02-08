use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::Command;
use crate::error::RecError;

/// Status of a recording session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Session is currently being recorded
    #[default]
    Recording,
    /// Session completed normally
    Completed,
    /// Session was interrupted (e.g., SIGINT, crash)
    Interrupted,
}

/// Header information for a recording session.
///
/// Contains metadata about the session that is written at the start
/// of recording. Matches the NDJSON schema from CONTEXT.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    /// Schema version (always 2)
    pub version: u8,

    /// Unique session identifier
    pub id: Uuid,

    /// Human-readable session name
    pub name: String,

    /// Shell type and version (e.g., "bash 5.1.16")
    pub shell: String,

    /// Operating system info (e.g., "Linux 6.14.0")
    pub os: String,

    /// Machine hostname
    pub hostname: String,

    /// Selected environment variables (PATH, SHELL, HOME, USER, PWD)
    pub env: HashMap<String, String>,

    /// User-defined tags for organization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether this session was recovered from a stale recording lock.
    /// Only present on sessions recovered via stale lock cleanup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovered: Option<bool>,

    /// Unix timestamp with milliseconds when session started
    pub started_at: f64,
}

/// Footer information for a recording session.
///
/// Contains summary information written when the session ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFooter {
    /// Unix timestamp with milliseconds when session ended
    pub ended_at: f64,

    /// Total number of commands recorded
    pub command_count: u32,

    /// Final status of the session
    pub status: SessionStatus,
}

/// A complete recording session.
///
/// Contains the header (metadata), list of commands, and optional footer
/// (None while still recording).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session metadata
    pub header: SessionHeader,

    /// Commands recorded in this session
    pub commands: Vec<Command>,

    /// Footer information (None while recording)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<SessionFooter>,
}

/// Generate a timestamp-based session name.
///
/// Format: session-YYYY-MM-DD-HHMMSS
/// Example: session-2026-01-26-143052
#[must_use]
pub fn generate_session_name() -> String {
    Local::now().format("session-%Y-%m-%d-%H%M%S").to_string()
}

/// Validate a session name.
///
/// Valid names contain only alphanumeric characters, dashes, and underscores.
/// No spaces or special characters allowed (filesystem-safe).
///
/// # Errors
///
/// Returns `RecError::InvalidSessionName` if the name is empty or contains
/// characters other than alphanumeric, dash, or underscore.
pub fn validate_session_name(name: &str) -> crate::error::Result<()> {
    if name.is_empty() {
        return Err(RecError::InvalidSessionName(
            "name cannot be empty".to_string(),
        ));
    }

    if name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(RecError::InvalidSessionName(name.to_string()))
    }
}

impl Session {
    /// Create a new recording session with auto-detected metadata.
    ///
    /// Automatically detects:
    /// - Shell type and version from SHELL env var
    /// - OS from `/etc/os-release` or uname
    /// - Hostname from `/etc/hostname` or environment
    /// - Environment variables: PATH, SHELL, HOME, USER, PWD
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let shell = detect_shell();
        let os = detect_os();
        let hostname = detect_hostname();
        let env_vars = capture_env_vars();

        Self {
            header: SessionHeader {
                version: 2,
                id: Uuid::new_v4(),
                name: name.to_string(),
                shell,
                os,
                hostname,
                env: env_vars,
                tags: Vec::new(),
                recovered: None,
                started_at,
            },
            commands: Vec::new(),
            footer: None,
        }
    }

    /// Add a command to the session.
    pub fn add_command(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    /// Mark the session as complete with the given status.
    ///
    /// Creates the footer with the current timestamp and command count.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    pub fn complete(&mut self, status: SessionStatus) {
        let ended_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        self.footer = Some(SessionFooter {
            ended_at,
            command_count: self.commands.len() as u32,
            status,
        });
    }

    /// Get the session ID.
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.header.id
    }

    /// Get the session name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.header.name
    }

    /// Check if the session is still recording.
    #[must_use]
    pub fn is_recording(&self) -> bool {
        self.footer.is_none()
    }
}

/// Detect shell type and version from environment.
fn detect_shell() -> String {
    let shell_path = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    // Try to get version by running shell --version
    // For simplicity, just return shell name without version for now
    shell_path.rsplit('/').next().unwrap_or("sh").to_string()
}

/// Detect operating system information.
fn detect_os() -> String {
    // Try to read from /etc/os-release first (Linux)
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let value = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                return value.to_string();
            }
        }
    }

    // Fallback to basic info from uname
    let os_type = env::consts::OS;
    let arch = env::consts::ARCH;
    format!("{os_type} {arch}")
}

/// Detect machine hostname.
fn detect_hostname() -> String {
    // Try /etc/hostname first
    if let Ok(hostname) = fs::read_to_string("/etc/hostname") {
        let hostname = hostname.trim();
        if !hostname.is_empty() {
            return hostname.to_string();
        }
    }

    // Try HOSTNAME env var
    if let Ok(hostname) = env::var("HOSTNAME") {
        return hostname;
    }

    // Final fallback
    "unknown".to_string()
}

/// Capture selected environment variables.
fn capture_env_vars() -> HashMap<String, String> {
    let mut env_vars = HashMap::new();
    let keys = ["PATH", "SHELL", "HOME", "USER", "PWD"];

    for key in keys {
        if let Ok(value) = env::var(key) {
            env_vars.insert(key.to_string(), value);
        }
    }

    env_vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new("test-session");

        assert_eq!(session.header.version, 2);
        assert_eq!(session.header.name, "test-session");
        assert!(session.header.started_at > 0.0);
        assert!(session.commands.is_empty());
        assert!(session.footer.is_none());
        assert!(session.is_recording());
    }

    #[test]
    fn test_session_add_command() {
        let mut session = Session::new("test-session");
        let cmd = Command::new(
            0,
            "echo hello".to_string(),
            std::path::PathBuf::from("/home"),
        );

        session.add_command(cmd);

        assert_eq!(session.commands.len(), 1);
        assert_eq!(session.commands[0].command, "echo hello");
    }

    #[test]
    fn test_session_complete() {
        let mut session = Session::new("test-session");
        session.add_command(Command::new(
            0,
            "echo hello".to_string(),
            std::path::PathBuf::from("/home"),
        ));

        session.complete(SessionStatus::Completed);

        assert!(session.footer.is_some());
        let footer = session.footer.as_ref().unwrap();
        assert_eq!(footer.command_count, 1);
        assert_eq!(footer.status, SessionStatus::Completed);
        assert!(!session.is_recording());
    }

    #[test]
    fn test_session_serialization() {
        let mut session = Session::new("test-session");
        session.complete(SessionStatus::Completed);

        let json = serde_json::to_string(&session).expect("Failed to serialize");
        assert!(json.contains("\"name\":\"test-session\""));
        assert!(json.contains("\"version\":2"));

        let deserialized: Session = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.header.name, session.header.name);
    }

    #[test]
    fn test_session_status_serialization() {
        let status = SessionStatus::Completed;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, "\"completed\"");

        let deserialized: SessionStatus =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized, SessionStatus::Completed);
    }

    #[test]
    fn test_generate_session_name() {
        let name = generate_session_name();
        assert!(name.starts_with("session-"));
        // Format: session-YYYY-MM-DD-HHMMSS (25 chars)
        assert_eq!(name.len(), 25);
        // Should be a valid session name
        assert!(validate_session_name(&name).is_ok());
    }

    #[test]
    fn test_validate_session_name_valid() {
        assert!(validate_session_name("my-session").is_ok());
        assert!(validate_session_name("test_123").is_ok());
        assert!(validate_session_name("MySession").is_ok());
        assert!(validate_session_name("session-2026-01-26-143052").is_ok());
        assert!(validate_session_name("a").is_ok());
    }

    #[test]
    fn test_validate_session_name_invalid() {
        assert!(validate_session_name("my session").is_err()); // space
        assert!(validate_session_name("test@123").is_err()); // special char
        assert!(validate_session_name("").is_err()); // empty
        assert!(validate_session_name("hello/world").is_err()); // slash
        assert!(validate_session_name("name.ext").is_err()); // dot
    }

    #[test]
    fn test_recovered_none_omitted_from_json() {
        let session = Session::new("test-serde-skip");
        assert!(session.header.recovered.is_none());

        let json = serde_json::to_string(&session.header).expect("Failed to serialize");
        assert!(
            !json.contains("\"recovered\""),
            "JSON should not contain '\"recovered\"' key when None: {json}"
        );
    }

    #[test]
    fn test_recovered_some_true_serializes() {
        let mut session = Session::new("test-recovered-true");
        session.header.recovered = Some(true);

        let json = serde_json::to_string(&session.header).expect("Failed to serialize");
        assert!(
            json.contains("\"recovered\":true"),
            "JSON should contain 'recovered':true: {json}"
        );
    }

    #[test]
    fn test_recovered_backward_compat_deserialization() {
        // JSON without 'recovered' field — simulates old session files
        let json = r#"{
            "version": 2,
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "old-session",
            "shell": "bash",
            "os": "linux",
            "hostname": "test",
            "env": {},
            "tags": [],
            "started_at": 1700000000.0
        }"#;

        let header: SessionHeader =
            serde_json::from_str(json).expect("Should deserialize without recovered field");
        assert_eq!(header.recovered, None);
        assert_eq!(header.name, "old-session");
    }
}
