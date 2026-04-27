# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-28

Initial public release.

### Added

- Workspace with 5 crates: victauri-core, victauri-macros, victauri-plugin, victauri-test, victauri-watchdog
- 55 MCP tools across 16 categories (WebView, Windows, Backend, Network, Storage, Navigation, Dialogs, Verification, Streaming, Intent, Wait, Time-Travel, CSS/Style, Visual Debug, Accessibility, Performance)
- 3 MCP resources: victauri://ipc-log, victauri://windows, victauri://state
- `#[inspectable]` proc macro for Tauri command instrumentation with JSON schema generation
- JS bridge v0.2.0 with DOM walking, ref handles, console capture, mutation observer, network interception, navigation tracking, dialog capture, waitFor polling
- IPC interception via `fetch` monkey-patching (Tauri 2.0 `ipc.localhost` protocol)
- Privacy layer: command allowlists/blocklists, tool disabling, regex-based output redaction, strict mode
- Rate limiting (100 req/sec default, token bucket with AtomicU64)
- `VictauriBuilder` for port/auth/capacity configuration
- `VICTAURI_PORT` and `VICTAURI_AUTH_TOKEN` environment variable support
- Release-safe: zero overhead in release builds via `#[cfg(debug_assertions)]`
- Cross-platform CI (Linux, Windows, macOS) with clippy, tests, docs, MSRV, security audit, dependency checks, semver checks
- 287 tests (115 core + 4 macro + 105 plugin unit + 52 plugin integration + 5 watchdog + 6 doctests)
- 13 Criterion benchmarks across 5 groups
- Windows screenshot via `PrintWindow` + custom PNG encoder (no external dependencies)
- macOS screenshot via `CGWindowListCreateImage` + alpha un-premultiply
- Linux screenshot via X11 `GetImage` (x11rb) with Wayland fallback via `grim`
- OS-level process memory stats (Windows `GetProcessMemoryInfo`, Linux `/proc/self/statm`)
- victauri-watchdog crash-recovery sidecar with configurable recovery commands
- victauri-test crate: typed MCP HTTP client (`VictauriClient`) with session management and assertion helpers (`assert_json_eq`, `assert_json_truthy`, `assert_no_a11y_violations`, `assert_performance_budget`, `assert_ipc_healthy`, `assert_state_matches`)
- Demo app example with 12 instrumented commands (greet, counter CRUD, todo CRUD, settings, state dump)
- Shadow DOM traversal in DOM snapshots
- Cross-boundary state verification, ghost command detection, IPC integrity checking
- Event recording with checkpoints, export/import, and replay sequences
- CSS inspection, visual debug overlays, accessibility auditing, performance profiling
- Tag-triggered release workflow for crates.io publishing

### Security

- JS injection prevention: all manual string escaping replaced with `serde_json::to_string()` via `js_string()` helper
- URL validation via scheme allowlist (http/https/file only) using the `url` crate parser
- DNS rebinding protection for localhost server
- Security headers (X-Frame-Options, X-Content-Type-Options, Cache-Control)
- Screenshot error handling: `GetDIBits()` return value checked on Windows

[Unreleased]: https://github.com/runyourempire/victauri/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/runyourempire/victauri/releases/tag/v0.1.0
