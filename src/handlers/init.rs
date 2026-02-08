use super::HandlerContext;
use super::common::print_json;
use rec::cli::Shell;
use rec::error::EXIT_USER_ERROR;
use rec::hooks::{BASH_PREEXEC, get_hook_script, get_init_command};
use std::process::ExitCode;

pub fn handle_init(ctx: &HandlerContext, shell: Option<Shell>) -> ExitCode {
    // Determine shell: use provided or auto-detect
    let shell = match shell {
        Some(s) => s,
        None => {
            if let Some(s) = Shell::detect() {
                if ctx.output.is_verbose() {
                    ctx.output.debug(&format!("Detected shell: {}", s.name()));
                }
                s
            } else {
                if ctx.output.json {
                    let json = serde_json::json!({
                        "error": "unknown_shell",
                        "message": "Could not detect shell from $SHELL"
                    });
                    print_json(&json);
                } else {
                    ctx.output.error(
                        "Unknown shell",
                        "Could not detect shell from $SHELL environment variable",
                        None,
                        Some("Specify shell explicitly: rec init bash, rec init zsh, or rec init fish"),
                    );
                }
                return ExitCode::from(EXIT_USER_ERROR);
            }
        }
    };

    if ctx.output.json {
        // JSON output mode - useful for tooling/scripts
        let script = if matches!(shell, Shell::Bash) {
            format!("{}\n{}", BASH_PREEXEC, get_hook_script(shell))
        } else {
            get_hook_script(shell).to_string()
        };

        let json = serde_json::json!({
            "shell": shell.name(),
            "rc_file": shell.rc_file(),
            "init_command": get_init_command(shell),
            "script": script
        });
        print_json(&json);
    } else {
        // Normal output - script to stdout for eval
        if matches!(shell, Shell::Bash) {
            println!("{BASH_PREEXEC}");
        }

        println!("{}", get_hook_script(shell));

        // In verbose mode, show setup instructions on stderr
        if ctx.output.is_verbose() {
            eprintln!();
            eprintln!("# Add to {}:", shell.rc_file());
            eprintln!("# {}", get_init_command(shell));
        }
    }

    ExitCode::SUCCESS
}
