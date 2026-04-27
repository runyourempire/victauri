# Example: AI Agent Session with Victauri

This shows a real Claude Code session using Victauri's MCP tools to inspect and test a Tauri app.

## Setup

Add the MCP server to your Claude Code config (`.mcp.json` or settings):

```json
{
  "mcpServers": {
    "my-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```

Start your Tauri app in debug mode, then connect Claude Code.

## Session Transcript

### 1. Discover Available Commands

```
Agent: Let me see what commands this app exposes.

→ get_registry()

Result: 12 commands registered
  - get_items (category: data, description: "Fetch all todo items")
  - add_item (category: data, description: "Add a new todo item")
  - delete_item (category: data, description: "Delete a todo item by ID")
  - save_api_key (category: settings, description: "Save API key for a provider")
  ...
```

### 2. Inspect the UI

```
Agent: What's currently rendered in the webview?

→ dom_snapshot(selector: "main", depth: 3)

Result: <main> with 2 children
  <div class="todo-list"> — 5 <li> items
  <div class="settings-panel" style="display:none">
```

### 3. Interact and Verify

```
Agent: I'll add a todo item through the UI and verify it hits the backend.

→ fill(selector: "#todo-input", value: "Write tests")
→ click(selector: "#add-button")
→ get_ipc_log(limit: 1)

Result: IPC call captured
  command: "add_item"
  args: {"text": "Write tests"}
  result: Ok({"id": 6, "text": "Write tests", "done": false})
  duration: 2ms
```

### 4. Cross-Boundary Verification

```
Agent: Let me verify the UI state matches what the backend returned.

→ verify_state(
    command: "get_items",
    js_expression: "document.querySelectorAll('.todo-item').length",
    assertion: { condition: "equals" }
  )

Result: ✓ Match — backend returns 6 items, DOM shows 6 .todo-item elements
```

### 5. Ghost Command Detection

```
Agent: Are there any commands registered but never called, or called but not registered?

→ detect_ghost_commands()

Result: 1 ghost found
  - "export_data" — registered in Rust but no frontend invoke() calls detected
```

### 6. Record and Replay

```
Agent: I'll record a user flow for regression testing.

→ start_recording()
→ click(selector: ".todo-item:first-child .checkbox")
→ checkpoint(label: "after-toggle")
→ click(selector: "#clear-completed")
→ checkpoint(label: "after-clear")
→ stop_recording()

→ events_between_checkpoints(from: "after-toggle", to: "after-clear")

Result: 3 events captured
  1. IPC: toggle_item({id: 1, done: true})
  2. DOM mutation: .todo-item:first-child class += "completed"
  3. IPC: clear_completed() → deleted 1 item
```

### 7. Performance Check

```
Agent: Any slow IPC calls?

→ slow_ipc_calls(threshold_ms: 100, limit: 5)

Result: 1 slow call
  - save_api_key: 340ms (file I/O on main thread)
```

## Key Patterns

| Pattern | Tools |
|---|---|
| **Smoke test** | `get_registry` → `invoke_command` → `verify_state` |
| **UI regression** | `start_recording` → interactions → `stop_recording` → `export_session` |
| **Ghost hunting** | `detect_ghost_commands` + `check_ipc_integrity` |
| **Performance audit** | `slow_ipc_calls` + `get_performance_metrics` |
| **Accessibility** | `audit_accessibility` |
| **Natural language** | `resolve_command("save the key")` → finds `save_api_key` |
