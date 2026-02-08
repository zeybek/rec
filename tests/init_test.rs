//! Integration tests for init shell hook generation (CONF-07 through CONF-10).

use rec::cli::Shell;
use rec::hooks::get_hook_script;

/// CONF-07: Init bash generates hook script with preexec/precmd.
#[test]
fn test_init_bash_generates_hooks() {
    let script = get_hook_script(Shell::Bash);

    assert!(
        script.contains("__rec_preexec"),
        "Should define preexec function"
    );
    assert!(
        script.contains("__rec_precmd"),
        "Should define precmd function"
    );
    assert!(
        script.contains("rec _hook preexec"),
        "Should call rec _hook preexec"
    );
    assert!(
        script.contains("rec _hook precmd"),
        "Should call rec _hook precmd"
    );
    assert!(
        script.contains("preexec_functions"),
        "Should register with bash-preexec"
    );
}

/// CONF-08: Init zsh generates hook script with zsh-specific patterns.
#[test]
fn test_init_zsh_generates_hooks() {
    let script = get_hook_script(Shell::Zsh);

    assert!(script.contains("add-zsh-hook"), "Should use add-zsh-hook");
    assert!(
        script.contains("__rec_preexec"),
        "Should define preexec function"
    );
    assert!(
        script.contains("__rec_precmd"),
        "Should define precmd function"
    );
    assert!(
        script.contains("add-zsh-hook preexec"),
        "Should register preexec hook"
    );
    assert!(
        script.contains("add-zsh-hook precmd"),
        "Should register precmd hook"
    );
}

/// CONF-09: Init fish generates hook script with fish event handlers.
#[test]
fn test_init_fish_generates_hooks() {
    let script = get_hook_script(Shell::Fish);

    assert!(
        script.contains("--on-event fish_preexec"),
        "Should use fish_preexec event"
    );
    assert!(
        script.contains("--on-event fish_postexec"),
        "Should use fish_postexec event"
    );
    assert!(
        script.contains("rec _hook preexec"),
        "Should call rec _hook preexec"
    );
    assert!(
        script.contains("rec _hook precmd"),
        "Should call rec _hook precmd"
    );
}

/// CONF-10: All shell outputs are non-empty and well-formed.
#[test]
fn test_init_output_is_evaluable() {
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        let script = get_hook_script(shell);

        // Non-empty
        assert!(
            !script.trim().is_empty(),
            "Script should not be empty for {shell:?}"
        );

        // Substantial content (at least 20 lines)
        let line_count = script.lines().count();
        assert!(
            line_count > 20,
            "Script for {shell:?} should have >20 lines, got {line_count}"
        );

        // Contains REC_RECORDING check (all shells guard on this)
        assert!(
            script.contains("REC_RECORDING"),
            "Script for {shell:?} should check REC_RECORDING"
        );
    }
}
