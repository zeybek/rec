# AGENTS.md - Coding Agent Guidelines for rec

## Project Overview

`rec` is a CLI terminal recorder written in Rust. It records shell commands, replays them, and exports to various formats (bash scripts, Makefiles, Markdown, etc.).

- **Binary:** `rec` (CLI entry point)
- **Library:** `rec` (public API for integration tests)
- **Rust Edition:** 2024, MSRV 1.85

## Build Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Lint
cargo clippy --all-targets     # Clippy with pedantic (configured in Cargo.toml)
cargo fmt --check              # Check formatting

# Test - all tests
cargo test                     # Run all 516+ tests (unit + integration)

# Test - single test by name
cargo test test_start_creates_session              # Run one test by name
cargo test start_                                  # Run tests matching pattern

# Test - single test file
cargo test --test start_test                       # Run tests/start_test.rs
cargo test --test common                           # Run tests/common/mod.rs

# Test - lib only (unit tests)
cargo test --lib                                   # 387 unit tests

# Test - integration only
cargo test --test '*'                              # 129 integration tests

# Test - with output
cargo test -- --nocapture                          # Show println! output
cargo test test_name -- --nocapture                # Single test with output
```

## Project Structure

```
src/
  main.rs           # Thin dispatcher (~180 lines), parses CLI, delegates to handlers
  lib.rs            # Library root, re-exports all public modules
  handlers/         # Command handlers (one file per command group)
    mod.rs          # HandlerContext struct
    common.rs       # Shared utilities (unix_timestamp, handle_error, etc.)
    start.rs, stop.rs, ...
  cli/              # CLI parsing (clap)
  models/           # Data models (Session, Command, Config)
  storage/          # Filesystem operations (Paths, SessionStore, AliasStore)
  recording/        # Recording lifecycle (state, capture)
  replay/           # Command replay engine
  export/           # Export formats (bash, makefile, markdown, etc.)
  import/           # Import from shell history
  error.rs          # RecError enum with exit codes
tests/
  common/mod.rs     # TestEnv, output factories (shared infrastructure)
  *_test.rs         # Integration tests (one file per command)
```

## Code Style

### Imports

Order imports in three groups, separated by blank lines:
1. Standard library (`std::`)
2. External crates (`clap`, `serde`, etc.)
3. Internal modules (`crate::`, `super::`, `rec::`)

```rust
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::error::RecError;
use crate::models::Session;
```

### Formatting

- 4 spaces for indentation (no tabs)
- Max line width: default (100)
- Use `cargo fmt` before committing
- Trailing newline required

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Modules | snake_case | `session_store.rs` |
| Types/Structs | PascalCase | `SessionHeader`, `RecError` |
| Functions | snake_case | `handle_start`, `load_session` |
| Constants | SCREAMING_SNAKE | `MAX_RETRIES` |
| Handler functions | `handle_<command>` | `handle_start`, `handle_stop` |

### Error Handling

Use `RecError` for all domain errors. Exit codes follow semantic convention:
- `0` = Success
- `1` = User error (bad input, not found, invalid state)
- `2` = System error (I/O, permissions, corrupt data)
- `130` = Interrupted (Ctrl+C)

```rust
// In handlers, return ExitCode directly
pub fn handle_foo(ctx: &HandlerContext) -> ExitCode {
    match do_something() {
        Ok(result) => ExitCode::SUCCESS,
        Err(e) => {
            ctx.output.error("Error", &e.to_string(), None, None);
            ExitCode::from(e.exit_code())
        }
    }
}

// Use handle_result for common pattern
use super::common::handle_result;
handle_result(some_operation(), &ctx.output, &ctx.paths)
```

### Handler Pattern

All command handlers:
1. Take `&HandlerContext` as first argument (paths, config, output, flags)
2. Return `ExitCode`
3. Use `Option<&T>` instead of `&Option<T>` for optional args
4. Live in `src/handlers/<command>.rs`

### Clippy

Pedantic lints enabled. See `Cargo.toml` `[lints.clippy]` for allowed exceptions.

## Testing Guidelines

### Test Environment

**CRITICAL:** Never call these in tests (they touch real XDG paths):
- `get_paths()` - poisons OnceLock singleton
- `Output::new()` - calls load_config()
- `Paths::new()` - uses real directories

Instead, use `TestEnv` for isolated filesystem:

```rust
use crate::common::{TestEnv, quiet_output};

#[test]
fn test_something() {
    let env = TestEnv::new();  // Creates temp directory
    let session = env.create_and_save_session("my-session");
    
    // Use env.paths, env.store, env.alias_store
    // Temp dir cleaned up when env drops
}
```

### Output Factories

Use factory functions (never `Output::new()`): `quiet_output()`, `verbose_output()`, `json_output()`, `normal_output()`

### Test File Naming

- One file per command: `tests/<command>_test.rs`
- Test function prefix: `test_<command>_<behavior>`

## Common Patterns

### Session Resolution

Sessions are identified by name, UUID, or alias:

```rust
use rec::session::resolve_session;

match resolve_session(&identifier, &store, Some(&alias_store)) {
    Ok(session) => { /* use session */ }
    Err(resolve_err) => {
        // resolve_err.suggestions contains fuzzy matches
        handle_resolve_error(&resolve_err, &output)
    }
}
```

### Recording State

```rust
use rec::recording::RecordingState;

let state = RecordingState::new(&paths.state_dir);
if state.is_recording() {
    let current = state.current()?;
    // ...
}
```
