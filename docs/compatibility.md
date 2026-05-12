# Tauri App Compatibility

Victauri works with any Tauri 2.x application. This page documents known compatibility considerations based on testing against real-world open-source Tauri apps.

## Content Security Policy (CSP)

Victauri's webview tools (`eval_js`, `dom_snapshot`, `click`, `fill`, `type_text`, etc.) require JavaScript execution in the webview. If your app sets a strict CSP without `'unsafe-eval'`, these tools will fail.

### Check your CSP

Look in `tauri.conf.json` for the `security.csp` field:

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; script-src 'self'"
    }
  }
}
```

### What works with strict CSP

These tools work regardless of CSP because they don't execute JavaScript in the webview:

| Tool | Why it works |
|---|---|
| `list_windows` | Reads Tauri window manager directly |
| `get_window_state` | Reads Tauri window properties directly |
| `screenshot` | Uses native OS capture (PrintWindow / CGWindowListCreateImage / X11) |
| `invoke_command` | Calls Tauri commands via IPC directly |
| `get_registry` | Reads the in-process command registry |
| `get_memory_stats` | Reads OS process memory |
| `get_plugin_info` | Reads plugin state |

### What requires JavaScript execution

These tools need the JS bridge and will fail under strict CSP:

- `eval_js`, `dom_snapshot`, `find_elements`, `click`, `fill`, `type_text`, `press_key`
- `get_styles`, `get_bounding_boxes`, `highlight_element`, `audit_accessibility`
- `get_performance_metrics`, `get_console_logs`, `get_ipc_log`
- `detect_ghost_commands`, `check_ipc_integrity`, `verify_state`
- `start_recording`, `get_event_stream`

### Recommended CSP for development

Since Victauri only runs in debug builds (`#[cfg(debug_assertions)]`), you can relax CSP in development without affecting production:

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'unsafe-eval'"
    }
  }
}
```

Or disable CSP entirely for development (Victauri's server is localhost-only):

```json
{
  "app": {
    "security": {
      "csp": null
    }
  }
}
```

Apps with `"csp": null` (like Vibe and Hoppscotch Agent) have zero friction with Victauri.

## IPC Patterns

### Standard Tauri commands (full support)

Apps using `#[tauri::command]` + `tauri::generate_handler!` are fully supported. All IPC tools work: `get_ipc_log`, `invoke_command`, `detect_ghost_commands`, `check_ipc_integrity`, `get_registry`.

### rspc / tauri-specta (partial support)

Some apps (like Spacedrive) use alternative RPC layers that route through a single Tauri command. Victauri's IPC log will show the wrapper command but not the inner procedure names. `get_registry` will return empty unless commands are also registered with `#[inspectable]`.

### Sidecar processes (invisible to Victauri)

Apps that use Node.js sidecars (Yaak) or separate daemon processes (Spacedrive) for backend logic — Victauri only sees the Tauri IPC boundary. Sidecar traffic is invisible and not captured.

## Multi-Window Apps

Victauri handles multi-window apps automatically. The default window selection prefers `"main"` → first visible → any. Use the `webview_label` parameter on webview tools to target specific windows:

```rust
client.eval_js_in("settings", "document.title").await?;
client.dom_snapshot_for("notification").await?;
```

Apps with many dynamic windows (like Spacedrive's 15+ window types or Seelen-UI's desktop environment) should target windows by label explicitly.

## Tested Apps

| App | Stars | Tauri | Frontend | CSP | Windows | Victauri Fit |
|---|---|---|---|---|---|---|
| [Vibe](https://github.com/thewh1teagle/vibe) | 6.1k | 2.x | TypeScript | None | 1 | Excellent |
| [Bokuchi](https://github.com/Bokuchi-Editor/bokuchi) | 68 | 2.x | React | Low risk | 1 | Excellent |
| [Yaak](https://github.com/mountain-loop/yaak) | 18.6k | 2.11 | React 19 | Unknown | 1 | Good |
| [Whispering](https://github.com/EpicenterHQ/epicenter) | 4.5k | 2.x | Svelte 5 | Unknown | 1 | Good |
| [Hoppscotch Agent](https://github.com/hoppscotch/hoppscotch) | 79k | 2.9 | Minimal | None | 1 | Too minimal (2 commands) |
| [Spacedrive](https://github.com/spacedriveapp/spacedrive) | 38k | 2.1 | React + rspc | Strict | 15+ | Hard (CSP + rspc) |
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | 16.8k | 2.10 | Svelte 5 | Unknown | Many | Hard (desktop env) |

## Framework Coverage

Victauri's JS bridge works with any frontend framework. The DOM snapshot uses the accessible tree (ARIA roles and names), which is framework-agnostic. Confirmed working with:

- React (demo-app, 4DA)
- Vue (Hoppscotch web client uses Vue 3.5)
- Svelte (Whispering uses Svelte 5)
- Vanilla HTML/JS
