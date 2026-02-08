//! Import sessions from external sources.
//!
//! Supports importing from Bash scripts, Bash/Zsh/Fish history files,
//! and auto-detects the input format.

pub mod bash_history;
pub mod bash_script;
pub mod detect;
pub mod fish_history;
pub mod importer;
pub mod zsh_history;

pub use detect::*;
pub use importer::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Paths, SessionStore};
    use std::path::Path;
    use tempfile::TempDir;

    // ─── Format detection tests ──────────────────────────────

    #[test]
    fn test_detect_cast_extension_rejected() {
        let result = detect_format("recording.cast", "");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not currently supported"), "Error was: {err}");
    }

    #[test]
    fn test_detect_sh_extension() {
        let result = detect_format("deploy.sh", "echo hello").unwrap();
        assert_eq!(result, ImportFormat::BashScript);
    }

    #[test]
    fn test_detect_bash_extension() {
        let result = detect_format("setup.bash", "echo hello").unwrap();
        assert_eq!(result, ImportFormat::BashScript);
    }

    #[test]
    fn test_detect_shebang_bash() {
        let result = detect_format("myscript", "#!/bin/bash\necho hello").unwrap();
        assert_eq!(result, ImportFormat::BashScript);
    }

    #[test]
    fn test_detect_shebang_sh() {
        let result = detect_format("myscript", "#!/bin/sh\necho hello").unwrap();
        assert_eq!(result, ImportFormat::BashScript);
    }

    #[test]
    fn test_detect_shebang_env_bash() {
        let result = detect_format("myscript", "#!/usr/bin/env bash\necho hello").unwrap();
        assert_eq!(result, ImportFormat::BashScript);
    }

    #[test]
    fn test_detect_zsh_extended() {
        let content = ": 1458291931:0;ls -l\n: 1458291945:3;git push";
        let result = detect_format("history", content).unwrap();
        assert_eq!(result, ImportFormat::ZshHistory);
    }

    #[test]
    fn test_detect_fish_history() {
        let content = "- cmd: ls -la\n  when: 123";
        let result = detect_format("fish_history", content).unwrap();
        assert_eq!(result, ImportFormat::FishHistory);
    }

    #[test]
    fn test_detect_fallback_bash_history() {
        let content = "ls -la\ngit status";
        let result = detect_format("history", content).unwrap();
        assert_eq!(result, ImportFormat::BashHistory);
    }

    // ─── Session name generation tests ───────────────────────

    #[test]
    fn test_name_from_sh_file() {
        let name = session_name_from_path(Path::new("deploy.sh"));
        assert_eq!(name, "deploy-sh");
    }

    #[test]
    fn test_name_from_hidden_file() {
        let name = session_name_from_path(Path::new(".bash_history"));
        assert_eq!(name, "bash_history");
    }

    #[test]
    fn test_name_from_zsh_hidden() {
        let name = session_name_from_path(Path::new(".zsh_history"));
        assert_eq!(name, "zsh_history");
    }

    #[test]
    fn test_name_with_spaces_and_parens() {
        let name = session_name_from_path(Path::new("my script (v2).sh"));
        assert_eq!(name, "my-script-v2-sh");
    }

    #[test]
    fn test_name_preserves_underscores() {
        let name = session_name_from_path(Path::new("my_script.sh"));
        assert_eq!(name, "my_script-sh");
    }

    #[test]
    fn test_name_from_full_path() {
        let name = session_name_from_path(Path::new("/home/user/.bash_history"));
        assert_eq!(name, "bash_history");
    }

    #[test]
    fn test_name_lowercased() {
        let name = session_name_from_path(Path::new("DeployScript.SH"));
        assert_eq!(name, "deployscript-sh");
    }

    // ─── ImportFormat Display tests ──────────────────────────

    #[test]
    fn test_format_display() {
        assert_eq!(ImportFormat::BashScript.to_string(), "bash-script");
        assert_eq!(ImportFormat::BashHistory.to_string(), "bash-history");
        assert_eq!(ImportFormat::ZshHistory.to_string(), "zsh-history");
        assert_eq!(ImportFormat::FishHistory.to_string(), "fish-history");
    }

    // ─── import_file integration tests ───────────────────────

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    #[test]
    fn test_import_bash_script_file() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let script_path = temp_dir.path().join("deploy.sh");
        std::fs::write(&script_path, "#!/bin/bash\n# setup\necho hello\ngit push\n").unwrap();

        let result = import_file(&script_path, None, &store).unwrap();

        assert_eq!(result.session_name, "deploy-sh");
        assert_eq!(result.command_count, 2);
        assert_eq!(result.format, ImportFormat::BashScript);
        assert_eq!(result.preview_commands, vec!["echo hello", "git push"]);
    }

    #[test]
    fn test_import_with_name_override() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let script_path = temp_dir.path().join("script.sh");
        std::fs::write(&script_path, "echo hello\n").unwrap();

        let result = import_file(&script_path, Some("custom-name"), &store).unwrap();
        assert_eq!(result.session_name, "custom-name");
    }

    #[test]
    fn test_import_empty_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let script_path = temp_dir.path().join("empty.sh");
        std::fs::write(&script_path, "#!/bin/bash\n# only comments\n").unwrap();

        let result = import_file(&script_path, None, &store);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No commands found"), "Error was: {err}");
    }

    #[test]
    fn test_import_preview_capped_at_5() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let script_path = temp_dir.path().join("many.sh");
        std::fs::write(
            &script_path,
            "cmd1\ncmd2\ncmd3\ncmd4\ncmd5\ncmd6\ncmd7\ncmd8\n",
        )
        .unwrap();

        let result = import_file(&script_path, None, &store).unwrap();
        assert_eq!(result.command_count, 8);
        assert_eq!(result.preview_commands.len(), 5);
    }

    #[test]
    fn test_import_cast_file_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let cast_path = temp_dir.path().join("recording.cast");
        std::fs::write(&cast_path, "some asciinema content").unwrap();

        let result = import_file(&cast_path, None, &store);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not currently supported"), "Error was: {err}");
    }

    #[test]
    fn test_import_session_persisted() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let script_path = temp_dir.path().join("test.sh");
        std::fs::write(&script_path, "echo hello\nls\n").unwrap();

        let result = import_file(&script_path, None, &store).unwrap();

        // Verify session was actually saved
        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 1);

        // Load and verify content
        let loaded = store.load(&sessions[0]).unwrap();
        assert_eq!(loaded.header.name, result.session_name);
        assert_eq!(loaded.commands.len(), 2);
        assert!(loaded.footer.is_some());
    }
}
