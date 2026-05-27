# Migration Guide

## v0.5.5 → v0.5.6

### Breaking Change: Auth Enabled by Default

The MCP server now **generates a Bearer token automatically** on startup and enforces authentication on all endpoints except `/health`. Previously auth was opt-in.

**If you use `VictauriClient::discover()`** — no change needed. The client reads the token from the discovery directory automatically.

**If you use a custom HTTP client** — read the token from `<temp>/victauri/<pid>/token` and send it as `Authorization: Bearer <token>`.

**If you want the old behavior (no auth):**

```rust
VictauriBuilder::new()
    .auth_disabled()   // Explicitly opt out of auth
    .build()
```

**If you set `VICTAURI_AUTH_TOKEN` env var** — that token is used instead of auto-generation. Behavior unchanged.

### Behavior Changes

**DNS rebinding guard** — All requests must have a `Host` header matching `localhost`, `127.0.0.1`, `[::1]`, or `localhost:<port>`. Requests from DNS-rebound hostnames (e.g. `evil.com` resolving to `127.0.0.1`) receive 403. This affects both the plugin MCP server and the browser native host.

**Security response headers** — All responses now include `X-Content-Type-Options: nosniff`, `Cache-Control: no-store`, `X-Frame-Options: DENY`, and `Content-Security-Policy: default-src 'none'`. If your client parses response headers, these are new.

**Eval output limit** — `eval_js` results exceeding 5 MB return an error instead of the result. If you eval expressions that produce very large strings (e.g. `JSON.stringify(document.body)`), you may need to trim output in your JS expression.

**Rate limiter 429 responses** now include a `Retry-After: 1` header. Clients should respect this before retrying.

**`get_diagnostics` env vars** — The environment variable allowlist was trimmed from ~30 to 16 prefixes. If you relied on seeing `PATH`, `RUST*`, `CARGO*`, `APPDATA`, or other system variables in diagnostics output, they are no longer exposed.

**SQL hardening** — `query_db` now strips SQL comments (`--` and `/* */`) before the read-only check and rejects stacked queries (statements with `;`). Legitimate multi-statement queries are not supported.

### Version Bump

```toml
victauri-plugin = "0.5.6"
victauri-test = "0.5.6"
```

---

## v0.5.4 → v0.5.5

### New Public API

**`AppEvent::Console` variant** — Console log events from the bridge are now typed as `AppEvent::Console { level, message, timestamp }` instead of `AppEvent::StateChange { key: "console.warn", caused_by: Some(message) }`. If you match on `AppEvent` variants, this is a new arm. Since `AppEvent` is `#[non_exhaustive]`, existing code with wildcard matches compiles unchanged.

**`AppEvent::is_internal()`** — Returns `true` for Victauri's own infrastructure events (`plugin:victauri|*` IPC). Use this instead of manual string-matching when filtering event logs.

### Behavior Changes

**Bridge ready signal** — The JS bridge now sends a `__victauri_bridge_ready__` callback when it initializes. The eval pipeline waits up to 5 seconds for this signal before the first eval. This eliminates the race condition on page load and removes the 2-second first-call latency from the probe mechanism.

**Discovery session tokens** — The server now always writes a session token to the discovery directory (`<temp>/victauri/<pid>/token`), even when auth is not enabled. `VictauriClient::discover()` reads this token and includes it as a Bearer header. When auth is off, the header is harmlessly ignored. This prepares the path for zero-config auth in a future release.

### Version Bump

```toml
victauri-plugin = "0.5.5"
victauri-test = "0.5.5"
```

---

## v0.5.2 → v0.5.3

### Behavior Changes

**`find_elements` with invalid CSS selectors now returns an error** instead of silently returning `[]`. If your code catches empty results as "not found", you may need to handle the error case:

```json
// Before: {"selector": "###invalid"} → []
// After:  {"selector": "###invalid"} → isError: "invalid CSS selector: ###invalid — ..."
```

**`eval_js` errors now set MCP `isError`** flag. Previously, `throw new Error("x")` returned `{"__error":"x"}` as a successful text result. Now it returns a proper MCP error with `isError: true` and message `"JavaScript error: x"`.

**`eval_js` with `undefined` and `null`** — Previously returned `{}` for both. Now returns `"undefined"` (string) or `null` respectively.

### Fixed

- **Network log pollution:** Victauri's own `plugin:victauri|*` IPC traffic no longer fills the 1000-entry `networkLog` buffer, preserving real app IPC evidence for `get_ipc_log`
- **Hidden window timeout:** Eval targeting hidden/unresponsive windows now fails in 2 seconds with a diagnostic message instead of timing out after 30s
- **Replay after stop:** `recording.replay` and `recording.export` now work after `recording.stop` — session data is persisted
- **Checkpoint label alias:** `recording.checkpoint` now accepts `label` as an alias for `checkpoint_label`
- **Explain noise:** `explain.summary`, `explain.last_action`, and `explain.diff` filter out Victauri's internal IPC and state change events

### Version Bump

```toml
victauri-plugin = "0.5.3"
victauri-test = "0.5.3"
```

---

## v0.5.0 → v0.5.2

### Introspect Action Renames

Three actions renamed for clarity:

| Old Name | New Name | Reason |
|----------|----------|--------|
| `managed_state` | `plugin_state` | Distinguishes Victauri's own state from app state |
| `tasks` | `plugin_tasks` | Distinguishes Victauri's async tasks from app tasks |
| `fs_scope` | *(removed)* | Redundant with `app_info` tool |

### Enhanced Actions

- **`capabilities`** now returns structured security config (CSP, `freeze_prototype`), configured plugins, window definitions, and privacy profile
- **`processes`** now enumerates child processes (sidecars, background workers) with PID, name, and memory usage
- **`event_bus`** events are now captured automatically — apps no longer need to manually push events

### New Builder Method

```rust
VictauriBuilder::new()
    .listen_events(&["notification-added", "settings-changed"])
    .build()
```

Window lifecycle events (resize, move, focus, close, theme, drag-drop) are captured automatically without `listen_events`.

---

## v0.4.x → v0.5.0

### New Tools

Three new compound tools added — no breaking changes, purely additive.

**`introspect`** — 13 actions for deep backend introspection:

```json
{"action": "command_timings", "slow_threshold_ms": 100}
{"action": "coverage"}
{"action": "contract_record", "command": "get_settings"}
{"action": "contract_check"}
{"action": "startup_timing"}
{"action": "capabilities"}
{"action": "db_health"}
{"action": "plugin_state"}
{"action": "processes"}
{"action": "plugin_tasks"}
{"action": "event_bus"}
```

**`fault`** — IPC chaos engineering (delay, error, drop, corrupt):

```json
{"action": "inject", "command": "get_settings", "fault_type": "delay", "delay_ms": 2000}
{"action": "inject", "command": "save_data", "fault_type": "error", "error_message": "disk full"}
{"action": "list"}
{"action": "clear_all"}
```

**`explain`** — Natural-language event narration:

```json
{"action": "summary", "seconds": 30}
{"action": "last_action"}
{"action": "diff", "seconds": 15}
```

**`recording.replay`** — New action on existing `recording` tool. Re-executes recorded IPC commands and checks for response shape drift:

```json
{"action": "replay"}
```

### Version Bump

Update your dependency version:

```toml
victauri-plugin = "0.5"
victauri-test = "0.5"
```

---

## v0.3.x → v0.4.0

### Breaking Change: Auth Disabled by Default

Authentication is now **disabled by default**. Previously, the plugin auto-generated a UUID Bearer token on startup. This caused silent MCP connection failures when the token wasn't configured in `.mcp.json`.

**If you were relying on auto-generated auth**, opt in explicitly:

```rust
VictauriBuilder::new()
    .auth_enabled()  // auto-generates UUID token, printed to console
    .build()
```

**If you were calling `.auth_disabled()`**, you can remove it — auth is already off by default. The method still exists as a no-op for backwards compatibility.

**If you were using `VICTAURI_AUTH_TOKEN` env var or `.auth_token("...")`**, no change needed — those still enable auth.

### New: `register_command_names` Builder API

Lightweight alternative to `#[inspectable]` proc macros:

```rust
VictauriBuilder::new()
    .register_command_names(&["get_settings", "save_settings", "search"])
    .build()
```

### New CLI Commands

- `victauri invoke <command>` — call any Tauri IPC command from terminal
- `victauri doctor` — full setup diagnosis
- `victauri init` now scaffolds CLAUDE.md with agent instructions

---

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
