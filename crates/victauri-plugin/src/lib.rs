pub mod bridge;
mod js_bridge;
pub mod mcp;
mod memory;
mod screenshot;
mod tools;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};
use tokio::sync::{Mutex, oneshot};
use victauri_core::{CommandRegistry, EventLog, EventRecorder};

pub use victauri_macros::inspectable;

const DEFAULT_PORT: u16 = 7373;
const DEFAULT_EVENT_CAPACITY: usize = 10_000;

pub type PendingCallbacks = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub struct VictauriState {
    pub event_log: EventLog,
    pub registry: CommandRegistry,
    pub port: u16,
    pub pending_evals: PendingCallbacks,
    pub recorder: EventRecorder,
}

/// Initialize the Victauri plugin.
///
/// In debug builds: starts the embedded MCP server, injects the JS bridge, and
/// registers all Tauri command handlers.
///
/// In release builds: returns a no-op plugin. The MCP server, JS bridge, and
/// all introspection tools are completely stripped — zero overhead, zero attack surface.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    #[cfg(not(debug_assertions))]
    {
        Builder::new("victauri").build()
    }

    #[cfg(debug_assertions)]
    {
        Builder::new("victauri")
            .setup(|app, _api| {
                let event_log = EventLog::new(DEFAULT_EVENT_CAPACITY);
                let registry = CommandRegistry::new();

                let state = Arc::new(VictauriState {
                    event_log,
                    registry,
                    port: DEFAULT_PORT,
                    pending_evals: Arc::new(Mutex::new(HashMap::new())),
                    recorder: EventRecorder::new(50_000),
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
                tools::victauri_eval_callback,
                tools::victauri_get_window_state,
                tools::victauri_list_windows,
                tools::victauri_get_ipc_log,
                tools::victauri_get_registry,
                tools::victauri_get_memory_stats,
                tools::victauri_dom_snapshot,
                tools::victauri_verify_state,
                tools::victauri_detect_ghost_commands,
                tools::victauri_check_ipc_integrity,
            ])
            .build()
    }
}
