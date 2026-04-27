# Contributing to Victauri

## Getting Started

```bash
git clone https://github.com/runyourempire/victauri.git
cd victauri
cargo build --workspace
cargo test --workspace
```

Requires Rust 1.85+ (edition 2024).

## Running Tests

```bash
cargo test --workspace          # All tests (205)
cargo test -p victauri-core     # Core crate only
cargo test -p victauri-plugin   # Plugin crate only
cargo bench -p victauri-core    # Criterion benchmarks (13)
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

All three checks (test, clippy, fmt) must pass. CI runs them on Linux, Windows, and macOS.

## Code Style

- `cargo fmt --all` before committing
- `cargo clippy -- -D warnings` must pass
- No `unwrap()` on mutexes or RwLocks — use `unwrap_or_else(|e| e.into_inner())` for poisoning recovery
- No `unreachable!()` in match arms that could be reached by malformed input

## Structure

| Crate | Purpose |
|---|---|
| `victauri-core` | Types, events, verification, registry — no Tauri dependency |
| `victauri-macros` | `#[inspectable]` proc macro |
| `victauri-plugin` | Tauri plugin: MCP server, JS bridge, auth, privacy |
| `victauri-watchdog` | Health-check sidecar |

## Adding MCP Tools

1. Add the handler function in `crates/victauri-plugin/src/mcp.rs`
2. Register it in the tool list and router
3. Add tests in the appropriate test module
4. Update the tool count in README and CHANGELOG

## Demo App

The demo app at `examples/demo-app/` exercises all command patterns. After changing the plugin API, verify the demo still builds:

```bash
cargo build -p demo-app
```

## Publishing to crates.io

Maintainers only. Publication order matters due to inter-crate dependencies:

```bash
cargo publish -p victauri-core
cargo publish -p victauri-macros
cargo publish -p victauri-plugin
cargo publish -p victauri-watchdog
```

## License

By contributing, you agree that your contributions will be licensed under Apache-2.0.
