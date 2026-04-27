# victauri-core

Shared types and data structures for [Victauri](https://github.com/runyourempire/victauri) -- Verified Introspection & Control for Tauri Applications.

This crate has **no Tauri dependency** and can be used standalone for building tools that interact with Victauri's data model.

## Key Types

| Type | Description |
|---|---|
| `EventLog` | Thread-safe append-only ring buffer for `AppEvent` variants (Ipc, StateChange, DomMutation, WindowEvent) |
| `CommandRegistry` | Thread-safe `BTreeMap` with substring search and natural-language-to-command resolution |
| `DomSnapshot` / `DomElement` | Accessible DOM tree with ref handles (Playwright pattern) |
| `WindowState` | Window position, size, visibility, focus, and URL |
| `EventRecorder` | Time-travel recording with named checkpoints |
| `VerificationResult` / `Divergence` | Cross-boundary state verification output |
| `GhostCommandReport` | Detects frontend-invoked commands missing from the registry |
| `IpcIntegrityReport` | IPC health metrics (pending, stale, errored calls) |

## Example

```rust
use victauri_core::{EventLog, CommandRegistry, CommandInfo};

// Ring buffer with capacity 1000
let log = EventLog::new(1000);
log.push_ipc("greet".into(), serde_json::json!({"name": "world"}));
assert_eq!(log.len(), 1);

// Command registry with search
let mut registry = CommandRegistry::new();
registry.register(CommandInfo {
    name: "greet".into(),
    description: Some("Greet the user".into()),
    ..Default::default()
});
let results = registry.search("greet");
assert_eq!(results.len(), 1);
```

## Documentation

Full API docs: [docs.rs/victauri-core](https://docs.rs/victauri-core)

## License

Apache-2.0 -- see [LICENSE](../../LICENSE)

Part of [Victauri](https://github.com/runyourempire/victauri). Built by [4DA Systems](https://4da.ai).
