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
| Memory & performance profiling | No | **Yes** |
| Event recording & replay | No | **Yes** |
| Semantic assertions | No | **Yes** |
| Accessibility auditing | Limited | **Yes** |
| CSS introspection & injection | No | **Yes** |
| Visual debug overlays | No | **Yes** |
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

For custom configuration:

```rust
use victauri_plugin::VictauriBuilder;

tauri::Builder::default()
    .plugin(VictauriBuilder::new().port(8080).build())
    // ...
```

Or set `VICTAURI_PORT=8080` as an environment variable.

Run your app in debug mode. Victauri starts an MCP server on `127.0.0.1:7373` (or your configured port). Connect Claude Code:

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

### 55 MCP Tools

| Category | Tools |
|---|---|
| **WebView** | `eval_js`, `dom_snapshot`, `click`, `double_click`, `hover`, `fill`, `type_text`, `press_key`, `select_option`, `scroll_to`, `focus_element` |
| **Windows** | `get_window_state`, `list_windows`, `screenshot`, `manage_window`, `resize_window`, `move_window`, `set_window_title` |
| **Backend** | `invoke_command`, `get_ipc_log`, `get_registry`, `get_memory_stats`, `get_console_logs` |
| **Network** | `get_network_log` |
| **Storage** | `get_storage`, `set_storage`, `delete_storage`, `get_cookies` |
| **Navigation** | `get_navigation_log`, `navigate`, `navigate_back` |
| **Dialogs** | `get_dialog_log`, `set_dialog_response` |
| **Verification** | `verify_state`, `detect_ghost_commands`, `check_ipc_integrity` |
| **Streaming** | `get_event_stream` |
| **Intent** | `resolve_command`, `assert_semantic` |
| **Wait** | `wait_for` |
| **Time-Travel** | `start_recording`, `stop_recording`, `checkpoint`, `list_checkpoints`, `get_replay_sequence`, `get_recorded_events`, `events_between_checkpoints` |
| **CSS/Style** | `get_styles`, `get_bounding_boxes`, `inject_css`, `remove_injected_css` |
| **Visual Debug** | `highlight_element`, `clear_highlights` |
| **Accessibility** | `audit_accessibility` |
| **Performance** | `get_performance_metrics` |

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

## Authentication

By default, the MCP endpoint is open to any process on localhost. To restrict access:

```rust
use victauri_plugin::VictauriBuilder;

tauri::Builder::default()
    .plugin(
        VictauriBuilder::new()
            .generate_auth_token()  // prints token to logs on startup
            .build()
    )
    // ...
```

Or set a specific token:

```rust
VictauriBuilder::new()
    .auth_token("my-secret-token")
    .build()
```

Or via environment variable: `VICTAURI_AUTH_TOKEN=my-secret-token`.

When auth is enabled, all requests to `/mcp` and `/info` require a `Authorization: Bearer <token>` header. The `/health` endpoint is always open.

```json
// .mcp.json with auth
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp",
      "headers": {
        "Authorization": "Bearer <token-from-logs>"
      }
    }
  }
}
```

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
cargo test                     # Run all 86 tests
cargo clippy -- -D warnings    # Lint (zero warnings)
cargo fmt --all -- --check     # Format check
cargo doc --no-deps --open     # Generate docs
```

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

Built by [4DA Systems](https://4da.ai).
