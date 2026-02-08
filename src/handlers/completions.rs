use std::process::ExitCode;

/// Handle the `completions` command — generates shell completions.
pub fn handle_completions(shell: rec::cli::Shell) -> ExitCode {
    use clap::CommandFactory;
    use clap_complete::generate;
    use rec::cli::Cli;

    let mut cmd = Cli::command();
    let shell_type = match shell {
        rec::cli::Shell::Bash => clap_complete::Shell::Bash,
        rec::cli::Shell::Zsh => clap_complete::Shell::Zsh,
        rec::cli::Shell::Fish => clap_complete::Shell::Fish,
    };

    generate(shell_type, &mut cmd, "rec", &mut std::io::stdout());
    ExitCode::SUCCESS
}
