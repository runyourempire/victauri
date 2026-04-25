# Victauri

**Verified Introspection & Control for Tauri Applications**

X-ray vision and hands for AI agents inside Tauri apps.

[![CI](https://github.com/runyourempire/victauri/actions/workflows/ci.yml/badge.svg)](https://github.com/runyourempire/victauri/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

Victauri is a Tauri 2.0 plugin that turns any Tauri application into an MCP-controllable target. AI agents get full-stack access — not just the webview, but the Rust backend, IPC layer, and native window state — all through a single [Model Context Protocol](https://modelcontextprotocol.io) interface.

## Why Not Playwright?

Playwright gives agents eyes and hands **on the glass**. It sees the DOM, clicks buttons, fills forms. But for Tauri apps, the interesting stuff lives *behind* the glass:

| Capability | Playwright | Victauri |
|---|---|---|
| DOM interaction | Yes | Yes |
| Screenshots | Yes | Yes |
| Backend state access | No | **Yes** |
| IPC interception | No | **Yes** |
| Command registry | No | **Yes** |
| Cross-boundary verification | No | **Yes** |
| Memory attribution | No | **Yes** |
| Event recording & replay | No | **Yes** |
| Semantic assertions | No | **Yes** |
| Native on all platforms | Browser only | **Native** |

Victauri doesn't replace Playwright for web testing. It does what Playwright structurally cannot do for desktop applications.

## Quick Start

Add to your Tauri app's `Cargo.toml`:

```toml
[dependencies]
victauri-plugin = { git = "https://github.com/runyourempire/victauri" }
```

Wire it up in your `main.rs` or `lib.rs`:

```rust
tauri::Builder::default()
    .plugin(victauri_plugin::init())
    // ... your other plugins and setup
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

`init()` is gated behind `#[cfg(debug_assertions)]` — in release builds it returns a no-op plugin with zero overhead. No conditional compilation needed on your side.

Run your app in debug mode. Victauri starts an MCP server on `127.0.0.1:7373`. Connect Claude Code:

```json
// .mcp.json (in your project root)
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

Now Claude Code can see and control your entire app.

## What You Get

### 24 MCP Tools

| Phase | Tools |
|---|---|
| **WebView** | `eval_js`, `dom_snapshot`, `click`, `fill`, `type_text` |
| **Windows** | `get_window_state`, `list_windows` |
| **IPC** | `get_ipc_log`, `get_registry`, `get_memory_stats` |
| **Verification** | `verify_state`, `detect_ghost_commands`, `check_ipc_integrity` |
| **Streaming** | `get_event_stream` |
| **Intent** | `resolve_command`, `assert_semantic` |
| **Time-Travel** | `start_recording`, `stop_recording`, `checkpoint`, `list_checkpoints`, `get_replay_sequence`, `get_recorded_events`, `events_between_checkpoints` |

### 3 MCP Resources

- `victauri://ipc-log` — Live IPC call log with subscribe/unsubscribe
- `victauri://windows` — Window state feed
- `victauri://state` — Plugin state (event count, registered commands, memory)

## How It Works

Victauri runs **inside** your Tauri app process. No external process, no socket bridge, no CDP dependency.

```
Claude Code <-> HTTP/SSE on :7373 <-> Victauri Plugin (same process as your app)
                                          |-- WebView: DOM snapshots, click, type, eval JS
                                          |-- IPC: command registry, invoke, intercept log
                                          '-- Backend: state reading, memory tracking
```

## Instrument Your Commands

```rust
use victauri_plugin::inspectable;

#[tauri::command]
#[inspectable(
    description = "Save API key for a provider",
    intent = "persist credentials",
    category = "settings",
    example = "save the API key"
)]
async fn save_api_key(provider: String, key: String) -> Result<(), String> {
    // your code
}
```

The `#[inspectable]` macro auto-generates a JSON schema for the command, making it discoverable by AI agents through the command registry and natural language resolution.

## Architecture

```
victauri/
├── crates/
│   ├── victauri-core/       # Shared types: events, registry, snapshots, verification
│   ├── victauri-macros/     # Proc macros: #[inspectable]
│   ├── victauri-plugin/     # Tauri plugin: embedded MCP server + JS bridge
│   └── victauri-watchdog/   # Crash-recovery sidecar
└── examples/
    └── demo-app/            # Minimal Tauri app with Victauri wired up
```

### Design Decisions

- **Embedded, not external** — the MCP server runs inside the Tauri app process. Direct `AppHandle` access gives sub-ms tool response times.
- **axum, not stdio** — Tauri apps are GUI processes. HTTP/SSE on localhost is the right transport for an already-running process.
- **Ref handles, not selectors** — following Playwright MCP's proven model. Refs are semantic (ARIA-derived), short-lived, and survive DOM restructuring within a snapshot.
- **Zero-cost in release** — everything gated behind `#[cfg(debug_assertions)]`. The MCP server, JS bridge, and all tools compile away completely.

## Watchdog

The watchdog sidecar monitors the MCP server and can execute recovery actions:

```bash
# Defaults: port 7373, 5s interval, 3 failures before action
cargo run -p victauri-watchdog

# Configure via environment
VICTAURI_PORT=7373 \
VICTAURI_INTERVAL=5 \
VICTAURI_MAX_FAILURES=3 \
VICTAURI_ON_FAILURE="./restart-app.sh" \
cargo run -p victauri-watchdog
```

## Development

```bash
cargo build                    # Build all crates
cargo test                     # Run all 64 tests
cargo clippy -- -D warnings    # Lint (zero warnings)
cargo fmt --all -- --check     # Format check
cargo doc --no-deps --open     # Generate docs
```

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

Built by [4DA Systems](https://4da.ai).
