# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **BREAKING:** `SemanticAssertion.condition` is now `AssertionCondition` enum instead of `String` — invalid conditions are caught at deserialization, not deep in evaluation logic
- **BREAKING:** All 9 compound tool `action` parameters are now typed enums (`InteractAction`, `InputAction`, `WindowAction`, `StorageAction`, `NavigateAction`, `RecordingAction`, `InspectAction`, `CssAction`, `LogsAction`) — invalid actions are rejected at JSON deserialization with clear variant listings
- **BREAKING:** `WaitForParams.condition` is now `WaitCondition` enum instead of `String`
- **BREAKING:** `StorageParams.storage_type` is now `Option<StorageType>` enum, `NavigateParams.dialog_type`/`dialog_action` are now `Option<DialogType>`/`Option<DialogAction>` enums, `WindowParams.manage_action` is now `Option<ManageAction>` enum
- **BREAKING:** `SnapshotParams.format` is now `Option<SnapshotFormat>` enum instead of `Option<String>`
- `events_between_checkpoints` returns `Result` with specific error variants instead of `Option`
- Extracted `json_result` helper in MCP handler, eliminating 14 repeated serialization blocks
- All match-based extractions replaced with `let...else` (32 sites)
- All `map().unwrap_or()` chains replaced with `map_or()` (10 sites)
- All redundant closures replaced with method references (53 sites)
- `Default::default()` replaced with explicit type calls per `default_trait_access`
- Scoped `use` imports moved before statements per `items_after_statements`

### Added

- `AssertionCondition` enum with `FromStr`, `Display`, `Serialize`/`Deserialize`, and feature-gated `JsonSchema` (`schema` feature on victauri-core)
- 16 typed enums replacing string parameters across plugin and core crates
- `Serialize` and `Display` implemented on all 13 action enums for symmetric serde and ergonomic formatting
- `#[must_use]` on all 26 value-returning public functions (constructors, getters, builders, analysis)
- `# Errors` documentation on all public `Result`-returning functions
- `# Panics` documentation on all functions containing panicking assertions
- Backticks on all code items in doc comments (73 sites)
- Crate-level documentation on all crates, binaries, and build scripts
- `#[warn(missing_docs)]` enforced in victauri-core, victauri-plugin, and victauri-test
- `FLOAT_EPSILON` named constant for floating-point severity classification
- **20 clippy lints enforced at deny level** (17 pedantic + 3 nursery) in workspace config: `redundant_closure_for_method_calls`, `missing_errors_doc`, `must_use_candidate`, `return_self_not_must_use`, `manual_let_else`, `map_unwrap_or`, `doc_markdown`, `uninlined_format_args`, `single_match_else`, `default_trait_access`, `cast_lossless`, `needless_raw_string_hashes`, `if_not_else`, `missing_panics_doc`, `items_after_statements`, `clippy::all`, `derive_partial_eq_without_eq`, `use_self`, `redundant_pub_crate`
- Centralized lint configuration in `[workspace.lints]` (Cargo edition 2024)

### Removed

- 39 dead param structs from pre-compound-tool era (replaced by compound params)
- `tool_not_found` helper (no longer needed with typed action enums)
- `introspection_params.rs` and `recording_params.rs` modules (all structs superseded)
- Dead `RecoveryHint` variants (`RetryLater`, `TryAlternative`) and `ref_not_found` helper
- Per-crate `#![deny(unsafe_code)]` attributes (now in workspace lints)

### Fixed

- 6 ghost tool names in `VictauriClient` test client (`fill`, `type_text`, `get_window_state`, `get_ipc_log`, `start_recording`, `wait_for` parameter name)
- Benchmark code silently discarding `Result` from `EventRecorder::start`
- README code examples using non-existent API methods

## [0.1.0] - 2026-04-28

Initial public release.

### Added

- Workspace with 5 crates: victauri-core, victauri-macros, victauri-plugin, victauri-test, victauri-watchdog
- 23 MCP tools (9 compound + 14 standalone) covering WebView, Windows, Backend, Storage, Navigation, Verification, Time-Travel, CSS/Style, Accessibility, Performance
- 3 MCP resources: victauri://ipc-log, victauri://windows, victauri://state
- `#[inspectable]` proc macro for Tauri command instrumentation with JSON schema generation
- JS bridge v0.3.0 with DOM walking, ref handles, console capture, mutation observer, network interception, navigation tracking, dialog capture, waitFor polling
- IPC interception via `fetch` monkey-patching (Tauri 2.0 `ipc.localhost` protocol)
- Privacy layer: command allowlists/blocklists, tool disabling, regex-based output redaction, strict mode
- Rate limiting (100 req/sec default, token bucket with AtomicU64)
- `VictauriBuilder` for port/auth/capacity configuration
- `VICTAURI_PORT` and `VICTAURI_AUTH_TOKEN` environment variable support
- Release-safe: zero overhead in release builds via `#[cfg(debug_assertions)]`
- Cross-platform CI (Linux, Windows, macOS) with clippy, tests, docs, MSRV, security audit, dependency checks, semver checks
- 415+ tests (121 core + 4 macro + 123 plugin unit + 118 integration + 38 adversarial + 5 watchdog + 6 doctests)
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
