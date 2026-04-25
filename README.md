# Victauri

**Verified Introspection & Control for Tauri Applications**

X-ray vision and hands for AI agents inside Tauri apps.

---

Victauri is a Tauri 2.0 plugin that turns any Tauri application into an MCP-controllable target. AI agents get full-stack access — not just the webview, but the Rust backend, IPC layer, database, and native window state.

## Why Not Playwright?

Playwright gives agents eyes and hands **on the glass**. It sees the DOM, clicks buttons, fills forms. But for Tauri apps, the interesting stuff lives *behind* the glass:

| Capability | Playwright | Victauri |
|---|---|---|
| DOM interaction | Yes | Yes |
| Screenshots | Yes | Yes |
| Backend state access | No | **Yes** |
| IPC interception | No | **Yes** |
| Database queries | No | **Yes** |
| Command registry | No | **Yes** |
| Cross-boundary verification | No | **Yes** |
| Memory attribution | No | **Yes** |
| Works on macOS | Browser only | **Native** |

Victauri doesn't replace Playwright for web testing. It does what Playwright structurally cannot do for desktop applications.

## Quick Start

Add to your Tauri app's `Cargo.toml`:

```toml
[dependencies]
victauri-plugin = "0.1"
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
// .mcp.json
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

## How It Works

Victauri runs **inside** your Tauri app process. No external process, no socket bridge, no CDP dependency.

```
Claude Code ←→ HTTP/SSE on :7373 ←→ Victauri Plugin (same process as your app)
                                          ├── WebView: DOM snapshots, click, type, eval JS
                                          ├── IPC: command registry, invoke, intercept log
                                          └── Backend: state reading, DB queries, memory tracking
```

The plugin is gated behind `#[cfg(debug_assertions)]` — it compiles away completely in release builds.

## Instrument Your Commands

```rust
use victauri_plugin::inspectable;

#[tauri::command]
#[inspectable(description = "Save API key for a provider")]
async fn save_api_key(provider: String, key: String) -> Result<(), String> {
    // your code
}
```

The `#[inspectable]` macro auto-generates a JSON schema for the command, making it discoverable by AI agents through the command registry.

## Architecture

Four crates in a Rust workspace:

- **victauri-core** — Shared types (events, registry, snapshots, verification)
- **victauri-macros** — `#[inspectable]` proc macro for command instrumentation
- **victauri-plugin** — Tauri plugin with embedded MCP server
- **victauri-watchdog** — Lightweight crash-recovery sidecar

## License

Apache-2.0
