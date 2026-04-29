# Victauri

**Full-stack testing for Tauri apps. Click a button in the frontend, verify the Rust handler ran, confirm the database row was written — from one test.**

[![CI](https://github.com/runyourempire/victauri/actions/workflows/ci.yml/badge.svg)](https://github.com/runyourempire/victauri/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/victauri-plugin.svg)](https://crates.io/crates/victauri-plugin)
[![docs.rs](https://docs.rs/victauri-plugin/badge.svg)](https://docs.rs/victauri-plugin)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

Testing Tauri apps today means choosing between frontend mocks that lie about your backend, WebDriver setups that take a weekend, or paying for macOS support. Victauri embeds an [MCP](https://modelcontextprotocol.io) server directly inside your Tauri process — giving test suites and AI agents simultaneous access to the DOM, IPC layer, Rust backend state, and native windows. No WebDriver binary. No browser dependency. **Works on macOS, Windows, and Linux for free.**

## What makes this different

Every other Tauri testing tool stops at the glass. They can click buttons and read text, but they can't tell you whether the Rust command handler actually executed, what arguments it received, or whether the result made it back to the frontend intact.

Victauri crosses the boundary:

```rust
// Click a button in the frontend
client.interact("click", "e5").await?;

// Verify the Rust command ran with correct args
let ipc = client.logs("ipc", Some(1)).await?;
assert_eq!(ipc[0]["command"], "save_settings");
assert_eq!(ipc[0]["args"]["theme"], "dark");

// Verify frontend state matches backend
let result = client.verify_state(
    "document.querySelector('.theme-label').textContent",
    json!({"theme": "dark"})
).await?;
assert!(result["divergences"].as_array().unwrap().is_empty());
```

| | Playwright | WebdriverIO + tauri-driver | tauri-plugin-mcp | mcp-server-tauri | **Victauri** |
|---|---|---|---|---|---|
| DOM interaction | Yes | Yes | Yes | Yes | **Yes** |
| macOS support | No (no CDP) | No (no WKWebView driver) | Yes | Yes | **Yes** |
| Backend state access | No | No | No | No | **Yes** |
| IPC call verification (args + result) | No | No | No | Partial | **Yes** |
| Ghost command detection | No | No | No | No | **Yes** |
| Cross-boundary verification | No | No | No | No | **Yes** |
| `#[inspectable]` command schemas | No | No | No | No | **Yes** |
| Time-travel recording | No | No | No | No | **Yes** |
| Zero-config setup | No | No | Near | Near | **Yes** |
| Works with `cargo test` | N/A | Limited | No | No | **Yes** |

## Quick Start

**1. Add the plugin:**

```toml
# Cargo.toml
[dev-dependencies]
victauri-plugin = "0.1"
```

**2. Wire it up** (one line in your app):

```rust
// src-tauri/src/main.rs
tauri::Builder::default()
    .plugin(victauri_plugin::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

In release builds, `init()` returns a no-op plugin — zero overhead, no feature flags needed.

**3. Connect your agent or test runner:**

For AI agents (Claude Code, Cursor, Windsurf) — add `.mcp.json` to your project:

```json
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

For `cargo test` — use the test client:

```rust
use victauri_test::VictauriClient;

#[tokio::test]
async fn settings_persist() {
    let mut client = VictauriClient::connect(7373).await.unwrap();
    
    // Invoke backend command directly
    let result = client.invoke_command("save_settings", Some(json!({"theme": "dark"}))).await.unwrap();
    assert_eq!(result, json!({"ok": true}));
    
    // Verify the UI updated
    client.wait_for("text", Some("Dark mode"), None, None).await.unwrap();
    
    // Check no ghost commands exist
    let ghosts = client.detect_ghost_commands().await.unwrap();
    assert!(ghosts["ghost_invocations"].as_array().unwrap().is_empty());
}
```

## MCP Tools

Victauri exposes ~20 focused MCP tools across 9 compound tools and 13 standalone tools:

| Tool | What it does |
|---|---|
| **`interact`** | Click, double-click, hover, focus, scroll, select — with auto-wait for actionability |
| **`input`** | Fill inputs, type character-by-character, press keyboard keys |
| **`window`** | Get state, list windows, manage (minimize/maximize/etc), resize, move, set title |
| **`storage`** | Read/write localStorage, sessionStorage, cookies |
| **`navigate`** | Go to URL, go back, get history, configure dialog auto-responses |
| **`recording`** | Start/stop sessions, create checkpoints, get events, export/import for replay |
| **`inspect`** | Computed CSS, bounding boxes, element highlighting, accessibility audit, performance metrics |
| **`logs`** | Console, network, IPC, navigation, dialog logs — filtered by time and content |
| **`css`** | Inject/remove debug CSS |
| `eval_js` | Execute JavaScript in the webview (async supported) |
| `dom_snapshot` | Full accessibility tree with ref handles for interaction |
| `invoke_command` | Call any registered Tauri command through real IPC |
| `screenshot` | Platform-native window capture (no Chromium dependency) |
| `verify_state` | Compare frontend DOM state against backend state — find divergences |
| `detect_ghost_commands` | Find frontend IPC calls with no backend handler (and vice versa) |
| `check_ipc_integrity` | Detect stuck, stale, or errored IPC calls |
| `wait_for` | Poll for conditions: text appears, selector matches, IPC settles |
| `assert_semantic` | Evaluate JS expression and assert against expected value |
| `resolve_command` | Natural language → matching Tauri command |
| `get_registry` | List all commands with schemas from `#[inspectable]` |

## Instrument Your Commands

```rust
use victauri_macros::inspectable;

#[tauri::command]
#[inspectable(
    description = "Save user preferences",
    intent = "persist settings",
    category = "settings",
    example = "save the user's theme preference"
)]
async fn save_settings(settings: Settings) -> Result<(), AppError> {
    // your code
}
```

`#[inspectable]` generates a command schema at compile time — zero runtime cost. Commands become discoverable through `get_registry` and natural language via `resolve_command`.

## How It Works

```
AI Agent / cargo test
        |
        v
  HTTP on :7373  (MCP protocol)
        |
        v
  Victauri Plugin  (inside Tauri process)
     |       |       |
     v       v       v
  WebView  IPC    Backend
  - DOM    - log   - state
  - click  - args  - memory
  - eval   - result - registry
```

The MCP server runs **inside** the Tauri process — not as a sidecar. Direct `AppHandle` access means sub-millisecond response times and zero state drift between what the tool sees and what the app does.

## Security & Privacy

Victauri is designed for development, not production:

- **Debug-only**: entire plugin compiles away in release builds (`#[cfg(debug_assertions)]`)
- **Localhost-only**: no remote access, DNS rebinding protection
- **Auth tokens**: auto-generated or configurable via `VICTAURI_AUTH_TOKEN`
- **Privacy controls**: command allowlists/blocklists, tool disabling, output redaction (API keys, JWTs, emails)
- **Strict mode**: one call to disable all mutating tools

```rust
VictauriBuilder::new()
    .strict_privacy_mode()   // disable eval_js, fill, type_text, navigate, etc.
    .auth_token("my-token")
    .build()
```

## Architecture

```
victauri/
├── crates/
│   ├── victauri-core/       # Shared types (events, registry, snapshots, verification)
│   ├── victauri-macros/     # #[inspectable] proc macro
│   ├── victauri-plugin/     # Tauri plugin + MCP server + JS bridge
│   ├── victauri-test/       # Typed HTTP client + assertion helpers
│   └── victauri-watchdog/   # Health-check sidecar
└── examples/
    └── demo-app/            # Minimal Tauri app with 12 instrumented commands
```

## What It Doesn't Do

- **No production use** — debug builds only, by design
- **No remote access** — localhost, no port forwarding
- **No iframe support** — single-frame webviews only (Tauri standard)
- **Pre-1.0** — API may change. Semver-checked in CI.

## Development

```bash
cargo build                    # Build all crates
cargo test --workspace         # Run all tests
cargo bench -p victauri-core   # Criterion benchmarks
cargo clippy -- -D warnings    # Lint (zero warnings policy)
cargo fmt --all -- --check     # Format
```

## License

Apache-2.0 — [LICENSE](LICENSE)

Built and maintained by [4DA Systems](https://4da.ai).
