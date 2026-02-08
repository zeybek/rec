//! Command-line argument parsing and output formatting.
//!
//! Defines the CLI structure using `clap` derive macros, including all
//! subcommands, flags, and the [`Output`] helper for consistent formatting.

pub mod commands;
pub mod output;

pub use commands::*;
pub use output::*;
