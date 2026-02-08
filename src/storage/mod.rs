//! XDG-compliant filesystem persistence.
//!
//! Manages session files (NDJSON), alias storage, and directory paths
//! following the XDG Base Directory specification.

pub mod alias_store;
pub mod paths;
pub mod session_store;

pub use alias_store::*;
pub use paths::*;
pub use session_store::*;
