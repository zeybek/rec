//! Configuration loading and management.
//!
//! Loads layered TOML config from `~/.config/rec/config.toml` with
//! environment variable overrides, validation, and in-place editing.

pub mod loader;

pub use loader::*;
