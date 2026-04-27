# Demo App

Minimal Tauri 2 application with Victauri wired up for testing and demonstration.

## Running

```bash
cd examples/demo-app
cargo tauri dev
```

The app starts with Victauri's MCP server on `127.0.0.1:7373`. Connect any MCP client to `http://127.0.0.1:7373/mcp`.

## Commands

All commands are decorated with `#[inspectable]` and appear in the command registry:

- `greet` — returns a greeting string
- `increment_counter` / `decrement_counter` / `get_counter` / `reset_counter` — counter CRUD
- `add_todo` / `get_todos` / `remove_todo` / `toggle_todo` — todo list
- `get_settings` — returns app settings
- `get_app_state` — dumps full application state

## MCP Configuration

The included `.mcp.json` allows Claude Code to connect immediately:

```json
{
  "mcpServers": {
    "demo-app": {
      "url": "http://127.0.0.1:7373/mcp"
    }
  }
}
```
