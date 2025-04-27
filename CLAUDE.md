# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands
- Build: `cargo build`
- Run tests: `cargo test`
- Run specific test: `cargo test test_name`
- Install uci daemon: `cargo install --path uci`
- Install cli: `cargo install --path uci_cli`
- Using Nix: `nix build .#ucid` or `nix build .#uci`
- Format code: `cargo fmt`
- Lint code: `cargo clippy`

## Code Style Guidelines
- Follow standard Rust naming conventions: snake_case for functions/variables, CamelCase for types
- Use anyhow for error handling and propagation
- Define custom errors with thiserror when appropriate
- Use tokio for async operations with #[tokio::test] for async tests
- Organize code hierarchically in modules
- Ensure proper error handling with Result<T, E> and propagate errors with `?`
- Use meaningful docstrings for public APIs
- Keep functions small and focused on single responsibilities
