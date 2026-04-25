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

### Phase 1: Foundation (Current)
- [x] Workspace structure
- [x] Core types (events, registry, snapshots)
- [x] Proc macro skeleton (#[inspectable])
- [x] Plugin skeleton (setup, JS bridge injection, axum server)
- [x] Basic tools (eval, window state, IPC log, registry, memory)
- [ ] Wire up rmcp MCP server with full tool definitions
- [ ] Implement eval-with-return (oneshot channel callback pattern)
- [ ] Platform screenshot (Windows first)
- [ ] Compile and pass all tests

### Phase 2: Dual-Context Verification
- [ ] Cross-boundary state verification tool
- [ ] Ghost command detection
- [ ] IPC round-trip integrity checking

### Phase 3: Reactive Streaming
- [ ] MCP resource subscriptions (ipc-log, windows, state)
- [ ] Push notifications on state change
- [ ] Event stream filtering

### Phase 4: Intent Layer
- [ ] Command-level intent annotations
- [ ] Natural language → command resolution
- [ ] Semantic test assertions

### Phase 5: Time-Travel
- [ ] IPC event recording
- [ ] State snapshot checkpointing
- [ ] Rewind/replay tools

## Current State (2026-04-25)

All 4 crates compile cleanly against Tauri 2.10.3 and rmcp 1.5.0. Initial commit `827ce49`.

### What exists and works:
- **victauri-core**: `EventLog` (append-only ring buffer), `CommandRegistry` (thread-safe BTreeMap with search), `DomSnapshot` with ref handles and accessible text output, `WindowState`, `VerificationResult`/`Divergence` types. Fully implemented.
- **victauri-macros**: `#[inspectable]` attribute proc macro. Parses `description` attr, extracts args (skipping Tauri framework types like AppHandle/State), generates `<fn>__schema()` companion returning `CommandInfo`. Skeleton — works but needs test coverage.
- **victauri-plugin**: `init<R: Runtime>()` entry point. `setup()` manages `VictauriState` (event log + registry), spawns axum server on `:7373`. `on_webview_ready()` injects JS bridge. 7 Tauri commands registered. JS bridge (`INIT_SCRIPT`) does full DOM walking with ref map, role inference, console hooking, click/fill/type via DOM events.
- **victauri-watchdog**: Polls `/health` every 5s, logs after 3 consecutive failures. Needs reqwest (added).

### What needs to happen next (Phase 1 completion):
1. **Wire up rmcp MCP server** — Replace the placeholder axum routes in `mcp.rs` with a full `rmcp` server using `#[tool]` macros. The server should expose tools matching what's in the Tauri commands (snapshot, screenshot, click, type, fill, invoke, eval, query_db, window state, memory). Use `transport-streamable-http-server` feature.
2. **Eval-with-return** — `victauri_eval_js` currently fires JS and returns "eval dispatched" (fire-and-forget). Implement the oneshot channel pattern: generate a UUID, inject JS that calls `invoke('plugin:victauri|victauri_eval_callback', {id, result})` back, Rust side awaits on a `tokio::sync::oneshot` with 10s timeout. Same pattern for `victauri_dom_snapshot`.
3. **Platform screenshot** — `screenshot.rs` has stubs. Implement Windows via `PrintWindow` Win32 API using the `windows` crate (already in deps). Return PNG bytes via `image` crate or raw bitmap → PNG encoding.
4. **Tests** — Unit tests for core types, integration test for proc macro output, basic smoke test for plugin initialization.
5. **Demo app** — Minimal Tauri 2 app in `examples/demo-app/` with Victauri wired up, proving the plugin works end-to-end.

### Key technical decisions already made:
- MCP server is EMBEDDED in Tauri process (not separate), via axum on `:7373`
- `rmcp` v1.5.0 is the MCP SDK, feature `transport-streamable-http-server`
- JS bridge uses ref handles (Playwright pattern), not CSS selectors
- All plugin code gated behind `#[cfg(debug_assertions)]`
- `GlobalAlloc` wrapper pattern for memory tracking (atomics, zero-dep)
- Event log is a `VecDeque` ring buffer with 10,000 capacity

### Relationship to 4DA:
Victauri is a standalone open-source project. 4DA will eventually add `victauri-plugin` as a dev dependency. They share no code. The 4DA repo is at `D:\4DA`, this repo is at `D:\runyourempire\victauri`.

### Owner:
4DA Systems Pty Ltd (ACN 696 078 841). Apache-2.0 license. Contact: hello@4da.ai.

## Never Commit
- `target/` — build artifacts
- Any API keys or credentials
