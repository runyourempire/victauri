use std::sync::Arc;
use tauri::{Manager, Runtime, State};
use victauri_core::{EventLog, IpcCall, WindowState};

use crate::VictauriState;

#[tauri::command]
pub async fn victauri_eval_js<R: Runtime>(
    webview: tauri::WebviewWindow<R>,
    code: String,
) -> Result<String, String> {
    webview.eval(&format!(
        r#"
        (async () => {{
            try {{
                const __result = await (async () => {{ {code} }})();
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    result: JSON.stringify(__result)
                }});
            }} catch (e) {{
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    result: JSON.stringify({{ __error: e.message }})
                }});
            }}
        }})();
        "#
    )).map_err(|e| format!("eval failed: {e}"))?;

    // TODO: Wire up oneshot channel for eval result callback
    // For now, fire-and-forget with acknowledgment
    Ok("eval dispatched".to_string())
}

#[tauri::command]
pub async fn victauri_get_window_state<R: Runtime>(
    app: tauri::AppHandle<R>,
    label: Option<String>,
) -> Result<Vec<WindowState>, String> {
    let windows = app.webview_windows();
    let mut states = Vec::new();

    for (win_label, window) in &windows {
        if let Some(ref filter) = label {
            if win_label != filter {
                continue;
            }
        }

        let pos = window.outer_position().unwrap_or_default();
        let size = window.inner_size().unwrap_or_default();

        states.push(WindowState {
            label: win_label.clone(),
            title: window.title().unwrap_or_default(),
            url: window.url().map(|u| u.to_string()).unwrap_or_default(),
            visible: window.is_visible().unwrap_or(false),
            focused: window.is_focused().unwrap_or(false),
            maximized: window.is_maximized().unwrap_or(false),
            minimized: window.is_minimized().unwrap_or(false),
            fullscreen: window.is_fullscreen().unwrap_or(false),
            position: (pos.x, pos.y),
            size: (size.width, size.height),
        });
    }

    Ok(states)
}

#[tauri::command]
pub async fn victauri_list_windows<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(app.webview_windows().keys().cloned().collect())
}

#[tauri::command]
pub async fn victauri_get_ipc_log(
    state: State<'_, Arc<VictauriState>>,
    limit: Option<usize>,
) -> Result<Vec<IpcCall>, String> {
    let mut calls = state.event_log.ipc_calls();
    if let Some(limit) = limit {
        let start = calls.len().saturating_sub(limit);
        calls = calls[start..].to_vec();
    }
    Ok(calls)
}

#[tauri::command]
pub async fn victauri_get_registry(
    state: State<'_, Arc<VictauriState>>,
    query: Option<String>,
) -> Result<serde_json::Value, String> {
    let commands = match query {
        Some(q) => state.registry.search(&q),
        None => state.registry.list(),
    };
    serde_json::to_value(commands).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn victauri_get_memory_stats() -> Result<serde_json::Value, String> {
    Ok(crate::memory::current_stats())
}

#[tauri::command]
pub async fn victauri_dom_snapshot<R: Runtime>(
    webview: tauri::WebviewWindow<R>,
) -> Result<String, String> {
    webview
        .eval("window.__VICTAURI__?.snapshot()")
        .map_err(|e| format!("snapshot eval failed: {e}"))?;

    // TODO: Wire up callback to receive snapshot result
    Ok("snapshot dispatched".to_string())
}
