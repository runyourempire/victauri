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

**All 5 phases complete. Extensively live-tested against 4DA (real multi-window Tauri app).** All 5 crates compile cleanly (`RUSTFLAGS="-Dwarnings" cargo clippy` passes). 64 tests pass (44 core + 4 macro + 16 plugin integration). CI green on Linux/Windows/macOS. Tauri 2.10.3 + rmcp 1.5.0.

### Live test results (4DA, 2026-04-26):
Tested against 4DA (3 windows: main 1200×800, notification 440×160, briefing 560×780; 135 DOM elements; React/Vite frontend on :4444):
- **eval_js**: `typeof __VICTAURI__` → `"object"`, `document.title` → `"4DA"`, IIFE with return → correct result, complex `Array.from()` expressions → button text list. Auto-return prepend verified working.
- **dom_snapshot**: 34KB accessible tree with ref handles, element bounds, roles, names. Refs survive across interactions.
- **click**: Clicked "Skip to main content" (`e3`) and "Signal" tab (`e87`) — both `{ok:true}`, UI updated (visible text confirmed "Navigated to Signal").
- **verify_state**: Detected URL divergence after anchor click (`#main-content` vs `/`). Cross-boundary comparison with zero divergences when states match.
- **assert_semantic**: Truthy (`nav exists` → `passed`), equals (`title === "4DA"` → `passed`).
- **list_windows / get_window_state**: All 3 windows with position, size, visibility, focus, URL.
- **check_ipc_integrity**: `healthy: true`, correct zero-counts.
- **detect_ghost_commands**: Empty when no IPC traffic (correct).
- **Time-travel**: start_recording → checkpoint → interaction → checkpoint → list_checkpoints → stop_recording — full session recorded with timestamps and state snapshots.
- **Resources**: `victauri://state` (port, commands, events), `victauri://windows` (all 3 windows), `victauri://ipc-log` (empty, correct).
- **Health/Info**: `/health` → `ok`, `/info` → `{name:"victauri", port:7373, protocol:"mcp", version:"0.1.0"}`.

### What exists and works:
- **victauri-core**: `EventLog` (ring buffer), `CommandRegistry` (BTreeMap with search + NL resolve), `DomSnapshot`, `WindowState`, `VerificationResult`/`Divergence`, `GhostCommandReport`, `IpcIntegrityReport`, `SemanticAssertion`/`AssertionResult`, `ScoredCommand`, `EventRecorder` (time-travel recording with checkpoints), `RecordedSession`, `RecordedEvent`, `StateCheckpoint`. 44 unit tests.
- **victauri-macros**: `#[inspectable]` proc macro with `description`, `intent`, `category`, `example` attributes. Uses proper `syn::meta` parsing (not string matching). Generates `<fn>__schema()` returning `CommandInfo` with full intent metadata. 4 integration tests.
- **victauri-plugin**: Full MCP server with 24 tools + 3 resources. Tools: eval_js, dom_snapshot, click, fill, type_text, get_window_state, list_windows, get_ipc_log, get_registry, get_memory_stats (P1), verify_state, detect_ghost_commands, check_ipc_integrity (P2), get_event_stream (P3), resolve_command, assert_semantic (P4), start_recording, stop_recording, checkpoint, list_checkpoints, get_replay_sequence, get_recorded_events, events_between_checkpoints (P5). Resources: victauri://ipc-log, victauri://windows, victauri://state with subscribe/unsubscribe. JS bridge includes MutationObserver + event stream. `EventRecorder` with 50,000 event capacity in `VictauriState`. **Release-safe**: `init()` returns a no-op plugin in release builds via `#[cfg(debug_assertions)]` gate. 16 integration tests (mock bridge, HTTP endpoints, full MCP protocol handshake, tool/resource listing).
- **victauri-watchdog**: Configurable via env vars (`VICTAURI_PORT`, `VICTAURI_INTERVAL`, `VICTAURI_MAX_FAILURES`, `VICTAURI_ON_FAILURE`). Proper `tracing-subscriber` log output. Executes configurable recovery commands on failure. Fires recovery action once per failure cycle, resets on recovery.
- **demo-app**: Minimal Tauri 2 app in `examples/demo-app/` with Victauri wired up. Greet command + counter with backend state. Includes `.mcp.json` for immediate Claude Code connection.
- **CI**: GitHub Actions workflow (`ci.yml`) — clippy + tests + docs on Linux/Windows/macOS, format check on Linux. All crate code passes `cargo fmt --check`.

### Architecture notes:
- **bridge.rs** — `WebviewBridge` trait (public) erases the Tauri `Runtime` generic, allowing the MCP handler (which can't be generic) to access webview windows via `Arc<dyn WebviewBridge>`. Impl provided for `AppHandle<R: Runtime>`. Testable via mock implementations.
- **mcp.rs** — rmcp `#[tool_router]` + `#[tool_handler]` macros generate the MCP server. `build_app()` constructs the axum `Router` independently of Tauri (testable). `StreamableHttpService` serves on `/mcp`. Health/info endpoints on `/health` and `/info`. Parameter structs derive `schemars::JsonSchema` for automatic MCP tool schema generation. `VictauriMcpHandler::new()` public constructor for testing.
- **tools.rs** — Tauri commands still work independently for in-app IPC. Both the MCP tools and Tauri commands use the same `pending_evals` mechanism for JS eval with return.
- **screenshot.rs** — Windows: `PrintWindow` → `GetDIBits` (BGRA) → RGBA → stored-deflate PNG. Zero external dependencies beyond the `windows` crate. PNG encoder: raw zlib stored blocks + CRC32 + Adler32.
- **JS bridge injection** — Uses `js_init_script()` (persistent) instead of `on_webview_ready()` + `eval()` (one-shot). This ensures the bridge survives page navigations in Vite dev mode.
- **eval auto-return** — `eval_with_return()` auto-prepends `return` to bare expressions (e.g. `document.title` → `return document.title`). Only checks `starts_with("return ")` — NOT `contains("return ")` — so IIFEs with internal returns are handled correctly. Skips statement keywords (`if`, `for`, `const`, etc.).
- **Multi-window safety** — Default window selection prefers "main" → first visible → any, avoiding silent failures when hidden windows lack plugin capabilities.
- **CSP compatibility** — `eval()` cannot be used inside injected scripts when CSP has `script-src 'self'` without `'unsafe-eval'`. The eval wrapper uses direct `(async () => { ... })()` pattern instead.

### Key technical decisions already made:
- MCP server is EMBEDDED in Tauri process (not separate), via axum on `:7373`
- `rmcp` v1.5.0 is the MCP SDK, feature `transport-streamable-http-server`
- JS bridge uses ref handles (Playwright pattern), not CSS selectors
- All plugin code gated behind `#[cfg(debug_assertions)]` — `init()` returns no-op plugin in release builds
- `GlobalAlloc` wrapper pattern for memory tracking (atomics, zero-dep)
- Event log is a `VecDeque` ring buffer with 10,000 capacity
- WebviewBridge trait object pattern for runtime-erased AppHandle access
- `tokio::sync::Mutex` for pending_evals (async lock needed across eval timeout awaits)
- `build_app()` separated from `start_server()` — router construction is testable without Tauri runtime

### Relationship to 4DA:
Victauri is a standalone open-source project. 4DA will eventually add `victauri-plugin` as a dev dependency. They share no code. The 4DA repo is at `D:\4DA`, this repo is at `D:\runyourempire\victauri`.

### Owner:
4DA Systems Pty Ltd (ACN 696 078 841). Apache-2.0 license. Contact: hello@4da.ai.

## Never Commit
- `target/` — build artifacts
- Any API keys or credentials
