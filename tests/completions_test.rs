//! Integration tests for shell completion generation (CONF-11 to CONF-13).

use clap::CommandFactory;
use clap_complete::generate;
use rec::cli::Cli;

/// CONF-11: Bash completions generate a valid script with completion registration.
#[test]
fn test_completions_bash_generates_script() {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(clap_complete::Shell::Bash, &mut cmd, "rec", &mut buf);
    let output = String::from_utf8(buf).expect("Bash completions should be valid UTF-8");

    assert!(!output.is_empty(), "Bash completions should not be empty");
    assert!(
        output.contains("_rec"),
        "Bash completions should contain _rec function"
    );
    assert!(
        output.contains("complete -F") || output.contains("complete -o"),
        "Bash completions should register with 'complete' builtin"
    );
}

/// CONF-12: Zsh completions generate a valid script with compdef or function.
#[test]
fn test_completions_zsh_generates_script() {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(clap_complete::Shell::Zsh, &mut cmd, "rec", &mut buf);
    let output = String::from_utf8(buf).expect("Zsh completions should be valid UTF-8");

    assert!(!output.is_empty(), "Zsh completions should not be empty");
    assert!(
        output.contains("#compdef") || output.contains("_rec"),
        "Zsh completions should contain #compdef or _rec function"
    );
}

/// CONF-13: Fish completions generate valid `complete -c rec` commands.
#[test]
fn test_completions_fish_generates_script() {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(clap_complete::Shell::Fish, &mut cmd, "rec", &mut buf);
    let output = String::from_utf8(buf).expect("Fish completions should be valid UTF-8");

    assert!(!output.is_empty(), "Fish completions should not be empty");
    assert!(
        output.contains("complete -c rec"),
        "Fish completions should contain 'complete -c rec' patterns"
    );
}
