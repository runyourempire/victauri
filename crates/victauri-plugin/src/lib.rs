mod js_bridge;
mod mcp;
mod memory;
mod screenshot;
mod tools;

use std::sync::Arc;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};
use victauri_core::{CommandRegistry, EventLog};

pub use victauri_macros::inspectable;

const DEFAULT_PORT: u16 = 7373;
const DEFAULT_EVENT_CAPACITY: usize = 10_000;

pub struct VictauriState {
    pub event_log: EventLog,
    pub registry: CommandRegistry,
    pub port: u16,
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("victauri")
        .setup(|app, _api| {
            let event_log = EventLog::new(DEFAULT_EVENT_CAPACITY);
            let registry = CommandRegistry::new();

            let state = Arc::new(VictauriState {
                event_log,
                registry,
                port: DEFAULT_PORT,
            });

            app.manage(state.clone());

            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mcp::start_server(app_handle, state, DEFAULT_PORT).await {
                    tracing::error!("Victauri MCP server failed to start: {e}");
                }
            });

            tracing::info!("Victauri plugin initialized — MCP server on port {DEFAULT_PORT}");
            Ok(())
        })
        .on_webview_ready(|webview| {
            let label = webview.label().to_string();
            tracing::info!("Victauri: injecting JS bridge into webview '{label}'");

            if let Err(e) = webview.eval(js_bridge::INIT_SCRIPT) {
                tracing::error!("Victauri: failed to inject JS bridge into '{label}': {e}");
            }
        })
        .invoke_handler(tauri::generate_handler![
            tools::victauri_eval_js,
            tools::victauri_get_window_state,
            tools::victauri_list_windows,
            tools::victauri_get_ipc_log,
            tools::victauri_get_registry,
            tools::victauri_get_memory_stats,
            tools::victauri_dom_snapshot,
        ])
        .build()
}
