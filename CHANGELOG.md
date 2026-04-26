# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- All 30+ `.lock().unwrap()` / `.read().unwrap()` calls replaced with mutex poisoning recovery
- `unreachable!()` panic in verification array comparison (now silent no-op)
- Float comparison silently coercing unparseable numbers to `0.0` in severity classification
- Unknown assertion conditions silently returning `false` (now returns explicit error message)
- NaN sort poisoning in registry `resolve()` (`partial_cmp` -> `total_cmp`)
- Case-insensitive Bearer token matching in auth middleware (RFC 7235)
- Eval timeout error message now uses `EVAL_TIMEOUT` constant instead of hardcoded "10s"

### Added

- Shadow DOM traversal in `walkDom()` — Web Components are now visible in DOM snapshots
- Shadow DOM text search in `waitFor` conditions
- `type()` uses native value setter via `Object.getOwnPropertyDescriptor` + `InputEvent` dispatch (React/Vue compatible)
- `PartialEq` derives on all core types (`VerificationResult`, `Divergence`, `DivergenceSeverity`, `WindowState`, `DomSnapshot`, `DomElement`, `ElementBounds`)
- `Default` impl for `EventRecorder` (50K event capacity)
- Checkpoint capacity limit (1000, was unbounded)
- 26 adversarial tests: mutex poisoning recovery, concurrent access, ring buffer edge cases, verification edge cases, ghost command edge cases, assertion edge cases
- Criterion benchmark suite: 13 benchmarks across 5 groups (event log, registry, verification, recording, ghost commands)
- Demo app frontend upgraded: full Todo list UI, Settings panel, Counter decrement/reset, debug state inspector (exercises all 12 backend commands)
- Honest README with limitations section, security model, privacy controls, roadmap

### Changed

- README rewritten: removed unverified performance claims, added "What It Doesn't Do (Yet)" section
- Checkpoints use `VecDeque` with bounded capacity instead of unbounded `Vec`
- Test count: 110 -> 136

## [0.1.0] - 2026-04-26

### Added

- Workspace with 4 crates: victauri-core, victauri-macros, victauri-plugin, victauri-watchdog
- 55 MCP tools across 16 categories (WebView, Windows, Backend, Network, Storage, Navigation, Dialogs, Verification, Streaming, Intent, Wait, Time-Travel, CSS/Style, Visual Debug, Accessibility, Performance)
- 3 MCP resources: victauri://ipc-log, victauri://windows, victauri://state
- `#[inspectable]` proc macro for Tauri command instrumentation with JSON schema generation
- JS bridge with DOM walking, ref handles, console capture, mutation observer, network interception
- IPC interception via `fetch` monkey-patching (Tauri 2.0 `ipc.localhost` protocol)
- Privacy layer: command allowlists/blocklists, tool disabling, regex-based output redaction, strict mode
- Rate limiting (100 req/sec default, token bucket with AtomicU64)
- `VictauriBuilder` for port/auth/capacity configuration
- `VICTAURI_PORT` and `VICTAURI_AUTH_TOKEN` environment variable support
- Release-safe: zero overhead in release builds via `#[cfg(debug_assertions)]`
- Cross-platform CI (Linux, Windows, macOS)
- 110 tests (44 core + 4 macro + 18 plugin unit + 44 plugin integration)
- Windows screenshot via `PrintWindow` + custom PNG encoder (no external dependencies)
- macOS screenshot via `CGWindowListCreateImage` + alpha un-premultiply
- OS-level process memory stats (Windows `GetProcessMemoryInfo`, Linux `/proc/self/statm`)
- victauri-watchdog crash-recovery sidecar
- Demo app example with 12 instrumented commands

[Unreleased]: https://github.com/runyourempire/victauri/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/runyourempire/victauri/releases/tag/v0.1.0
