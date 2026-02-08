use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single command captured during a recording session.
///
/// Each command records the command text, working directory, timing information,
/// and exit code. This matches the NDJSON schema from CONTEXT.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Command sequence number within the session (0-indexed)
    pub index: u32,

    /// The actual command text that was executed
    pub command: String,

    /// Working directory when the command was executed
    pub cwd: PathBuf,

    /// Unix timestamp with milliseconds when command started
    pub started_at: f64,

    /// Unix timestamp with milliseconds when command ended (None if still running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<f64>,

    /// Exit code of the command (None if still running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Duration in milliseconds (calculated from timestamps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl Command {
    /// Create a new command with the current timestamp.
    ///
    /// The command starts in a "running" state with `ended_at`, `exit_code`,
    /// and `duration_ms` all set to `None`.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    #[must_use]
    pub fn new(index: u32, command: String, cwd: PathBuf) -> Self {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        Self {
            index,
            command,
            cwd,
            started_at,
            ended_at: None,
            exit_code: None,
            duration_ms: None,
        }
    }

    /// Mark the command as complete with the given exit code.
    ///
    /// Sets `ended_at` to the current timestamp, records the `exit_code`,
    /// and calculates `duration_ms`.
    ///
    /// # Panics
    ///
    /// Panics if the system clock is before the Unix epoch.
    pub fn complete(&mut self, exit_code: i32) {
        let ended_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        self.ended_at = Some(ended_at);
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(((ended_at - self.started_at) * 1000.0) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_command_new() {
        let cmd = Command::new(0, "echo hello".to_string(), PathBuf::from("/home/user"));

        assert_eq!(cmd.index, 0);
        assert_eq!(cmd.command, "echo hello");
        assert_eq!(cmd.cwd, PathBuf::from("/home/user"));
        assert!(cmd.started_at > 0.0);
        assert!(cmd.ended_at.is_none());
        assert!(cmd.exit_code.is_none());
        assert!(cmd.duration_ms.is_none());
    }

    #[test]
    fn test_command_complete() {
        let mut cmd = Command::new(0, "echo hello".to_string(), PathBuf::from("/home/user"));

        // Small delay to ensure measurable duration
        sleep(Duration::from_millis(10));

        cmd.complete(0);

        assert!(cmd.ended_at.is_some());
        assert_eq!(cmd.exit_code, Some(0));
        assert!(cmd.duration_ms.is_some());
        assert!(cmd.duration_ms.unwrap() >= 10);
    }

    #[test]
    fn test_command_serialization() {
        let mut cmd = Command::new(0, "echo hello".to_string(), PathBuf::from("/home/user"));
        cmd.complete(0);

        let json = serde_json::to_string(&cmd).expect("Failed to serialize");
        assert!(json.contains("\"command\":\"echo hello\""));
        assert!(json.contains("\"exit_code\":0"));

        let deserialized: Command = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.command, cmd.command);
        assert_eq!(deserialized.exit_code, cmd.exit_code);
    }
}
