# Victauri — Claude Code Instructions

## What Is Victauri

**Victauri — Verified Introspection & Control for Tauri Applications.**

X-ray vision and hands for AI agents inside Tauri apps. Unlike Playwright (which sees only the browser glass), Victauri gives agents simultaneous access to the webview DOM, the Rust backend, the IPC layer, the database, and native window state — all through a single MCP interface.

**Stack:** Pure Rust workspace (4 crates) | **Target:** Tauri 2.0 applications

## Commands

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo clippy -- -D warnings    # Lint
cargo doc --no-deps --open     # Generate docs
```

## Architecture

```
victauri/
├── crates/
│   ├── victauri-core/       # Shared types: events, registry, snapshots, verification
│   ├── victauri-macros/     # Proc macros: #[inspectable] for command instrumentation
│   ├── victauri-plugin/     # Tauri plugin: embedded MCP server + JS bridge + tools
│   └── victauri-watchdog/   # Crash-recovery sidecar (monitors plugin health)
└── examples/
    └── demo-app/            # Minimal Tauri app with Victauri wired up
```

### How It Works

1. **victauri-plugin** is added as a dev dependency to any Tauri app
2. The plugin starts an axum HTTP server on `127.0.0.1:7373` inside the app process
3. This server speaks MCP protocol (Streamable HTTP + SSE)
4. Claude Code (or any MCP client) connects and gets full-stack control

### The Three Layers

| Layer | What It Does | How |
|---|---|---|
| **WebView** | DOM snapshots, click, type, fill, eval JS | Injected JS bridge via `on_webview_ready()` |
| **IPC** | Command registry, invoke commands, intercept IPC log | Custom invoke handler wrapper + proc macros |
| **Backend** | State reading, DB queries, memory tracking | Direct `AppHandle` access (same process) |

## Crate Responsibilities

### victauri-core
Shared types used by all other crates. No Tauri dependency.
- `EventLog` — append-only ring buffer of `AppEvent` variants
- `CommandRegistry` — thread-safe registry of `CommandInfo` with search
- `DomSnapshot` / `DomElement` — accessible tree with ref handles
- `WindowState` — position, size, visibility, focus state
- `VerificationResult` / `Divergence` — cross-boundary verification output

### victauri-macros
Proc macro crate. Single attribute macro: `#[inspectable]`.
- Generates `<fn>__schema()` companion returning `CommandInfo`
- Designed to sit alongside `#[tauri::command]`
- Zero runtime cost — all code generation is compile-time

### victauri-plugin
The main crate. Tauri plugin + embedded MCP server.
- `init<R: Runtime>()` — plugin entry point, gated behind `#[cfg(debug_assertions)]`
- JS bridge injection (`js_bridge.rs`) — DOM walking, ref map, console hooks
- MCP server (`mcp.rs`) — axum on :7373, will wire up full rmcp tools/resources/prompts
- Tools (`tools.rs`) — Tauri commands for eval, window state, IPC log, registry, memory
- Screenshot (`screenshot.rs`) — platform-native window capture
- Memory (`memory.rs`) — atomic allocation tracking

### victauri-watchdog
Standalone binary. Monitors the MCP server health endpoint.
- Polls `GET /health` every 5 seconds
- Logs warnings on failure, errors after 3 consecutive misses
- Future: configurable recovery actions (restart app, notify agent)

## Principles

1. **Same-process** — the MCP server runs inside the Tauri app, not as a separate process
2. **Zero-cost in release** — everything gated behind `#[cfg(debug_assertions)]`
3. **Full-stack** — webview + IPC + backend + DB, not just DOM
4. **MCP-native** — speaks the protocol AI agents already understand
5. **Cross-platform** — no CDP dependency, works on Windows/macOS/Linux identically
6. **Plugin, not framework** — one line in Cargo.toml to add, one line to remove

## Design Decisions

- **Why embedded, not external?** Eliminates the three-hop state drift that plagues Playwright. Direct AppHandle access gives sub-ms tool response times.
- **Why axum, not stdio?** Tauri apps are GUI processes — stdin/stdout aren't wired for MCP. HTTP/SSE on localhost is the right transport for an already-running process.
- **Why ref handles, not selectors?** Following Playwright MCP's proven model. Refs are semantic (ARIA-derived), short-lived, and survive DOM restructuring within a snapshot.
- **Why a watchdog?** If the app crashes, the embedded MCP server dies. The watchdog detects this and can alert the agent or trigger recovery.

## Code Conventions

- **Rust:** snake_case functions, PascalCase types, `thiserror` for errors, `anyhow` for application errors
- **Files:** snake_case for Rust
- **No unwrap/panic in library code** — use `?` and `Result` everywhere
- **Imports:** std > external crates > workspace crates > local modules

## Phase Roadmap

### Phase 1: Foundation (Complete)
- [x] Workspace structure
- [x] Core types (events, registry, snapshots)
- [x] Proc macro skeleton (#[inspectable])
- [x] Plugin skeleton (setup, JS bridge injection, axum server)
- [x] Basic tools (eval, window state, IPC log, registry, memory)
- [x] Wire up rmcp MCP server with full tool definitions (11 tools)
- [x] Implement eval-with-return (oneshot channel callback pattern)
- [x] Platform screenshot (Windows PrintWindow → PNG)
- [x] Unit tests (10 core type tests, 3 proc macro tests)
- [x] Fix proc macro bug (type extraction via quote)
- [x] Demo app (minimal Tauri 2 app in examples/demo-app)

### Phase 2: Dual-Context Verification (Complete)
- [x] Cross-boundary state verification tool
- [x] Ghost command detection
- [x] IPC round-trip integrity checking

### Phase 3: Reactive Streaming (Complete)
- [x] MCP resource subscriptions (ipc-log, windows, state)
- [x] Push notifications on state change
- [x] Event stream filtering

### Phase 4: Intent Layer (Complete)
- [x] Command-level intent annotations
- [x] Natural language → command resolution
- [x] Semantic test assertions

### Phase 5: Time-Travel (Complete)
- [x] IPC event recording
- [x] State snapshot checkpointing
- [x] Rewind/replay tools

## Current State (2026-04-26)

**All 5 phases complete + Phase 6 enhancements + Phase 7 expansion (IPC interception, network monitoring, storage, navigation, dialogs, window management, wait_for).** All 5 crates compile cleanly (`RUSTFLAGS="-Dwarnings" cargo clippy` passes). 86 tests pass (44 core + 4 macro + 38 plugin integration). CI green on Linux/Windows/macOS. Tauri 2.10.3 + rmcp 1.5.0.

### Live test results (4DA, 2026-04-26):
Tested against 4DA (3 windows: main 1200×800, notification 440×160, briefing 560×780; 135 DOM elements; 11 buttons; React/Vite frontend on :4444). **30/30 tools+resources pass (all 27 tools + 3 resources).**

**WebView tools:**
- **eval_js**: `document.title` → `"4DA"`, `typeof __VICTAURI__` → `"object"`, complex `JSON.stringify({url, keys})` → URL + 12 bridge methods. Auto-return prepend verified. Window targeting (`webview_label:"main"`) works.
- **dom_snapshot**: Full accessible tree with ref handles, element bounds, roles, names. 1192×800 body viewport. Refs survive across interactions.
- **click**: `ref_id:"e3"` → `{ok:true}`. URL updated to `#main-content` confirming UI interaction.
- **fill**: Returns error on non-input elements (correct — e3 is a div, no input elements on current page). Fix applied: handles textarea prototype + fallback.
- **type_text**: `ref_id:"e3"` → `{ok:true}`. Dispatches keydown/keypress/input/keyup events.
- **press_key** [NEW]: Tab → `{ok:true}`, Escape → `{ok:true}`, Enter → `{ok:true}`, ArrowDown → `{ok:true}`, F5 → `{ok:true}`.

**Window tools:**
- **list_windows**: `["notification","briefing","main"]` — all 3 windows.
- **get_window_state**: main (1200×800, visible, `http://localhost:4444/#main-content`), notification (440×160, hidden), briefing (560×780, hidden). Full position/size/visibility/focus/URL data.
- **screenshot** [NEW]: Returns valid base64 PNG (`iVBORw0KGgo...`) via `PrintWindow`+`GetDIBits`. Both default and `window_label:"main"` work.

**Backend tools:**
- **invoke_command** [NEW]: `get_settings` → full 4DA settings JSON (license tier, LLM config, rerank settings, monitoring). `get_monitoring_status` → live monitoring state (enabled, interval, last check timestamp). `get_license_status` → `{}`. Works with args: `search_context` with query param. Nonexistent commands return `{}` (Tauri behavior).
- **get_ipc_log**: `[]` (correct — no IPC intercepted yet).
- **get_registry**: `[]` (4DA doesn't use `#[inspectable]`).
- **get_memory_stats** [NEW IMPL]: Real OS process memory — `working_set_bytes: 77MB`, `peak_working_set_bytes: 290MB`, `page_fault_count: 450K`, `page_file_bytes: 26MB`.
- **get_console_logs** [NEW]: Captures React DevTools message + i18next message with timestamps. `since` filter works.

**Verification tools:**
- **verify_state**: JSON comparison `{title:"4DA"}` → `passed:true, divergences:[]`. Detects divergence when backend_state mismatches (`"Wrong Title"` → `passed:false` with Error severity).
- **detect_ghost_commands**: `ghost_commands:[], total_frontend_commands:0, total_registry_commands:0`.
- **check_ipc_integrity**: `healthy:true, total_calls:0, completed:0, pending:0, errored:0`.

**Streaming tools:**
- **get_event_stream**: Returns combined console+DOM mutation events with timestamps. Previously broken (getEventStream undefined in bridge) — **fixed**: deferred MutationObserver init until DOM ready.

**Intent tools:**
- **resolve_command**: `"show settings"` and `"increase counter"` → `[]` (4DA has no `#[inspectable]` commands registered). Correct behavior.
- **assert_semantic**: `expression:"document.title", condition:"equals", expected:"4DA"` → `passed:true, actual:"4DA"`. `expression:"document.querySelectorAll('nav').length", condition:"truthy"` → `passed:true, actual:1`.

**Time-travel tools:**
- **start_recording** → `session_id` UUID, `started:true`.
- **checkpoint** → `checkpoint_id`, `created:true`, `event_index:0`. Label supported.
- **list_checkpoints** → array with id, label, timestamp, state, event_index.
- **get_recorded_events** → events array. **get_replay_sequence** → IPC events only.
- **events_between_checkpoints** → events between named checkpoints.
- **stop_recording** → full session with events + checkpoints.

**Resources:**
- `victauri://state` → `{commands_registered:0, events_captured:0, memory:{working_set_bytes:...}, port:7373}`.
- `victauri://windows` → JSON array of all 3 window states.
- `victauri://ipc-log` → `[]`.

**Health/Info:**
- `/health` → `ok` (no auth required).
- `/info` → `{name:"victauri", port:7373, protocol:"mcp", version:"0.1.0", auth_required:false, commands_registered:0, events_captured:0}`.

**Bridge methods (12):** version, snapshot, getRef, click, fill, type, pressKey, getConsoleLogs, clearConsoleLogs, getMutationLog, clearMutationLog, getEventStream.

### What exists and works:
- **victauri-core**: `EventLog` (ring buffer), `CommandRegistry` (BTreeMap with search + NL resolve), `DomSnapshot`, `WindowState`, `VerificationResult`/`Divergence`, `GhostCommandReport`, `IpcIntegrityReport`, `SemanticAssertion`/`AssertionResult`, `ScoredCommand`, `EventRecorder` (time-travel recording with checkpoints), `RecordedSession`, `RecordedEvent`, `StateCheckpoint`. 44 unit tests.
- **victauri-macros**: `#[inspectable]` proc macro with `description`, `intent`, `category`, `example` attributes. Uses proper `syn::meta` parsing (not string matching). Generates `<fn>__schema()` returning `CommandInfo` with full intent metadata. 4 integration tests.
- **victauri-plugin**: Full MCP server with **47 tools** + 3 resources. Tools organized by category:
  - **WebView (11)**: eval_js, dom_snapshot, click, double_click, hover, fill, type_text, press_key, select_option, scroll_to, focus_element
  - **Windows (7)**: get_window_state, list_windows, screenshot, manage_window, resize_window, move_window, set_window_title
  - **Backend (5)**: invoke_command, get_ipc_log, get_registry, get_memory_stats, get_console_logs
  - **Network (1)**: get_network_log
  - **Storage (4)**: get_storage, set_storage, delete_storage, get_cookies
  - **Navigation (3)**: get_navigation_log, navigate, navigate_back
  - **Dialogs (2)**: get_dialog_log, set_dialog_response
  - **Verification (3)**: verify_state, detect_ghost_commands, check_ipc_integrity
  - **Streaming (1)**: get_event_stream
  - **Intent (2)**: resolve_command, assert_semantic
  - **Wait (1)**: wait_for
  - **Time-Travel (7)**: start_recording, stop_recording, checkpoint, list_checkpoints, get_replay_sequence, get_recorded_events, events_between_checkpoints
  Resources: victauri://ipc-log, victauri://windows, victauri://state with subscribe/unsubscribe. JS bridge v0.2.0 with IPC interception, network monitoring, storage access, navigation tracking, dialog capture, extended interactions, and waitFor. `EventRecorder` with 50,000 event capacity. **Release-safe**: `init()` returns a no-op plugin in release builds via `#[cfg(debug_assertions)]` gate. `VictauriBuilder` for port/capacity/auth configuration + `VICTAURI_PORT`/`VICTAURI_AUTH_TOKEN` env vars. Bearer token auth middleware (opt-in). Tool enable/disable via builder. 38 integration tests (mock bridge, HTTP endpoints, MCP protocol, tool/resource listing, resource reading, recording, memory stats, auth accept/reject/bypass).
- **victauri-watchdog**: Configurable via env vars (`VICTAURI_PORT`, `VICTAURI_INTERVAL`, `VICTAURI_MAX_FAILURES`, `VICTAURI_ON_FAILURE`). Proper `tracing-subscriber` log output. Executes configurable recovery commands on failure. Fires recovery action once per failure cycle, resets on recovery.
- **demo-app**: Tauri 2 app in `examples/demo-app/` with Victauri wired up. 12 commands (greet, counter CRUD, todo CRUD, settings, app state dump) all decorated with `#[inspectable]` including intent, category, examples. Includes `.mcp.json` for immediate Claude Code connection.
- **CI**: GitHub Actions workflow (`ci.yml`) — clippy + tests + docs on Linux/Windows/macOS, format check on Linux. All crate code passes `cargo fmt --check`.

### Architecture notes:
- **bridge.rs** — `WebviewBridge` trait (public) erases the Tauri `Runtime` generic, allowing the MCP handler (which can't be generic) to access webview windows via `Arc<dyn WebviewBridge>`. 8 methods: eval_webview, get_window_states, list_window_labels, get_native_handle, manage_window, resize_window, move_window, set_window_title. Impl provided for `AppHandle<R: Runtime>`. Testable via mock implementations.
- **mcp.rs** — rmcp `#[tool_router]` + `#[tool_handler]` macros generate the MCP server. `build_app()` constructs the axum `Router` independently of Tauri (testable). `StreamableHttpService` serves on `/mcp`. Health/info endpoints on `/health` and `/info`. Parameter structs derive `schemars::JsonSchema` for automatic MCP tool schema generation. `VictauriMcpHandler::new()` public constructor for testing.
- **tools.rs** — Tauri commands still work independently for in-app IPC. Both the MCP tools and Tauri commands use the same `pending_evals` mechanism for JS eval with return.
- **screenshot.rs** — Windows: `PrintWindow` → `GetDIBits` (BGRA) → RGBA → stored-deflate PNG. macOS: `CGWindowListCreateImage` → `CGBitmapContext` (RGBA) → PNG. Both use the same custom PNG encoder (raw zlib stored blocks + CRC32 + Adler32). Linux: returns error (not yet implemented).
- **auth.rs** — Optional Bearer token authentication. `require_auth` axum middleware skips `/health` but protects `/mcp` and `/info`. Token from `VictauriBuilder::auth_token()`, `generate_auth_token()`, or `VICTAURI_AUTH_TOKEN` env var.
- **JS bridge injection** — Uses `js_init_script()` (persistent) instead of `on_webview_ready()` + `eval()` (one-shot). This ensures the bridge survives page navigations in Vite dev mode. MutationObserver init is deferred via `DOMContentLoaded` fallback to avoid crash when `document.documentElement` isn't ready during early script execution. Bridge v0.2.0 includes IPC interception (monkey-patches `__TAURI_INTERNALS__.invoke`, skips `plugin:victauri|` calls), network interception (fetch + XMLHttpRequest), navigation tracking (pushState/replaceState/popstate/hashchange), dialog capture with configurable auto-responses, and waitFor polling. Log caps: consoleLogs 1000, ipcLog 2000, networkLog 1000, navigationLog 200, dialogLog 100.
- **eval auto-return** — `eval_with_return()` auto-prepends `return` to bare expressions (e.g. `document.title` → `return document.title`). Only checks `starts_with("return ")` — NOT `contains("return ")` — so IIFEs with internal returns are handled correctly. Skips statement keywords (`if`, `for`, `const`, etc.).
- **Multi-window safety** — Default window selection prefers "main" → first visible → any, avoiding silent failures when hidden windows lack plugin capabilities.
- **CSP compatibility** — `eval()` cannot be used inside injected scripts when CSP has `script-src 'self'` without `'unsafe-eval'`. The eval wrapper uses direct `(async () => { ... })()` pattern instead.

### Key technical decisions already made:
- MCP server is EMBEDDED in Tauri process (not separate), via axum on `:7373`
- `rmcp` v1.5.0 is the MCP SDK, feature `transport-streamable-http-server`
- JS bridge uses ref handles (Playwright pattern), not CSS selectors
- All plugin code gated behind `#[cfg(debug_assertions)]` — `init()` returns no-op plugin in release builds
- OS-level process memory tracking (Windows `GetProcessMemoryInfo`, Linux `/proc/self/statm`) — real metrics, no consumer opt-in needed
- Event log is a `VecDeque` ring buffer with 10,000 capacity
- WebviewBridge trait object pattern for runtime-erased AppHandle access
- `tokio::sync::Mutex` for pending_evals (async lock needed across eval timeout awaits)
- `build_app()` separated from `start_server()` — router construction is testable without Tauri runtime
- IPC data pipeline reads from JS bridge (not Rust EventLog) — `get_ipc_log`, `detect_ghost_commands`, `check_ipc_integrity` all call `__VICTAURI__` methods via `eval_with_return()`
- Eval timeout is 30s (not 10s) to support `wait_for` tool's configurable polling timeout

### Relationship to 4DA:
Victauri is a standalone open-source project. 4DA has `victauri-plugin` as a path dependency (`path = "../../runyourempire/victauri/crates/victauri-plugin"` in `src-tauri/Cargo.toml`), with `victauri:default` in capabilities and `.mcp.json` configured. They share no code. The 4DA repo is at `D:\4DA`, this repo is at `D:\runyourempire\victauri`.

### Owner:
4DA Systems Pty Ltd (ACN 696 078 841). Apache-2.0 license. Contact: hello@4da.ai.

## Never Commit
- `target/` — build artifacts
- Any API keys or credentials
