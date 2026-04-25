use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, Runtime, State};
use victauri_core::{IpcCall, WindowState};

use crate::VictauriState;

const EVAL_TIMEOUT: Duration = Duration::from_secs(10);

#[tauri::command]
pub async fn victauri_eval_js<R: Runtime>(
    webview: tauri::WebviewWindow<R>,
    state: State<'_, Arc<VictauriState>>,
    code: String,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    state.pending_evals.lock().await.insert(id.clone(), tx);

    let inject = format!(
        r#"
        (async () => {{
            try {{
                const __result = await (async () => {{ {code} }})();
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    id: '{id}',
                    result: JSON.stringify(__result)
                }});
            }} catch (e) {{
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    id: '{id}',
                    result: JSON.stringify({{ __error: e.message }})
                }});
            }}
        }})();
        "#
    );

    if let Err(e) = webview.eval(&inject) {
        state.pending_evals.lock().await.remove(&id);
        return Err(format!("eval failed: {e}"));
    }

    match tokio::time::timeout(EVAL_TIMEOUT, rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err("eval callback channel closed".to_string()),
        Err(_) => {
            state.pending_evals.lock().await.remove(&id);
            Err("eval timed out after 10s".to_string())
        }
    }
}

#[tauri::command]
pub async fn victauri_eval_callback(
    state: State<'_, Arc<VictauriState>>,
    id: String,
    result: String,
) -> Result<(), String> {
    if let Some(tx) = state.pending_evals.lock().await.remove(&id) {
        let _ = tx.send(result);
    }
    Ok(())
}

#[tauri::command]
pub async fn victauri_dom_snapshot<R: Runtime>(
    webview: tauri::WebviewWindow<R>,
    state: State<'_, Arc<VictauriState>>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    state.pending_evals.lock().await.insert(id.clone(), tx);

    let inject = format!(
        r#"
        (async () => {{
            try {{
                const snapshot = window.__VICTAURI__?.snapshot();
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    id: '{id}',
                    result: JSON.stringify(snapshot)
                }});
            }} catch (e) {{
                await window.__TAURI__.core.invoke('plugin:victauri|victauri_eval_callback', {{
                    id: '{id}',
                    result: JSON.stringify({{ __error: e.message }})
                }});
            }}
        }})();
        "#
    );

    if let Err(e) = webview.eval(&inject) {
        state.pending_evals.lock().await.remove(&id);
        return Err(format!("snapshot eval failed: {e}"));
    }

    match tokio::time::timeout(EVAL_TIMEOUT, rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err("snapshot callback channel closed".to_string()),
        Err(_) => {
            state.pending_evals.lock().await.remove(&id);
            Err("snapshot timed out after 10s".to_string())
        }
    }
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
pub async fn victauri_verify_state(
    _state: State<'_, Arc<VictauriState>>,
    frontend_state: serde_json::Value,
    backend_state: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let result = victauri_core::verify_state(frontend_state, backend_state);
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn victauri_detect_ghost_commands(
    state: State<'_, Arc<VictauriState>>,
) -> Result<serde_json::Value, String> {
    let ipc_calls = state.event_log.ipc_calls();
    let frontend_commands: Vec<String> = ipc_calls
        .iter()
        .map(|c| c.command.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let report = victauri_core::detect_ghost_commands(&frontend_commands, &state.registry);
    serde_json::to_value(report).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn victauri_check_ipc_integrity(
    state: State<'_, Arc<VictauriState>>,
    stale_threshold_ms: Option<i64>,
) -> Result<serde_json::Value, String> {
    let threshold = stale_threshold_ms.unwrap_or(5000);
    let report = victauri_core::check_ipc_integrity(&state.event_log, threshold);
    serde_json::to_value(report).map_err(|e| e.to_string())
}
