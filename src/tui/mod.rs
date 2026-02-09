//! Terminal User Interface for rec.
//!
//! This module provides an interactive TUI for managing recording sessions.
//! Enable with: `cargo install rec-cli --features tui`

mod app;
mod event;
mod screens;
mod ui;
mod widgets;

pub use app::{App, Screen, SessionInfo, run};
