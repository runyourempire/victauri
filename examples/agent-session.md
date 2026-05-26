# Example: AI Agent Session with Victauri

A real Claude Code session using Victauri's MCP tools to inspect and test a Tauri app.

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

→ dom_snapshot()

Result (compact accessible-text format):
  [e0] main
    [e1] div.todo-list
      [e2] listitem "Buy groceries"
      [e3] listitem "Write tests"
      [e4] listitem "Deploy v2"
    [e5] div.settings-panel [hidden]

Note: ref handles (e0-e5) are stable across calls.
Use format: "json" for full accessibility tree.
```

### 3. Find Specific Elements

```
Agent: I need the add button without scanning the full tree.

→ find_elements(text: "Add", role: "button")

Result: 1 match
  [e12] button "Add Todo" @add-btn
```

### 4. Interact and Verify

```
Agent: I'll add a todo item through the UI and verify it hits the backend.

→ input(action: "fill", ref_id: "e10", value: "Write tests")
→ interact(action: "click", ref_id: "e12")
→ logs(action: "ipc", limit: 1)

Result: IPC call captured
  command: "add_item"
  request_args: {"text": "Write tests"}
  response_body: {"id": 6, "text": "Write tests", "done": false}
  duration: 2ms
```

### 5. Cross-Boundary Verification

```
Agent: Let me verify the UI state matches what the backend returned.

→ verify_state(
    command: "get_items",
    frontend_expr: "document.querySelectorAll('.todo-item').length",
    backend_state: {"key": "items_count"}
  )

Result: ✓ Match — backend returns 6 items, DOM shows 6 .todo-item elements
```

### 6. Ghost Command Detection

```
Agent: Are there any commands registered but never called, or called but not registered?

→ detect_ghost_commands()

Result: 1 ghost found
  - "export_data" — registered in Rust but no frontend invoke() calls detected
```

### 7. Record and Replay

```
Agent: I'll record a user flow for regression testing.

→ recording(action: "start", session_name: "toggle-flow")
→ interact(action: "click", ref_id: "e2")
→ recording(action: "checkpoint", checkpoint_id: "after-toggle")
→ interact(action: "click", ref_id: "e15")
→ recording(action: "checkpoint", checkpoint_id: "after-clear")
→ recording(action: "stop")

→ recording(action: "events_between", from_checkpoint: "after-toggle", to_checkpoint: "after-clear")

Result: 3 events captured
  1. IPC: toggle_item({id: 1, done: true})
  2. DOM mutation: .todo-item:first-child class += "completed"
  3. IPC: clear_completed() → deleted 1 item
```

### 8. Performance & Accessibility

```
Agent: Any slow IPC calls or accessibility issues?

→ logs(action: "slow_ipc", threshold_ms: 100)

Result: 1 slow call
  - save_api_key: 340ms (file I/O on main thread)

→ inspect(action: "audit_accessibility")

Result: 2 violations
  - missing aria-label on icon button (#settings-btn)
  - insufficient color contrast on .muted-text (3.2:1, needs 4.5:1)
```

## Key Patterns

| Pattern | Tools |
|---|---|
| **Smoke test** | `get_registry` → `invoke_command` → `verify_state` |
| **UI regression** | `recording(start)` → interactions → `recording(stop/export)` |
| **Ghost hunting** | `detect_ghost_commands` + `check_ipc_integrity` |
| **Performance audit** | `logs(slow_ipc)` + `inspect(get_performance)` |
| **Accessibility** | `inspect(audit_accessibility)` |
| **Element search** | `find_elements(text: "Save")` → `interact(click, ref_id)` |
| **Natural language** | `resolve_command("save the key")` → finds `save_api_key` |
| **Window inspection** | `window(list)` → `window(get_state, label: "main")` |

## Tool Reference (31 tools)

**19 standalone:** `eval_js`, `dom_snapshot`, `find_elements`, `invoke_command`, `screenshot`, `verify_state`, `detect_ghost_commands`, `check_ipc_integrity`, `wait_for`, `assert_semantic`, `resolve_command`, `get_registry`, `get_memory_stats`, `get_plugin_info`, `get_diagnostics`, `app_info`, `list_app_dir`, `read_app_file`, `query_db`

**12 compound** (pass `action` to select behavior):
- `interact` — click, double_click, hover, focus, scroll_into_view, select_option
- `input` — fill, type_text, press_key
- `window` — get_state, list, manage, resize, move_to, set_title
- `storage` — get, set, delete, get_cookies
- `navigate` — go_to, go_back, get_history, set_dialog_response, get_dialog_log
- `recording` — start, stop, checkpoint, list_checkpoints, get_events, events_between, get_replay, export, import, replay
- `inspect` — get_styles, get_bounding_boxes, highlight, clear_highlights, audit_accessibility, get_performance
- `css` — inject, remove
- `logs` — console, network, ipc, navigation, dialogs, events, slow_ipc
- `introspect` — command_timings, coverage, contract_record, contract_check, contract_list, contract_clear, startup_timing, capabilities, db_health, managed_state, processes, tasks, fs_scope, event_bus, event_bus_clear
- `fault` — inject, list, clear, clear_all
- `explain` — summary, last_action, diff
