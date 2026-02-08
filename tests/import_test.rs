//! Integration tests for `rec import` command (EXIM-13 through EXIM-19).

mod common;

use common::TestEnv;
use rec::import::{ImportFormat, import_file};

// ---------------------------------------------------------------------------
// EXIM-13: import bash script creates session
// ---------------------------------------------------------------------------

#[test]
fn test_import_bash_script_creates_session() {
    let env = TestEnv::new();
    let script_path = env.paths.state_dir.join("deploy.sh");
    std::fs::write(
        &script_path,
        "#!/bin/bash\necho hello\nnpm install\nnpm test\n",
    )
    .unwrap();

    let result = import_file(&script_path, None, &env.store).unwrap();

    assert_eq!(result.format, ImportFormat::BashScript);
    assert_eq!(result.command_count, 3);
    assert!(
        result.session_name.contains("deploy"),
        "session name '{}' should contain 'deploy'",
        result.session_name
    );

    // Verify session persisted
    let sessions = env.store.list().unwrap();
    assert_eq!(sessions.len(), 1);
}

// ---------------------------------------------------------------------------
// EXIM-14: import bash history parses lines
// ---------------------------------------------------------------------------

#[test]
fn test_import_bash_history_parses_lines() {
    let env = TestEnv::new();
    let hist_path = env.paths.state_dir.join("bash_history");
    std::fs::write(&hist_path, "ls -la\ncd /tmp\npwd\n").unwrap();

    let result = import_file(&hist_path, None, &env.store).unwrap();

    assert_eq!(result.format, ImportFormat::BashHistory);
    assert_eq!(result.command_count, 3);
}

// ---------------------------------------------------------------------------
// EXIM-15: import zsh extended history
// ---------------------------------------------------------------------------

#[test]
fn test_import_zsh_extended_history() {
    let env = TestEnv::new();
    let hist_path = env.paths.state_dir.join("zsh_history");
    std::fs::write(
        &hist_path,
        ": 1458291931:0;ls -l\n: 1458291945:3;git push\n",
    )
    .unwrap();

    let result = import_file(&hist_path, None, &env.store).unwrap();

    assert_eq!(result.format, ImportFormat::ZshHistory);
    assert_eq!(result.command_count, 2);
}

// ---------------------------------------------------------------------------
// EXIM-16: import fish YAML history
// ---------------------------------------------------------------------------

#[test]
fn test_import_fish_history() {
    let env = TestEnv::new();
    let hist_path = env.paths.state_dir.join("fish_history");
    std::fs::write(
        &hist_path,
        "- cmd: ls -la\n  when: 123\n- cmd: pwd\n  when: 124\n",
    )
    .unwrap();

    let result = import_file(&hist_path, None, &env.store).unwrap();

    assert_eq!(result.format, ImportFormat::FishHistory);
    assert_eq!(result.command_count, 2);
}

// ---------------------------------------------------------------------------
// EXIM-17: import --name sets session name
// ---------------------------------------------------------------------------

#[test]
fn test_import_name_override_sets_session_name() {
    let env = TestEnv::new();
    let script_path = env.paths.state_dir.join("script.sh");
    std::fs::write(&script_path, "echo hello\nls\n").unwrap();

    let result = import_file(&script_path, Some("custom-name"), &env.store).unwrap();

    assert_eq!(result.session_name, "custom-name");
}

// ---------------------------------------------------------------------------
// EXIM-18: import auto-detects format from content
// ---------------------------------------------------------------------------

#[test]
fn test_import_autodetects_format_from_shebang() {
    let env = TestEnv::new();
    // No .sh extension, but has bash shebang
    let script_path = env.paths.state_dir.join("myscript");
    std::fs::write(&script_path, "#!/bin/bash\necho hello\ngit push\n").unwrap();

    let result = import_file(&script_path, None, &env.store).unwrap();

    assert_eq!(result.format, ImportFormat::BashScript);
    assert_eq!(result.command_count, 2);
}

// ---------------------------------------------------------------------------
// EXIM-19: import invalid/empty file returns error
// ---------------------------------------------------------------------------

#[test]
fn test_import_empty_file_returns_error() {
    let env = TestEnv::new();
    let script_path = env.paths.state_dir.join("empty.sh");
    std::fs::write(&script_path, "#!/bin/bash\n# only comments\n").unwrap();

    let result = import_file(&script_path, None, &env.store);

    assert!(result.is_err(), "importing empty file should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("No commands found"),
        "error should mention no commands, got: {err}"
    );
}
