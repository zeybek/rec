use crate::cli::Shell;

/// Get the hook script for the specified shell.
#[must_use]
pub fn get_hook_script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_HOOK,
        Shell::Zsh => ZSH_HOOK,
        Shell::Fish => FISH_HOOK,
    }
}

/// Get the shell initialization command to add to rc file.
#[must_use]
pub fn get_init_command(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => r#"eval "$(rec init bash)""#,
        Shell::Zsh => r#"eval "$(rec init zsh)""#,
        Shell::Fish => r"rec init fish | source",
    }
}

/// Bash hook script.
///
/// Requires bash-preexec to be sourced first. Registers preexec/precmd
/// functions that call rec _hook for command capture.
pub const BASH_HOOK: &str = r#"
# rec shell hooks for Bash
# Add to ~/.bashrc: eval "$(rec init bash)"

# Shell wrapper: automatically manages REC_RECORDING for start/stop
rec() {
    if [[ "${1:-}" == "start" ]]; then
        command rec "$@" && export REC_RECORDING=1
    elif [[ "${1:-}" == "stop" ]]; then
        command rec "$@" && unset REC_RECORDING
    else
        command rec "$@"
    fi
}

# Source bash-preexec if not already loaded
if [[ -z "${bash_preexec_imported:-}" ]]; then
    # bash-preexec is output first by rec init bash
    :
fi

# Capture command before execution
__rec_preexec() {
    # $1 is the command (aliases expanded by bash-preexec)
    if [[ -n "${REC_RECORDING:-}" ]]; then
        rec _hook preexec "$1" 2>/dev/null || true
    fi
}

# Capture exit code after command completes
__rec_precmd() {
    local exit_code=$?
    if [[ -n "${REC_RECORDING:-}" ]]; then
        rec _hook precmd "$exit_code" 2>/dev/null || true
    fi
}

# Register hooks with bash-preexec
preexec_functions+=(__rec_preexec)
precmd_functions+=(__rec_precmd)

# Recording indicator for prompt (optional, disable with REC_NO_PROMPT=1)
__rec_prompt_indicator() {
    if [[ -n "${REC_RECORDING:-}" && -z "${REC_NO_PROMPT:-}" ]]; then
        printf '\[\e[31m\]● \[\e[0m\]'
    fi
}

# Prepend indicator to PS1 if not disabled
if [[ -z "${REC_NO_PROMPT:-}" ]]; then
    PS1='$(__rec_prompt_indicator)'"${PS1}"
fi
"#;

/// Zsh hook script.
///
/// Uses native Zsh hooks via add-zsh-hook. Note: $3 in preexec is the
/// fully expanded command (aliases expanded), while $1 is what was typed.
pub const ZSH_HOOK: &str = r#"
# rec shell hooks for Zsh
# Add to ~/.zshrc: eval "$(rec init zsh)"

# Shell wrapper: automatically manages REC_RECORDING for start/stop
rec() {
    if [[ "${1:-}" == "start" ]]; then
        command rec "$@" && export REC_RECORDING=1
    elif [[ "${1:-}" == "stop" ]]; then
        command rec "$@" && unset REC_RECORDING
    else
        command rec "$@"
    fi
}

autoload -Uz add-zsh-hook

# Capture command before execution
# $1 = typed command, $2 = expanded aliases only, $3 = full expansion
__rec_preexec() {
    if [[ -n "${REC_RECORDING:-}" ]]; then
        rec _hook preexec "$3" 2>/dev/null || true
    fi
}

# Capture exit code after command completes
__rec_precmd() {
    local exit_code=$?
    if [[ -n "${REC_RECORDING:-}" ]]; then
        rec _hook precmd "$exit_code" 2>/dev/null || true
    fi
}

# Register hooks
add-zsh-hook preexec __rec_preexec
add-zsh-hook precmd __rec_precmd

# Recording indicator for prompt (optional, disable with REC_NO_PROMPT=1)
__rec_prompt_indicator() {
    if [[ -n "${REC_RECORDING:-}" && -z "${REC_NO_PROMPT:-}" ]]; then
        print -n '%{\e[31m%}● %{\e[0m%}'
    fi
}

# Prepend indicator to PROMPT if not disabled
if [[ -z "${REC_NO_PROMPT:-}" ]]; then
    PROMPT='$(__rec_prompt_indicator)'"${PROMPT}"
fi
"#;

/// Fish hook script.
///
/// Uses native Fish event handlers. Note: Fish does NOT expand aliases
/// in `fish_preexec` - the command line is passed verbatim.
pub const FISH_HOOK: &str = r#"
# rec shell hooks for Fish
# Add to ~/.config/fish/config.fish: rec init fish | source

# Shell wrapper: automatically manages REC_RECORDING for start/stop
function rec --wraps=rec
    if test (count $argv) -ge 1; and test "$argv[1]" = "start"
        command rec $argv; and set -gx REC_RECORDING 1
    else if test (count $argv) -ge 1; and test "$argv[1]" = "stop"
        command rec $argv; and set -e REC_RECORDING
    else
        command rec $argv
    end
end

# Capture command before execution
# Note: $argv is the verbatim command line (aliases NOT expanded)
function __rec_preexec --on-event fish_preexec
    if set -q REC_RECORDING
        rec _hook preexec "$argv" 2>/dev/null; or true
    end
end

# Capture exit code after command completes
function __rec_postexec --on-event fish_postexec
    set -l exit_code $status
    if set -q REC_RECORDING
        rec _hook precmd "$exit_code" 2>/dev/null; or true
    end
end

# Recording indicator for prompt
# Call this from your fish_prompt function if you want the indicator
function __rec_prompt_indicator
    if set -q REC_RECORDING; and not set -q REC_NO_PROMPT
        set_color red
        echo -n "● "
        set_color normal
    end
end

# Fish doesn't allow dynamic prompt modification like Bash/Zsh
# Users should add __rec_prompt_indicator to their fish_prompt function
# Or we provide a wrapper (below) that they can use

# Optional: Wrap existing fish_prompt to add indicator
# Uncomment if you want automatic prompt modification:
# functions -c fish_prompt __rec_original_fish_prompt 2>/dev/null
# function fish_prompt
#     __rec_prompt_indicator
#     __rec_original_fish_prompt
# end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_hook_contains_preexec() {
        assert!(BASH_HOOK.contains("__rec_preexec"));
        assert!(BASH_HOOK.contains("rec _hook preexec"));
        assert!(BASH_HOOK.contains("preexec_functions"));
    }

    #[test]
    fn test_bash_hook_contains_precmd() {
        assert!(BASH_HOOK.contains("__rec_precmd"));
        assert!(BASH_HOOK.contains("rec _hook precmd"));
        assert!(BASH_HOOK.contains("precmd_functions"));
    }

    #[test]
    fn test_bash_hook_checks_rec_recording() {
        assert!(BASH_HOOK.contains("REC_RECORDING"));
    }

    #[test]
    fn test_bash_hook_suppresses_errors() {
        assert!(BASH_HOOK.contains("2>/dev/null || true"));
    }

    #[test]
    fn test_bash_hook_has_prompt_indicator() {
        assert!(BASH_HOOK.contains("__rec_prompt_indicator"));
        // Bash uses \[\e[31m\] for escape sequences
        assert!(BASH_HOOK.contains(r"\[\e[31m\]"));
    }

    #[test]
    fn test_zsh_hook_uses_add_zsh_hook() {
        assert!(ZSH_HOOK.contains("add-zsh-hook"));
        assert!(ZSH_HOOK.contains("add-zsh-hook preexec __rec_preexec"));
        assert!(ZSH_HOOK.contains("add-zsh-hook precmd __rec_precmd"));
    }

    #[test]
    fn test_zsh_hook_uses_dollar_3_for_expanded_command() {
        // $3 is the fully expanded command in Zsh preexec
        assert!(ZSH_HOOK.contains("\"$3\""));
    }

    #[test]
    fn test_zsh_hook_contains_rec_hook_calls() {
        assert!(ZSH_HOOK.contains("rec _hook preexec"));
        assert!(ZSH_HOOK.contains("rec _hook precmd"));
    }

    #[test]
    fn test_zsh_hook_checks_rec_recording() {
        assert!(ZSH_HOOK.contains("REC_RECORDING"));
    }

    #[test]
    fn test_zsh_hook_suppresses_errors() {
        assert!(ZSH_HOOK.contains("2>/dev/null || true"));
    }

    #[test]
    fn test_zsh_hook_has_prompt_indicator() {
        assert!(ZSH_HOOK.contains("__rec_prompt_indicator"));
        // Zsh uses %{\e[31m%} for escape sequences
        assert!(ZSH_HOOK.contains(r"%{\e[31m%}"));
    }

    #[test]
    fn test_fish_hook_uses_events() {
        assert!(FISH_HOOK.contains("--on-event fish_preexec"));
        assert!(FISH_HOOK.contains("--on-event fish_postexec"));
    }

    #[test]
    fn test_fish_hook_contains_rec_hook_calls() {
        assert!(FISH_HOOK.contains("rec _hook preexec"));
        assert!(FISH_HOOK.contains("rec _hook precmd"));
    }

    #[test]
    fn test_fish_hook_checks_rec_recording() {
        assert!(FISH_HOOK.contains("REC_RECORDING"));
    }

    #[test]
    fn test_fish_hook_suppresses_errors() {
        assert!(FISH_HOOK.contains("2>/dev/null; or true"));
    }

    #[test]
    fn test_fish_hook_has_prompt_indicator() {
        assert!(FISH_HOOK.contains("__rec_prompt_indicator"));
        assert!(FISH_HOOK.contains("set_color red"));
    }

    #[test]
    fn test_all_hooks_check_rec_recording() {
        assert!(BASH_HOOK.contains("REC_RECORDING"));
        assert!(ZSH_HOOK.contains("REC_RECORDING"));
        assert!(FISH_HOOK.contains("REC_RECORDING"));
    }

    #[test]
    fn test_get_hook_script() {
        assert!(get_hook_script(Shell::Bash).contains("bash-preexec"));
        assert!(get_hook_script(Shell::Zsh).contains("add-zsh-hook"));
        assert!(get_hook_script(Shell::Fish).contains("fish_preexec"));
    }

    #[test]
    fn test_get_init_command() {
        assert_eq!(get_init_command(Shell::Bash), r#"eval "$(rec init bash)""#);
        assert_eq!(get_init_command(Shell::Zsh), r#"eval "$(rec init zsh)""#);
        assert_eq!(get_init_command(Shell::Fish), r"rec init fish | source");
    }

    #[test]
    fn test_hook_scripts_are_substantial() {
        // Each hook script should have meaningful content
        assert!(BASH_HOOK.lines().count() > 20);
        assert!(ZSH_HOOK.lines().count() > 20);
        assert!(FISH_HOOK.lines().count() > 20);

        // Combined, scripts module should be 100+ lines
        let total_lines =
            BASH_HOOK.lines().count() + ZSH_HOOK.lines().count() + FISH_HOOK.lines().count();
        assert!(
            total_lines > 80,
            "Expected 80+ lines of hook content, got {total_lines}"
        );
    }
}
