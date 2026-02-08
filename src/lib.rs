// Pedantic lints — enabled globally, with pragmatic exceptions
#![warn(clippy::pedantic)]
// These are noisy in a CLI app and don't improve readability or safety:
#![allow(clippy::module_name_repetitions)] // e.g. RecError in rec::error is fine
#![allow(clippy::struct_excessive_bools)] // CLI option structs legitimately have many bools
#![allow(clippy::too_many_lines)] // will be addressed when main.rs is refactored
#![allow(clippy::cast_possible_truncation)] // u64→u32 etc. are intentional in duration/size calcs
#![allow(clippy::cast_sign_loss)] // f64→u64 for timestamps is intentional
#![allow(clippy::cast_precision_loss)] // u64→f64 for display purposes is acceptable
#![allow(clippy::cast_possible_wrap)] // usize→i64 is intentional
#![allow(clippy::similar_names)] // short variable names in tight scopes are fine
#![allow(clippy::unreadable_literal)] // timestamp literals like 1700000000.0 are clearer without separators

//! # rec — CLI Terminal Recorder
//!
//! `rec` is a command-line tool for recording, replaying, and exporting terminal
//! sessions. It captures commands as you work and lets you export them to
//! executable formats like bash scripts, Makefiles, and CI/CD pipelines.
//!
//! ## Architecture
//!
//! - [`cli`] — Command-line argument parsing and dispatch (clap-based)
//! - [`config`] — Configuration loading and management (TOML-based)
//! - [`recording`] — Session recording lifecycle (start/stop/status/hook)
//! - [`session`] — Session data model and storage operations
//! - [`storage`] — XDG-compliant filesystem paths and persistence
//! - [`replay`] — Command replay engine with safety controls
//! - [`export`] — Multi-format export (bash, makefile, markdown, etc.)
//! - [`import`] — Import sessions from shell history files
//! - [`doctor`] — Diagnostic checks for installation health
//! - [`hooks`] — Shell hook scripts for bash, zsh, and fish
//! - [`error`] — Error types with semantic exit codes
//! - [`models`] — Shared data models (sessions, commands, config)
//! - [`demo`] — Demo session generation for testing

pub mod cli;
pub mod config;
pub mod demo;
pub mod doctor;
pub mod error;
pub mod export;
pub mod hooks;
pub mod import;
pub mod models;
pub mod recording;
pub mod replay;
pub mod session;
pub mod storage;
