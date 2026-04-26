# Victauri

**Full-stack introspection for Tauri apps via MCP.**

[![CI](https://github.com/runyourempire/victauri/actions/workflows/ci.yml/badge.svg)](https://github.com/runyourempire/victauri/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

Victauri is a Tauri 2.0 plugin that embeds an [MCP](https://modelcontextprotocol.io) server inside your app process. AI agents get direct access to the webview, Rust backend, IPC layer, and native window state — not through an external bridge, but from inside the process itself.

## What It Does

Playwright sees the DOM. Victauri sees the DOM, the IPC bridge, the command registry, backend state, and the gaps between them.

| Capability | Playwright | Victauri |
|---|---|---|
| DOM interaction | Yes | Yes |
| Screenshots | Yes | Yes |
| Backend state access | No | **Yes** |
| IPC interception | No | **Yes** |
| Command registry introspection | No | **Yes** |
| Cross-boundary state verification | No | **Yes** |
| Ghost command detection | No | **Yes** |
| Event recording & replay | No | **Yes** |

## What It Doesn't Do (Yet)

- **No multi-window orchestration** — tools operate on the primary webview
- **No Linux screenshots** — Windows and macOS only
- **No persistent recording** — sessions live in memory, lost on restart
- **No production use** — gated behind `#[cfg(debug_assertions)]`, debug builds only
- **No remote access** — localhost only, by design
- **Early project** — API surface may change between versions

## Quick Start

Add to your Tauri app's `Cargo.toml`:

```toml
[dependencies]
victauri-plugin = { git = "https://github.com/runyourempire/victauri" }
```

Wire it up:

```rust
tauri::Builder::default()
    .plugin(victauri_plugin::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

In release builds, `init()` returns a no-op plugin. Zero overhead, no conditional compilation needed.

Connect Claude Code (or any MCP client):

```json
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

## 55 MCP Tools

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

## How It Works

```
MCP Client <-> HTTP/SSE on :7373 <-> Victauri Plugin (in-process)
                                        |-- WebView: DOM, click, type, eval JS
                                        |-- IPC: registry, invoke, intercept
                                        '-- Backend: state, memory, events
```

The MCP server runs inside the Tauri process. No external sidecar, no CDP, no WebDriver. Direct `AppHandle` access means tool calls resolve in the same process as your app.

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

`#[inspectable]` auto-generates a JSON schema, making commands discoverable through the registry and natural language resolution.

## Authentication

By default, the MCP endpoint is open to localhost. To restrict access:

```rust
VictauriBuilder::new()
    .generate_auth_token()  // prints token to logs on startup
    .build()
```

Or via environment variable: `VICTAURI_AUTH_TOKEN=my-secret-token`.

```json
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

## Privacy Controls

Victauri includes a privacy layer for controlling what tools and commands are exposed:

- **Command allowlists/blocklists** — restrict which Tauri commands are invokable
- **Tool disabling** — hide specific MCP tools (e.g., `eval_js`, `screenshot`)
- **Output redaction** — automatic regex-based redaction of API keys, JWTs, emails, credit card numbers, and sensitive JSON fields
- **Strict mode** — one-call preset that disables all mutating tools and enables full redaction

## Security Model

- Localhost only — no remote access, no forwarding
- Auth tokens are optional but recommended for shared machines
- Rate limiting (100 req/sec default, configurable)
- All tools compile away in release builds
- `eval_js` executes arbitrary JavaScript in the webview — treat it like a browser devtools console
- IPC interception is read-only (monitors `fetch` to `ipc.localhost`)

## Architecture

```
victauri/
├── crates/
│   ├── victauri-core/       # Types: events, registry, snapshots, verification
│   ├── victauri-macros/     # #[inspectable] proc macro
│   ├── victauri-plugin/     # Tauri plugin: MCP server + JS bridge
│   └── victauri-watchdog/   # Health-check sidecar
└── examples/
    └── demo-app/            # Minimal Tauri app with Victauri wired up
```

## Development

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests (136)
cargo bench -p victauri-core   # Criterion benchmarks (13)
cargo clippy -- -D warnings    # Lint
cargo fmt --all -- --check     # Format check
```

## CI Integration

Run Victauri-powered checks in GitHub Actions:

```yaml
- name: Build app (debug)
  run: cargo build -p my-app

- name: Start app in background
  run: cargo run -p my-app &
  env:
    VICTAURI_AUTH_TOKEN: ${{ secrets.VICTAURI_TOKEN }}

- name: Wait for MCP server
  run: |
    for i in $(seq 1 30); do
      curl -sf http://127.0.0.1:7373/mcp && break || sleep 1
    done

- name: Run MCP-based checks
  run: node tests/mcp-checks.js
```

Because Victauri compiles away in release builds, CI runs debug builds to get introspection. The MCP server starts automatically with the app — no separate process to manage.

## Roadmap

- [ ] Multi-window tool targeting
- [ ] Linux screenshot support
- [ ] Session persistence (export/import recordings)
- [x] Benchmark suite with real response time data
- [ ] IPC debugger UI (visual timeline of command flow)
- [ ] Test assertion helpers as a standalone crate
- [ ] crates.io publication

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

Built by [4DA Systems](https://4da.ai).
