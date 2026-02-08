use clap::CommandFactory;
use clap_complete::{Shell, generate_to};
use std::path::PathBuf;

fn main() {
    let outdir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "completions".to_string());
    let outdir = PathBuf::from(outdir);
    std::fs::create_dir_all(&outdir).expect("Failed to create output directory");

    let mut cmd = rec::cli::Cli::command();

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        let path =
            generate_to(shell, &mut cmd, "rec", &outdir).expect("Failed to generate completions");
        println!("Generated: {}", path.display());
    }
}
