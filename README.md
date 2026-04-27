# Victauri

**X-ray vision and hands for AI agents inside Tauri apps.**

[![CI](https://github.com/runyourempire/victauri/actions/workflows/ci.yml/badge.svg)](https://github.com/runyourempire/victauri/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/victauri-plugin.svg)](https://crates.io/crates/victauri-plugin)
[![docs.rs](https://docs.rs/victauri-plugin/badge.svg)](https://docs.rs/victauri-plugin)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

Unlike Playwright (which sees only the browser glass), Victauri gives agents simultaneous access to the webview DOM, the Rust backend, the IPC layer, and native window state — all through a single [MCP](https://modelcontextprotocol.io) interface running inside the app process.

| Capability | Playwright | Victauri |
|---|---|---|
| DOM interaction | Yes | Yes |
| Screenshots | Yes | Yes |
| Backend state access | No | **Yes** |
| IPC interception | No | **Yes** |
| Command registry introspection | No | **Yes** |
| Cross-boundary state verification | No | **Yes** |
| Ghost command detection | No | **Yes** |
| Multi-window targeting | Limited | **Yes** |
| Event recording & replay | No | **Yes** |

## Quick Start

**1. Add the plugin** (your Tauri app's `Cargo.toml`):

```toml
[dev-dependencies]
victauri-plugin = "0.1"
```

**2. Wire it up** (`src-tauri/src/main.rs`):

```rust
tauri::Builder::default()
    .plugin(victauri_plugin::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

In release builds, `init()` returns a no-op plugin — zero overhead, no conditional compilation needed.

**3. Connect your AI agent** (`.mcp.json` in your project root):

```json
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

That's it. Claude Code (or any MCP client) now has full-stack access to your running app.

## 55 MCP Tools

| Category | Tools |
|---|---|
| **WebView** (11) | `eval_js`, `dom_snapshot`, `click`, `double_click`, `hover`, `fill`, `type_text`, `press_key`, `select_option`, `scroll_to`, `focus_element` |
| **Windows** (7) | `get_window_state`, `list_windows`, `screenshot`, `manage_window`, `resize_window`, `move_window`, `set_window_title` |
| **Backend** (5) | `invoke_command`, `get_ipc_log`, `get_registry`, `get_memory_stats`, `get_console_logs` |
| **Network** (1) | `get_network_log` |
| **Storage** (4) | `get_storage`, `set_storage`, `delete_storage`, `get_cookies` |
| **Navigation** (3) | `get_navigation_log`, `navigate`, `navigate_back` |
| **Dialogs** (2) | `get_dialog_log`, `set_dialog_response` |
| **Verification** (3) | `verify_state`, `detect_ghost_commands`, `check_ipc_integrity` |
| **Streaming** (1) | `get_event_stream` |
| **Intent** (2) | `resolve_command`, `assert_semantic` |
| **Wait** (1) | `wait_for` |
| **Time-Travel** (7) | `start_recording`, `stop_recording`, `checkpoint`, `list_checkpoints`, `get_replay_sequence`, `get_recorded_events`, `events_between_checkpoints` |
| **CSS/Style** (4) | `get_styles`, `get_bounding_boxes`, `inject_css`, `remove_injected_css` |
| **Visual Debug** (2) | `highlight_element`, `clear_highlights` |
| **Accessibility** (1) | `audit_accessibility` |
| **Performance** (1) | `get_performance_metrics` |

## How It Works

```
MCP Client <-> HTTP/SSE on :7373 <-> Victauri Plugin (in-process)
                                        |-- WebView: DOM, click, type, eval JS
                                        |-- IPC: registry, invoke, intercept
                                        '-- Backend: state, memory, events
```

The MCP server runs inside the Tauri process. No external sidecar, no CDP, no WebDriver. Direct `AppHandle` access means sub-millisecond tool response times.

## Test Assertions

The `victauri-test` crate provides a typed HTTP client and assertion helpers for CI testing:

```rust
use victauri_test::{VictauriClient, assert_json_eq, assert_ipc_healthy};
use serde_json::json;

#[tokio::test]
async fn app_loads_correctly() {
    let mut client = VictauriClient::connect(7373).await.unwrap();

    let title = client.eval_js("document.title").await.unwrap();
    assert_eq!(title.as_str(), Some("My App"));

    let state = client.get_window_state(Some("main")).await.unwrap();
    assert_json_eq(&state, "/visible", &json!(true));

    let integrity = client.check_ipc_integrity().await.unwrap();
    assert_ipc_healthy(&integrity);
}
```

```toml
[dev-dependencies]
victauri-test = "0.1"
```

## Instrument Your Commands

```rust
use victauri_macros::inspectable;

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

```rust
VictauriBuilder::new()
    .generate_auth_token()  // prints token to logs on startup
    .build()
```

Or via environment variable: `VICTAURI_AUTH_TOKEN=my-secret-token`.

## Privacy Controls

- **Command allowlists/blocklists** — restrict which Tauri commands are invokable
- **Tool disabling** — hide specific MCP tools (e.g., `eval_js`, `screenshot`)
- **Output redaction** — automatic regex-based redaction of API keys, JWTs, emails, and sensitive JSON fields
- **Strict mode** — one-call preset that disables all mutating tools and enables full redaction

## Architecture

```
victauri/
├── crates/
│   ├── victauri-core/       # Types: events, registry, snapshots, verification
│   ├── victauri-macros/     # #[inspectable] proc macro
│   ├── victauri-plugin/     # Tauri plugin: MCP server + JS bridge
│   ├── victauri-test/       # Test client + assertion helpers
│   └── victauri-watchdog/   # Health-check sidecar
└── examples/
    └── demo-app/            # Minimal Tauri app with Victauri wired up
```

## Security Model

- Localhost only — no remote access, no forwarding
- DNS rebinding protection
- Auth tokens optional but recommended for shared machines
- Rate limiting (100 req/sec default, configurable)
- All tools compile away in release builds
- `eval_js` executes arbitrary JavaScript — treat it like browser devtools

## Development

```bash
cargo build                    # Build all crates
cargo test --workspace         # Run all tests (287)
cargo bench -p victauri-core   # Criterion benchmarks (13)
cargo clippy -- -D warnings    # Lint
cargo fmt --all -- --check     # Format check
```

## What It Doesn't Do

- **No production use** — gated behind `#[cfg(debug_assertions)]`, debug builds only
- **No remote access** — localhost only, by design
- **Early project** — API surface may change before 1.0

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

Built by [4DA Systems](https://4da.ai).
