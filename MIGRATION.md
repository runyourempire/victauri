# Migration Guide

## v0.2.x → v0.3.0

### New Features

- **Browser extension ecosystem** — `victauri-browser` crate with native messaging host for Chrome/Edge/Brave/Arc/Firefox. MCP inspection for any website, not just Tauri apps.
- **Firefox extension** — full MV3 port using `browser.*` namespace in `extensions/firefox/`
- **npm package** — `@anthropic/victauri-browser` with postinstall binary download from GitHub releases
- **163 JavaScript tests** — vitest + jsdom test suite for the Chrome extension JS bridge
- **52 E2E Rust tests** — full pipeline integration tests for victauri-browser
- **mdbook documentation site** — 10-page docs site in `docs/`
- **Release workflow** — GitHub Actions pipeline: test gate → cross-platform matrix builds → Chrome extension zip → sequential crates.io publish → GitHub Release with all artifacts
- **CI for JS tests** — Chrome extension vitest job added to CI workflow

### No Breaking Changes

This is a feature-only release. All existing APIs are unchanged.

---

## v0.1.x → v0.2.0

### Breaking Changes

#### `file:` URL navigation blocked by default

The `navigate` tool's `go_to` action now rejects `file://` URLs by default to prevent local filesystem access. If your tests navigate to local HTML files, opt in explicitly:

```rust
VictauriBuilder::new()
    .allow_file_navigation()
    .build()
```

#### `DomElement.attributes` and `DomSnapshot.ref_map` use `BTreeMap`

These fields changed from `HashMap<String, String>` to `BTreeMap<String, String>` for deterministic serialization order. Update your imports:

```rust
// Before
use std::collections::HashMap;
// After
use std::collections::BTreeMap;
```

The API is identical — `BTreeMap` implements the same traits. If you were constructing `DomElement` or `DomSnapshot` in test code, change `HashMap::new()` to `BTreeMap::new()`.

#### Privacy profiles replace boolean strict mode

`strict_privacy_mode()` still works but now maps to `PrivacyProfile::Observe`. The new `privacy_profile()` builder method gives finer control:

```rust
// Before (v0.1.x) — boolean, all-or-nothing
VictauriBuilder::new().strict_privacy_mode()

// After (v0.2.0) — three tiers
use victauri_plugin::PrivacyProfile;

// Read-only: snapshots, logs, registry — no clicks, no input, no eval
VictauriBuilder::new().privacy_profile(PrivacyProfile::Observe)

// Testing: observe + interactions + input + storage writes + recording
VictauriBuilder::new().privacy_profile(PrivacyProfile::Test)

// Everything (default, same as v0.1.x)
VictauriBuilder::new().privacy_profile(PrivacyProfile::FullControl)
```

`strict_privacy_mode()` is equivalent to `privacy_profile(PrivacyProfile::Observe)`.

**New tool gates:** `interact` and `recording` are now privacy-gated (blocked in `Observe`, allowed in `Test`). `invoke_command` in `Test` profile requires the command to be on the allowlist.

`disable_tools()` still works as an override layer on top of any profile.

#### `TestError::Connection` is now a struct variant

`TestError::Connection(String)` changed to `TestError::Connection { host, port, reason }` for richer diagnostics. Update pattern matches:

```rust
// Before
Err(TestError::Connection(msg)) => eprintln!("{msg}"),

// After
Err(TestError::Connection { host, port, reason }) => {
    eprintln!("failed to reach {host}:{port}: {reason}");
}
```

### New Features

- **`VictauriBuilder::allow_file_navigation()`** — opt in to `file://` URL navigation
- **Mutex poisoning recovery** — all lock acquisitions now use `acquire_lock`/`acquire_read`/`acquire_write` helpers with tracing diagnostics instead of panicking
- **Origin guard hardening** — URL-parsing based validation replaces vulnerable `starts_with` pattern
- **Centralized output redaction** — redaction applies uniformly at the `call_tool` boundary, not per-tool
- **TestApp stderr capture** — connection timeout errors now include the last 10 lines of the app's stderr
- **CLI empty registry warning** — `victauri coverage` exits with code 1 when no commands are registered (use `--allow-empty-registry` to override)
- **Release workflow hardening** — dry-run gate before real publishes, no `continue-on-error`

### Deprecations

- **`ipc_checkpoint()`** → renamed to **`create_ipc_checkpoint()`** (verb-first naming). The old name still works with a deprecation warning and forwards to the new method.
