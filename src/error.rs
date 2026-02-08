use thiserror::Error;

/// Exit code for successful execution
pub const EXIT_SUCCESS: u8 = 0;
/// Exit code for user errors (bad input, not found, invalid state)
pub const EXIT_USER_ERROR: u8 = 1;
/// Exit code for system errors (I/O, permissions, corrupt data)
pub const EXIT_SYSTEM_ERROR: u8 = 2;
/// Exit code for interrupted execution (Ctrl+C)
pub const EXIT_INTERRUPTED: u8 = 130;

/// Error types for the rec CLI.
///
/// Covers all anticipated failure modes including:
/// - I/O errors (file operations)
/// - Serialization errors (JSON, TOML)
/// - Domain errors (session management, recording state)
#[derive(Error, Debug)]
pub enum RecError {
    /// I/O error during file operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Session not found by name or ID
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already exists with this name
    #[error("Session already exists: {0}")]
    SessionExists(String),

    /// Invalid session file format
    #[error("Invalid session format: {0}")]
    InvalidSession(String),

    /// Configuration error
    #[error("Config error: {0}")]
    Config(String),

    /// Attempted to start recording while already recording
    #[error("Recording already in progress")]
    RecordingInProgress,

    /// Attempted to stop recording when not recording
    #[error("No active recording")]
    NoActiveRecording,

    /// Stale lock file from a crashed process (includes PID)
    #[error("Stale lock from process {0}")]
    StaleLock(String),

    /// Session name contains invalid characters
    #[error("Invalid session name '{0}': only alphanumeric, dash, and underscore allowed")]
    InvalidSessionName(String),

    /// Alias name contains invalid characters
    #[error("Invalid alias name '{0}': only alphanumeric, dash, and underscore allowed")]
    InvalidAliasName(String),

    /// Tag name contains invalid characters after normalization
    #[error("Invalid tag name '{0}': only alphanumeric, dash, and underscore allowed")]
    InvalidTagName(String),
}

impl RecError {
    /// Return the appropriate exit code for this error.
    ///
    /// Exit code semantics:
    /// - `EXIT_USER_ERROR` (1) = user error (bad input, not found, invalid state)
    /// - `EXIT_SYSTEM_ERROR` (2) = system error (I/O failure, permissions, corrupt data)
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            RecError::Io(_) | RecError::Json(_) | RecError::Toml(_) | RecError::StaleLock(_) => {
                EXIT_SYSTEM_ERROR
            }
            RecError::SessionNotFound(_)
            | RecError::SessionExists(_)
            | RecError::InvalidSession(_)
            | RecError::Config(_)
            | RecError::RecordingInProgress
            | RecError::NoActiveRecording
            | RecError::InvalidSessionName(_)
            | RecError::InvalidAliasName(_)
            | RecError::InvalidTagName(_) => EXIT_USER_ERROR,
        }
    }
}

/// Result type alias using `RecError`
pub type Result<T> = std::result::Result<T, RecError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_user_errors() {
        assert_eq!(
            RecError::SessionNotFound("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
        assert_eq!(
            RecError::SessionExists("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
        assert_eq!(
            RecError::InvalidSession("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
        assert_eq!(RecError::Config("x".into()).exit_code(), EXIT_USER_ERROR);
        assert_eq!(RecError::RecordingInProgress.exit_code(), EXIT_USER_ERROR);
        assert_eq!(RecError::NoActiveRecording.exit_code(), EXIT_USER_ERROR);
        assert_eq!(
            RecError::InvalidSessionName("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
        assert_eq!(
            RecError::InvalidAliasName("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
        assert_eq!(
            RecError::InvalidTagName("x".into()).exit_code(),
            EXIT_USER_ERROR
        );
    }

    #[test]
    fn test_exit_code_system_errors() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        assert_eq!(RecError::Io(io_err).exit_code(), EXIT_SYSTEM_ERROR);

        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        assert_eq!(RecError::Json(json_err).exit_code(), EXIT_SYSTEM_ERROR);

        assert_eq!(
            RecError::StaleLock("1234".into()).exit_code(),
            EXIT_SYSTEM_ERROR
        );
    }

    #[test]
    fn test_error_display() {
        let err = RecError::SessionNotFound("test-session".to_string());
        assert_eq!(err.to_string(), "Session not found: test-session");

        let err = RecError::RecordingInProgress;
        assert_eq!(err.to_string(), "Recording already in progress");

        let err = RecError::Config("invalid key".to_string());
        assert_eq!(err.to_string(), "Config error: invalid key");

        let err = RecError::InvalidAliasName("bad/alias".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid alias name 'bad/alias': only alphanumeric, dash, and underscore allowed"
        );

        let err = RecError::InvalidTagName("bad@tag".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid tag name 'bad@tag': only alphanumeric, dash, and underscore allowed"
        );
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let rec_err: RecError = io_err.into();

        match rec_err {
            RecError::Io(_) => {}
            _ => panic!("Expected RecError::Io"),
        }
    }

    #[test]
    fn test_json_error_conversion() {
        let json_str = "{ invalid json }";
        let json_result: std::result::Result<serde_json::Value, _> = serde_json::from_str(json_str);
        let json_err = json_result.unwrap_err();
        let rec_err: RecError = json_err.into();

        match rec_err {
            RecError::Json(_) => {}
            _ => panic!("Expected RecError::Json"),
        }
    }

    #[test]
    fn test_result_type() {
        fn returns_error() -> Result<()> {
            Err(RecError::NoActiveRecording)
        }

        assert!(returns_error().is_err());
    }
}
