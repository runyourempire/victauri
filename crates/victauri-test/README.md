# victauri-test

Test assertion helpers for AI-agent and CI testing of Tauri apps via [Victauri](https://github.com/runyourempire/victauri).

## What It Does

Provides a typed HTTP client for the Victauri MCP server plus assertion helpers for common test patterns:

- **DOM checks** — JSON pointer assertions on snapshots
- **IPC verification** — integrity and ghost command detection
- **State comparison** — cross-boundary frontend/backend verification
- **Accessibility audits** — WCAG violation assertions
- **Performance budgets** — load time and heap size guards

## Quick Start

Add to your test dependencies:

```toml
[dev-dependencies]
victauri-test = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Write a test:

```rust
use victauri_test::{VictauriClient, assert_json_eq, assert_ipc_healthy};
use serde_json::json;

#[tokio::test]
async fn app_loads_correctly() {
    let mut client = VictauriClient::connect(7373).await.unwrap();

    // Evaluate JS in the webview
    let title = client.eval_js("document.title").await.unwrap();
    assert_eq!(title.as_str(), Some("My App"));

    // Check window state
    let state = client.get_window_state(Some("main")).await.unwrap();
    assert_json_eq(&state, "/visible", &json!(true));

    // Verify IPC health
    let integrity = client.check_ipc_integrity().await.unwrap();
    assert_ipc_healthy(&integrity);
}
```

## Authentication

If the Victauri server requires a Bearer token:

```rust
let mut client = VictauriClient::connect_with_token(7373, Some("my-secret")).await.unwrap();
```

## Assertion Helpers

| Function | What It Checks |
|---|---|
| `assert_json_eq(value, pointer, expected)` | JSON pointer equals expected value |
| `assert_json_truthy(value, pointer)` | JSON pointer is truthy (not null/false/0/"") |
| `assert_no_a11y_violations(audit)` | Accessibility audit has zero violations |
| `assert_performance_budget(metrics, max_load_ms, max_heap_mb)` | Load time and heap within budget |
| `assert_ipc_healthy(integrity)` | No stale or errored IPC calls |
| `assert_state_matches(verification)` | Frontend/backend state verification passed |

## License

Apache-2.0
