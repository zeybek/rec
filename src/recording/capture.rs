use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::models::command::Command;
use crate::models::session::{SessionFooter, SessionHeader};
use crate::storage::set_restrictive_permissions;

/// An NDJSON line in the session file.
///
/// Each line is tagged with `type` for easy parsing:
/// - `header`: Session metadata (first line)
/// - `command`: A captured command (middle lines)
/// - `footer`: Session summary (last line)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NdjsonLine {
    /// Session header metadata
    Header(SessionHeader),
    /// A captured command
    Command(Command),
    /// Session footer summary
    Footer(SessionFooter),
}

/// Command capture with durable NDJSON append.
///
/// Provides static methods for writing NDJSON lines to session files
/// with `fsync` after each write for crash safety. Each method opens
/// the file, writes a single NDJSON line, syncs to disk, and closes.
///
/// # File Format
///
/// ```text
/// {"type":"header","version":2,"id":"...","name":"...","shell":"bash",...}
/// {"type":"command","index":0,"command":"echo hello","cwd":"/home",...}
/// {"type":"command","index":1,"command":"ls -la","cwd":"/home",...}
/// {"type":"footer","ended_at":1234567890.123,"command_count":2,"status":"completed"}
/// ```
pub struct CommandCapture;

impl CommandCapture {
    /// Write the session header as the first line of the NDJSON file.
    ///
    /// Creates the file (or truncates if exists) and writes the header
    /// line followed by `sync_all()` for durability. Sets restrictive
    /// permissions (0o600) to prevent other users from reading session data.
    ///
    /// # Errors
    ///
    /// Returns an error if file creation or writing fails.
    pub fn write_header(path: &Path, header: &SessionHeader) -> Result<()> {
        let line = NdjsonLine::Header(header.clone());
        let file = File::create(path)?;
        let mut writer = BufWriter::new(&file);
        serde_json::to_writer(&mut writer, &line)?;
        writeln!(writer)?;
        writer.flush()?;
        file.sync_all()?;

        // Set restrictive permissions (0o600) to prevent other users from reading
        set_restrictive_permissions(path)?;

        Ok(())
    }

    /// Append a command as an NDJSON line to the session file.
    ///
    /// Opens the file in append mode and writes a single command line
    /// followed by `sync_all()` for durability.
    ///
    /// # Errors
    ///
    /// Returns an error if file opening or writing fails.
    pub fn append_command(path: &Path, command: &Command) -> Result<()> {
        let line = NdjsonLine::Command(command.clone());
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut writer = BufWriter::new(&file);
        serde_json::to_writer(&mut writer, &line)?;
        writeln!(writer)?;
        writer.flush()?;
        file.sync_all()?;
        Ok(())
    }

    /// Write the session footer as the last line of the NDJSON file.
    ///
    /// Opens the file in append mode and writes the footer line
    /// followed by `sync_all()` for durability.
    ///
    /// # Errors
    ///
    /// Returns an error if file opening or writing fails.
    pub fn write_footer(path: &Path, footer: &SessionFooter) -> Result<()> {
        let line = NdjsonLine::Footer(footer.clone());
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut writer = BufWriter::new(&file);
        serde_json::to_writer(&mut writer, &line)?;
        writeln!(writer)?;
        writer.flush()?;
        file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::session::SessionStatus;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn sample_header() -> SessionHeader {
        SessionHeader {
            version: 2,
            id: Uuid::new_v4(),
            name: "test-session".to_string(),
            shell: "bash".to_string(),
            os: "linux".to_string(),
            hostname: "testhost".to_string(),
            env: HashMap::new(),
            tags: vec!["test".to_string()],
            recovered: None,
            started_at: 1700000000.123,
        }
    }

    fn sample_command(index: u32) -> Command {
        Command {
            index,
            command: format!("echo command-{index}"),
            cwd: PathBuf::from("/home/user"),
            started_at: 1700000000.123 + (f64::from(index) * 1.0),
            ended_at: Some(1700000000.223 + (f64::from(index) * 1.0)),
            exit_code: Some(0),
            duration_ms: Some(100),
        }
    }

    fn sample_footer(count: u32) -> SessionFooter {
        SessionFooter {
            ended_at: 1700000010.456,
            command_count: count,
            status: SessionStatus::Completed,
        }
    }

    #[test]
    fn test_write_header() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");
        let header = sample_header();

        CommandCapture::write_header(&path, &header).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let parsed: NdjsonLine = serde_json::from_str(lines[0]).unwrap();
        match parsed {
            NdjsonLine::Header(h) => {
                assert_eq!(h.name, "test-session");
                assert_eq!(h.version, 2);
                assert_eq!(h.shell, "bash");
            }
            _ => panic!("Expected Header line"),
        }
    }

    #[test]
    fn test_append_command() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");
        let header = sample_header();

        CommandCapture::write_header(&path, &header).unwrap();
        CommandCapture::append_command(&path, &sample_command(0)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let parsed: NdjsonLine = serde_json::from_str(lines[1]).unwrap();
        match parsed {
            NdjsonLine::Command(c) => {
                assert_eq!(c.index, 0);
                assert_eq!(c.command, "echo command-0");
                assert_eq!(c.exit_code, Some(0));
            }
            _ => panic!("Expected Command line"),
        }
    }

    #[test]
    fn test_write_footer() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");
        let header = sample_header();

        CommandCapture::write_header(&path, &header).unwrap();
        CommandCapture::append_command(&path, &sample_command(0)).unwrap();
        CommandCapture::write_footer(&path, &sample_footer(1)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        let parsed: NdjsonLine = serde_json::from_str(lines[2]).unwrap();
        match parsed {
            NdjsonLine::Footer(f) => {
                assert_eq!(f.command_count, 1);
                assert_eq!(f.status, SessionStatus::Completed);
            }
            _ => panic!("Expected Footer line"),
        }
    }

    #[test]
    fn test_multiple_commands_preserve_order() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");
        let header = sample_header();

        CommandCapture::write_header(&path, &header).unwrap();
        for i in 0..5 {
            CommandCapture::append_command(&path, &sample_command(i)).unwrap();
        }
        CommandCapture::write_footer(&path, &sample_footer(5)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 7); // header + 5 commands + footer

        // Verify order: header, commands 0-4, footer
        let first: NdjsonLine = serde_json::from_str(lines[0]).unwrap();
        assert!(matches!(first, NdjsonLine::Header(_)));

        for i in 0..5 {
            let line: NdjsonLine = serde_json::from_str(lines[i + 1]).unwrap();
            match line {
                NdjsonLine::Command(c) => assert_eq!(c.index, i as u32),
                _ => panic!("Expected Command at line {}", i + 1),
            }
        }

        let last: NdjsonLine = serde_json::from_str(lines[6]).unwrap();
        assert!(matches!(last, NdjsonLine::Footer(_)));
    }

    #[test]
    fn test_ndjson_line_serialization_roundtrip() {
        let header_line = NdjsonLine::Header(sample_header());
        let json = serde_json::to_string(&header_line).unwrap();
        assert!(json.contains("\"type\":\"header\""));

        let parsed: NdjsonLine = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, NdjsonLine::Header(_)));

        let cmd_line = NdjsonLine::Command(sample_command(0));
        let json = serde_json::to_string(&cmd_line).unwrap();
        assert!(json.contains("\"type\":\"command\""));

        let footer_line = NdjsonLine::Footer(sample_footer(1));
        let json = serde_json::to_string(&footer_line).unwrap();
        assert!(json.contains("\"type\":\"footer\""));
    }

    #[test]
    fn test_write_header_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new-session.ndjson");

        assert!(!path.exists());
        CommandCapture::write_header(&path, &sample_header()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_each_line_is_valid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");

        CommandCapture::write_header(&path, &sample_header()).unwrap();
        CommandCapture::append_command(&path, &sample_command(0)).unwrap();
        CommandCapture::write_footer(&path, &sample_footer(1)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        for (i, line) in content.lines().enumerate() {
            let parsed: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("Line {i} is not valid JSON: {e}"));
            assert!(
                parsed.get("type").is_some(),
                "Line {i} missing 'type' field"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_write_header_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.ndjson");

        CommandCapture::write_header(&path, &sample_header()).unwrap();

        // Verify permissions are 0o600 (read/write for owner only)
        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode();

        // On Unix, mode includes file type bits. We only care about permission bits (lower 9 bits)
        let permission_bits = mode & 0o777;
        assert_eq!(
            permission_bits, 0o600,
            "Session file should have 0o600 permissions, got 0o{permission_bits:o}"
        );
    }
}
