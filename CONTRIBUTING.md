# Contributing to rec

Thank you for your interest in contributing to `rec`! This document provides guidelines and information for contributors.

## Development Setup

### Prerequisites

- Rust 1.85+ (edition 2024)
- Git

### Getting Started

```bash
git clone https://github.com/zeybek/rec.git
cd rec
cargo build
cargo test
```

### Running Checks

```bash
# Run tests
cargo test

# Run clippy lints
cargo clippy --all-targets -- -D warnings

# Check formatting
cargo fmt --check

# Run cargo-deny (license and advisory checks)
cargo deny check
```

## Making Changes

### Branch Naming

- `feat/description` — New features
- `fix/description` — Bug fixes
- `docs/description` — Documentation changes
- `refactor/description` — Code refactoring

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new export format
fix: handle empty sessions in replay
docs: update README with new flags
test: add integration tests for import
refactor: extract config validation logic
```

### Pull Request Process

1. Fork the repository and create your branch from `main`
2. Add tests for any new functionality
3. Ensure all tests pass: `cargo test`
4. Ensure no clippy warnings: `cargo clippy --all-targets -- -D warnings`
5. Ensure code is formatted: `cargo fmt`
6. Update documentation if needed
7. Submit a pull request

### Code Style

- Follow standard Rust conventions (enforced by `rustfmt` and `clippy`)
- Use `thiserror` for library error types
- Use library-level integration tests (preferred over `assert_cmd` subprocess tests)
- Use `TestEnv` struct for test isolation (see `tests/common/mod.rs`)

### Testing

- **Unit tests**: In-module `#[cfg(test)]` blocks
- **Integration tests**: In `tests/` directory using `TestEnv`
- **Binary tests**: Use `assert_cmd` only for CLI-level smoke tests

### Architecture

- `src/lib.rs` — Library root (public API)
- `src/main.rs` — Binary entry point
- `src/cli/` — CLI argument parsing (clap)
- `src/config/` — Configuration management
- `src/recording/` — Session recording lifecycle
- `src/session/` — Session data operations
- `src/storage/` — Filesystem persistence
- `src/replay/` — Command replay engine
- `src/export/` — Multi-format export
- `src/import/` — Shell history import
- `src/doctor/` — Diagnostic checks

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Check existing issues before opening a new one
- Use the issue templates when available

## License

By contributing, you agree that your contributions will be licensed under the same dual license as the project: MIT OR Apache-2.0.
