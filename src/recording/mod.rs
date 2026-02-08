//! Session recording lifecycle management.
//!
//! Handles start/stop recording, file locking, crash recovery,
//! command capture via shell hooks, and NDJSON persistence.

pub mod capture;
pub mod state;

pub use capture::*;
pub use state::*;
