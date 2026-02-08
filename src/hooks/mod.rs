//! Shell hook scripts for Bash, Zsh, and Fish.
//!
//! Generates shell-specific hook code that integrates with `rec` to
//! transparently capture commands via `preexec`/`precmd` mechanisms.

pub mod bash_preexec;
pub mod scripts;

pub use bash_preexec::BASH_PREEXEC;
pub use scripts::*;
