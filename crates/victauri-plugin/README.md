# victauri-plugin

The main [Victauri](https://github.com/runyourempire/victauri) crate -- an embedded MCP server that gives AI agents full-stack control of Tauri 2.0 applications.

## Quick Start

Add the dependency (dev-only):

```toml
[dev-dependencies]
victauri-plugin = "0.1"
```

Wire it into your Tauri app:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(victauri_plugin::init())
        .run(tauri::generate_context!())
        .expect("error running app");
}
```

Connect Claude Code (or any MCP client) to `http://127.0.0.1:7373/mcp`.

## Configuration

Use `VictauriBuilder` for advanced setup:

```rust,ignore
victauri_plugin::VictauriBuilder::new()
    .port(8080)
    .auth_token("my-secret-token")
    .disable_tools(&["screenshot"])
    .add_redaction_pattern(r"\b\d{3}-\d{2}-\d{4}\b")
    .enable_redaction()
    .build()
    .expect("valid config")
```

## Tools

55 MCP tools across 17 categories:

| Category | Tools | Examples |
|---|---|---|
| WebView | 11 | eval_js, dom_snapshot, click, fill, type_text, press_key |
| Windows | 7 | get_window_state, list_windows, screenshot, manage_window |
| Backend | 5 | invoke_command, get_ipc_log, get_registry, get_memory_stats |
| Verification | 3 | verify_state, detect_ghost_commands, check_ipc_integrity |
| Time-Travel | 7 | start_recording, checkpoint, get_replay_sequence |
| CSS/Style | 4 | get_styles, get_bounding_boxes, inject_css |
| Accessibility | 1 | audit_accessibility (WCAG checks) |
| Performance | 1 | get_performance_metrics (timing, heap, resources) |
| + 9 more | 16 | storage, navigation, dialogs, network, wait_for, ... |

## Security

- **Localhost only** -- binds to `127.0.0.1`, never exposed to network
- **Debug-only** -- `init()` returns a no-op plugin in release builds
- **Optional auth** -- Bearer token via builder or `VICTAURI_AUTH_TOKEN` env var
- **Rate limiting** -- Token-bucket at 100 req/sec (default)
- **Privacy layer** -- Command allowlists/blocklists, tool disabling, regex redaction

See the full [project README](https://github.com/runyourempire/victauri) for architecture details and live test results.

## Documentation

Full API docs: [docs.rs/victauri-plugin](https://docs.rs/victauri-plugin)

## License

Apache-2.0 -- see [LICENSE](../../LICENSE)

Part of [Victauri](https://github.com/runyourempire/victauri). Built by [4DA Systems](https://4da.ai).
