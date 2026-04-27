# Victauri: Remaining Work Plan

**Goal:** Every item needed for serious developers to rely on Victauri as a production-grade open-source crate.

**Current state:** 209 tests, 59 tools, 3 resources, security hardening (DNS rebinding, origin guard, rate limiting), typed errors, full CI pipeline, crates.io metadata. What follows is everything that remains.

---

## Phase 1: Reliability & Safety (Critical)

These are bugs or safety gaps that would cause a developer to lose trust immediately.

### 1.1 Graceful MCP Server Shutdown
**Why:** When a Tauri app exits, the axum server is orphaned — no clean socket close, no in-flight request draining, no pending eval cleanup. Agents see connection reset errors.

**How:**
- Add `shutdown_tx: tokio::sync::watch::Sender<()>` to `VictauriState`
- Pass shutdown future to `axum::serve().with_graceful_shutdown()`
- Wire `RunEvent::Exit` in the Tauri plugin `on_event` handler to `shutdown_tx.send(())`
- Store the server `JoinHandle` in state as a hard-abort fallback
- Clean up all `pending_evals` callbacks on shutdown (send error to waiting receivers)

**Files:** `crates/victauri-plugin/src/lib.rs`, `crates/victauri-plugin/src/mcp.rs`
**Tests:** Integration test that starts server, sends shutdown signal, verifies clean exit.

### 1.2 Builder Validation
**Why:** `VictauriBuilder::port(0)` or `event_capacity(0)` silently creates broken config.

**How:**
- Change `build()` to return `Result<TauriPlugin, BuilderError>`
- Validate: port in 1024..=65535 (or 0 for OS-assigned), capacities > 0, timeout >= 1s
- Add `BuilderError` variants to `PluginError` (or a dedicated enum)
- Update `init()` to call `VictauriBuilder::default().build().expect(...)` (panic is acceptable for the convenience function since misconfiguration is a programmer error)

**Files:** `crates/victauri-plugin/src/lib.rs`, `crates/victauri-plugin/src/error.rs`
**Tests:** Unit tests for each validation rule — valid, boundary, and invalid inputs.

### 1.3 Bridge Version Handshake
**Why:** If the Rust side upgrades but the cached JS bridge is stale (Vite HMR, browser cache), tools silently break.

**How:**
- Add `BRIDGE_VERSION` constant in Rust (from `Cargo.toml` version)
- Inject it as `window.__VICTAURI__.expectedVersion` in the init script
- Set `window.__VICTAURI__.version` in the JS bridge (already `'0.2.0'`)
- `eval_with_return` checks `__VICTAURI__.version === __VICTAURI__.expectedVersion` before the first call and logs a warning on mismatch
- `get_plugin_info` tool reports both versions so agents can detect drift

**Files:** `crates/victauri-plugin/src/js_bridge.rs`, `crates/victauri-plugin/src/mcp.rs`, `crates/victauri-plugin/src/lib.rs`
**Tests:** Unit test with mock bridge returning mismatched version.

### 1.4 JS Resource Cleanup on Unload
**Why:** MutationObserver, fetch interceptor, and event listeners accumulate across page navigations in SPA dev mode.

**How:**
- Add `window.addEventListener('beforeunload', cleanup)` in the JS bridge
- `cleanup()` disconnects MutationObserver, restores original `fetch`/`XMLHttpRequest`, removes event listeners
- Add `__VICTAURI__.destroy()` public method for manual cleanup
- Re-initialize on next `js_init_script()` execution (already happens on navigation)

**Files:** `crates/victauri-plugin/src/js_bridge.rs`
**Tests:** Manual test in demo app — navigate away and back, verify no duplicate observers.

---

## Phase 2: Documentation & Developer Experience (High Priority)

A serious developer evaluates a crate by reading docs.rs before writing code.

### 2.1 Complete Doc Comments on All Public API
**Why:** 15+ public types/functions lack doc comments. `cargo doc` produces pages with no descriptions.

**Items needing docs:**
- `victauri-core`: `RefHandle`, `MemoryDelta`, `VerificationResult`, `Divergence`, `DivergenceSeverity`, `WindowState`, `DomSnapshot`, `DomElement`, `ElementBounds` (types.rs, snapshot.rs)
- `victauri-plugin`: `generate_token`, `AuthState`, `require_auth`, `default_rate_limiter` (auth.rs), `strict_privacy_config` (privacy.rs), `Redactor` methods (redaction.rs)

**Standard:** Every `pub` item gets a `///` one-liner. Structs with non-obvious fields get field-level docs. Key types get `# Examples` doctests.

**Files:** All `src/*.rs` files in core and plugin crates.
**Validation:** `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS="-Dwarnings"` — zero warnings.

### 2.2 Doctests on Key Public APIs
**Why:** Doctests are the gold standard for Rust crates — they're both documentation and tests that can't go stale.

**Priority items (5-8 examples):**
- `VictauriBuilder` — basic setup with `.port().build()`
- `EventLog` — push events, query IPC calls
- `CommandRegistry` — register and search commands
- `EventRecorder` — start recording, checkpoint, stop
- `Redactor` — create and run redaction
- `PrivacyConfig` — allowlist/blocklist setup

**Files:** Inline in the relevant source files.
**Validation:** `cargo test --doc` passes.

### 2.3 Crate-Level README Files
**Why:** crates.io renders the crate's README as the landing page. Currently all crates point to the workspace README, which doesn't explain individual crate usage.

**Create:**
- `crates/victauri-core/README.md` — what types are provided, when to use standalone
- `crates/victauri-macros/README.md` — `#[inspectable]` usage with examples
- `crates/victauri-plugin/README.md` — quick start, tool list, auth/privacy
- `crates/victauri-watchdog/README.md` — usage, env vars, recovery commands

**Files:** New files in each crate directory.

---

## Phase 3: Test Coverage (High Priority)

### 3.1 Core Crate Coverage Gaps
**Why:** 20+ public functions have zero test coverage.

**Missing tests:**
- `EventLog`: `snapshot_range()`, `since()`, `ipc_calls()`, `ipc_calls_since()`, `len()`, `is_empty()`, `clear()`
- `EventRecorder`: `checkpoint_count()`, `session_id()`, `events_between()`
- `DomSnapshot`: `to_accessible_text()`, `DomElement` accessors
- `verify_state`, `check_ipc_integrity` — edge cases with malformed inputs

**Files:** `crates/victauri-core/tests/core_tests.rs`
**Target:** Cover every public function with at least one positive and one edge-case test.

### 3.2 Plugin Unit Test Gaps
**Why:** auth module, bridge module, memory module, and individual Tauri command handlers lack dedicated tests.

**Missing tests:**
- `auth.rs`: `dns_rebinding_guard` (allowed hosts, blocked hosts, missing header), `origin_guard` (allowed origins, blocked origins, no origin), `security_headers` (verify all 3 headers present)
- `bridge.rs`: Mock bridge method tests (already has some in integration, needs unit coverage)
- `memory.rs`: Verify JSON structure of `current_stats()` output
- `tools.rs`: Individual Tauri command handlers (need mock AppHandle)

**Files:** Add `#[cfg(test)]` modules in each file.
**Target:** 90%+ function coverage across the plugin crate.

### 3.3 MCP Protocol Compliance Tests
**Why:** The MCP server should be tested against the actual protocol, not just individual tools.

**Tests to add:**
- Send malformed JSON-RPC to `/mcp` — verify proper error response
- Send unknown method — verify `MethodNotFound` error
- Verify `tools/list` returns all 59 tools with valid schemas
- Verify zero-param tools have `{"type": "object", "properties": {}}` inputSchema
- Verify `resources/list` returns all 3 resources
- Verify SSE event stream opens and closes cleanly
- Verify concurrent tool calls don't deadlock

**Files:** `crates/victauri-plugin/tests/mcp_protocol_tests.rs`

---

## Phase 4: Platform Completeness (Medium Priority)

### 4.1 Linux Screenshot (X11)
**Why:** Linux is the third leg of the platform tripod. Returning an error is acceptable for Wayland, but X11 should work.

**How:**
- Add `x11rb = "0.14"` dependency (optional, behind `cfg(target_os = "linux")`)
- Implement via `x11rb::protocol::xproto::GetImage` on the target window
- Convert `ZPixmap` BGRA data to RGBA, encode with existing `encode_png()`
- Wayland: return descriptive error explaining xdg-portal requirement
- Feature-gate behind `linux-screenshot` feature flag (off by default to avoid CI dependency)

**Files:** `crates/victauri-plugin/src/screenshot.rs`, `crates/victauri-plugin/Cargo.toml`
**Tests:** Unit test with a synthetic pixel buffer through `encode_png()` (already exists). Integration test only on Linux CI with X11.

### 4.2 Session Disk Persistence
**Why:** Time-travel recordings are lost on app restart. Developers want to save and replay sessions.

**How:**
- Add `save_session(path)` and `load_session(path)` functions to `EventRecorder`
- `RecordedSession` already derives `Serialize`/`Deserialize` — just need `serde_json::to_writer` / `from_reader`
- Add `save_recording` and `load_recording` MCP tools (path parameter, security: validate path doesn't escape app data dir)
- Default save directory: Tauri's `app_data_dir()` + `/victauri/recordings/`

**Files:** `crates/victauri-core/src/recording.rs`, `crates/victauri-plugin/src/mcp.rs`
**Tests:** Round-trip test: record, save, load, verify equality.

---

## Phase 5: Observability & Diagnostics (Medium Priority)

### 5.1 Structured Tracing on Tool Invocations
**Why:** When debugging agent behavior, developers need to see which tools were called, with what params, and how long they took.

**How:**
- Do NOT use `#[tracing::instrument]` on every handler (overhead too high for polled tools)
- Add `tracing::debug!(tool = "eval_js", webview = ?label)` at the top of each handler
- Add `tracing::warn!` for tool failures (timeout, eval error, privacy block)
- Add tool invocation counter to `get_plugin_info` response
- Set default tracing filter to `warn`; `RUST_LOG=victauri=debug` for verbose mode

**Files:** `crates/victauri-plugin/src/mcp.rs`
**Tests:** Verify `get_plugin_info` includes invocation counts.

### 5.2 Health Endpoint Enhancement
**Why:** `/health` returns just `"ok"`. Production monitoring needs more.

**How:**
- Return JSON: `{"status": "ok", "uptime_secs": N, "tools_invoked": N, "pending_evals": N, "bridge_connected": bool}`
- Keep backward compatible — `"ok"` string response when no `Accept: application/json` header
- Add `GET /metrics` endpoint (opt-in) with Prometheus-format counters

**Files:** `crates/victauri-plugin/src/mcp.rs`
**Tests:** Integration test parsing health JSON response.

---

## Phase 6: Ecosystem & Publishing (Lower Priority)

### 6.1 Crate Publication Workflow
**Why:** Manual `cargo publish` in dependency order is error-prone.

**How:**
- Add `.github/workflows/release.yml` triggered by git tags matching `v*`
- Publish order: victauri-core, victauri-macros, victauri-plugin, victauri-watchdog
- Wait between publishes for crates.io index propagation
- Include changelog extraction from CHANGELOG.md
- Create GitHub Release with tag

**Files:** `.github/workflows/release.yml`

### 6.2 cargo-deny Configuration
**Why:** Dependency license and advisory scanning beyond cargo-audit.

**How:**
- Add `deny.toml` with: license allowlist (Apache-2.0, MIT, ISC, BSD-2, BSD-3, MPL-2.0), advisory db check, duplicate crate detection
- Add `cargo deny check` to CI

**Files:** `deny.toml`, `.github/workflows/ci.yml`

### 6.3 Example: AI Agent Integration
**Why:** Developers want to see a real agent interacting with Victauri, not just a demo app.

**How:**
- Add `examples/agent-test/` — a script that connects to a running Victauri app via MCP and runs a scripted test sequence
- Use `rmcp` client or raw HTTP to demonstrate: snapshot DOM, click button, verify state changed, take screenshot
- Document in workspace README

**Files:** `examples/agent-test/`

---

## Execution Order

The phases are ordered by developer trust impact. Within each phase, items are independent and can be parallelized.

| Priority | Items | Est. Effort | Impact |
|----------|-------|-------------|--------|
| **Do first** | 1.1 Shutdown, 1.2 Validation, 2.1 Docs | 4-6 hours | Prevents trust-breaking failures |
| **Do second** | 1.3 Bridge version, 1.4 Cleanup, 3.1-3.3 Tests | 6-8 hours | Completeness and confidence |
| **Do third** | 2.2 Doctests, 2.3 READMEs, 5.1 Tracing | 3-4 hours | Developer experience |
| **Do fourth** | 4.1 Linux screenshot, 4.2 Session persistence | 4-6 hours | Platform completeness |
| **Do last** | 5.2 Health, 6.1-6.3 Ecosystem | 3-4 hours | Polish |

**Total remaining: ~20-28 hours of implementation work.**

---

## What's NOT on This List (and Why)

- **Rewrite tools.rs to use PluginError**: Deferred to 0.2.0 — changing error types across 59 tools is a breaking API change best done once.
- **WebSocket transport**: The MCP spec favors Streamable HTTP. WebSocket adds complexity with no current client demand.
- **Plugin feature flags**: Premature — the crate is small enough that compile times aren't a concern yet.
- **Internationalization**: Debug tool — English only is fine.
- **GUI configuration panel**: The builder API and env vars are sufficient for 0.x.
