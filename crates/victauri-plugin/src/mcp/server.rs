use std::sync::Arc;
use std::sync::atomic::Ordering;

use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tauri::Runtime;

use crate::VictauriState;
use crate::bridge::WebviewBridge;

use super::{MAX_PENDING_EVALS, VictauriMcpHandler};

// ── Server startup ───────────────────────────────────────────────────────────

/// Build an Axum router for the MCP server with default options (no auth token).
pub fn build_app(state: Arc<VictauriState>, bridge: Arc<dyn WebviewBridge>) -> axum::Router {
    build_app_with_options(state, bridge, None)
}

/// Build an Axum router for the MCP server with an optional auth token.
pub fn build_app_with_options(
    state: Arc<VictauriState>,
    bridge: Arc<dyn WebviewBridge>,
    auth_token: Option<String>,
) -> axum::Router {
    let handler = VictauriMcpHandler::new(state.clone(), bridge);

    let mcp_service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let auth_state = Arc::new(crate::auth::AuthState {
        token: auth_token.clone(),
    });
    let health_state = state.clone();
    let info_state = state.clone();
    let info_auth = auth_token.is_some();

    let privacy_enabled = !state.privacy.disabled_tools.is_empty()
        || state.privacy.command_allowlist.is_some()
        || !state.privacy.command_blocklist.is_empty()
        || state.privacy.redaction_enabled;

    let mut router = axum::Router::new()
        .route_service("/mcp", mcp_service)
        .route(
            "/info",
            axum::routing::get(move || {
                let s = info_state.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "name": "victauri",
                        "version": env!("CARGO_PKG_VERSION"),
                        "protocol": "mcp",
                        "commands_registered": s.registry.count(),
                        "events_captured": s.event_log.len(),
                        "port": s.port.load(Ordering::Relaxed),
                        "auth_required": info_auth,
                        "privacy_mode": privacy_enabled,
                    }))
                }
            }),
        );

    if auth_token.is_some() {
        router = router.layer(axum::middleware::from_fn_with_state(
            auth_state,
            crate::auth::require_auth,
        ));
    }

    let rate_limiter = crate::auth::default_rate_limiter();
    router = router.layer(axum::middleware::from_fn_with_state(
        rate_limiter,
        crate::auth::rate_limit,
    ));

    router
        .route(
            "/health",
            axum::routing::get(move || {
                let s = health_state.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "status": "ok",
                        "uptime_secs": s.started_at.elapsed().as_secs(),
                        "events_captured": s.event_log.len(),
                        "commands_registered": s.registry.count(),
                        "memory": crate::memory::current_stats(),
                    }))
                }
            }),
        )
        .layer(axum::middleware::from_fn(crate::auth::security_headers))
        .layer(axum::middleware::from_fn(crate::auth::origin_guard))
        .layer(axum::middleware::from_fn(crate::auth::dns_rebinding_guard))
}

#[doc(hidden)]
pub mod tests_support {
    pub fn get_memory_stats() -> serde_json::Value {
        crate::memory::current_stats()
    }
}

const PORT_FALLBACK_RANGE: u16 = 10;

/// Start the MCP server on the given port with default options (no auth token).
///
/// # Errors
///
/// Returns an error if the server fails to bind to the requested port (or any port in the
/// fallback range), or if the server exits unexpectedly.
pub async fn start_server<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    start_server_with_options(app_handle, state, port, None, shutdown_rx).await
}

/// Start the MCP server on the given port with an optional auth token.
///
/// # Errors
///
/// Returns an error if the server fails to bind to the requested port (or any port in the
/// fallback range), or if the server exits unexpectedly.
pub async fn start_server_with_options<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
    auth_token: Option<String>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let bridge: Arc<dyn WebviewBridge> = Arc::new(app_handle);
    let token_for_file = auth_token.clone();
    let app = build_app_with_options(state.clone(), bridge.clone(), auth_token);

    let (listener, actual_port) = try_bind(port).await?;

    if actual_port != port {
        tracing::warn!("Victauri: port {port} in use, fell back to {actual_port}");
    }

    state.port.store(actual_port, Ordering::Relaxed);
    write_port_file(actual_port);
    if let Some(ref token) = token_for_file {
        write_token_file(token);
    }

    tracing::info!("Victauri MCP server listening on 127.0.0.1:{actual_port}");

    let drain_state = state.clone();
    let drain_bridge = bridge;
    let drain_shutdown = state.shutdown_tx.subscribe();
    tokio::spawn(event_drain_loop(drain_state, drain_bridge, drain_shutdown));

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.wait_for(|&v| v).await;
            remove_port_file();
            tracing::info!("Victauri MCP server shutting down gracefully");
        })
        .await?;
    Ok(())
}

async fn try_bind(preferred: u16) -> anyhow::Result<(tokio::net::TcpListener, u16)> {
    if let Ok(listener) = tokio::net::TcpListener::bind(format!("127.0.0.1:{preferred}")).await {
        return Ok((listener, preferred));
    }

    for offset in 1..=PORT_FALLBACK_RANGE {
        let port = preferred + offset;
        if let Ok(listener) = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
            return Ok((listener, port));
        }
    }

    anyhow::bail!(
        "could not bind to any port in range {preferred}-{}",
        preferred + PORT_FALLBACK_RANGE
    )
}

fn port_file_path() -> std::path::PathBuf {
    std::env::temp_dir().join("victauri.port")
}

fn token_file_path() -> std::path::PathBuf {
    std::env::temp_dir().join("victauri.token")
}

fn write_port_file(port: u16) {
    if let Err(e) = std::fs::write(port_file_path(), port.to_string()) {
        tracing::debug!("could not write port file: {e}");
    }
}

fn write_token_file(token: &str) {
    if let Err(e) = std::fs::write(token_file_path(), token) {
        tracing::debug!("could not write token file: {e}");
    }
}

fn remove_port_file() {
    let _ = std::fs::remove_file(port_file_path());
    let _ = std::fs::remove_file(token_file_path());
}

async fn event_drain_loop(
    state: Arc<VictauriState>,
    bridge: Arc<dyn WebviewBridge>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    use chrono::Utc;
    use victauri_core::AppEvent;

    let mut last_drain_ts: f64 = 0.0;

    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            _ = shutdown.changed() => break,
        }

        if !state.recorder.is_recording() {
            continue;
        }

        let code = format!(
            "return window.__VICTAURI__?.getEventStream({})",
            last_drain_ts
        );
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut pending = state.pending_evals.lock().await;
            if pending.len() >= MAX_PENDING_EVALS {
                continue;
            }
            pending.insert(id.clone(), tx);
        }

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

        if bridge.eval_webview(None, &inject).is_err() {
            state.pending_evals.lock().await.remove(&id);
            continue;
        }

        let result = match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
            Ok(Ok(r)) => r,
            _ => {
                state.pending_evals.lock().await.remove(&id);
                continue;
            }
        };

        let events: Vec<serde_json::Value> = match serde_json::from_str(&result) {
            Ok(v) => v,
            Err(_) => continue,
        };

        for ev in &events {
            let ts = ev
                .get("timestamp")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            if ts > last_drain_ts {
                last_drain_ts = ts;
            }

            let event_type = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let now = Utc::now();

            let app_event = match event_type {
                "console" => AppEvent::StateChange {
                    key: format!(
                        "console.{}",
                        ev.get("level").and_then(|l| l.as_str()).unwrap_or("log")
                    ),
                    timestamp: now,
                    caused_by: ev
                        .get("message")
                        .and_then(|m| m.as_str())
                        .map(std::string::ToString::to_string),
                },
                "dom_mutation" => AppEvent::DomMutation {
                    webview_label: "main".to_string(),
                    timestamp: now,
                    mutation_count: ev
                        .get("count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u32,
                },
                "ipc" => {
                    let cmd = ev
                        .get("command")
                        .and_then(|c| c.as_str())
                        .unwrap_or("unknown");
                    AppEvent::Ipc(victauri_core::IpcCall {
                        id: uuid::Uuid::new_v4().to_string(),
                        command: cmd.to_string(),
                        timestamp: now,
                        result: match ev.get("status").and_then(|s| s.as_str()) {
                            Some("ok") => victauri_core::IpcResult::Ok(serde_json::Value::Null),
                            Some("error") => victauri_core::IpcResult::Err("error".to_string()),
                            _ => victauri_core::IpcResult::Pending,
                        },
                        duration_ms: ev
                            .get("duration_ms")
                            .and_then(serde_json::Value::as_f64)
                            .map(|d| d as u64),
                        arg_size_bytes: 0,
                        webview_label: "main".to_string(),
                    })
                }
                "network" => AppEvent::StateChange {
                    key: format!(
                        "network.{}",
                        ev.get("method").and_then(|m| m.as_str()).unwrap_or("GET")
                    ),
                    timestamp: now,
                    caused_by: ev
                        .get("url")
                        .and_then(|u| u.as_str())
                        .map(std::string::ToString::to_string),
                },
                "navigation" => AppEvent::WindowEvent {
                    label: "main".to_string(),
                    event: format!(
                        "navigation.{}",
                        ev.get("nav_type")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                    ),
                    timestamp: now,
                },
                _ => continue,
            };

            state.recorder.record_event(app_event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn try_bind_preferred_port_available() {
        let (listener, port) = try_bind(0).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(port, 0);
        assert_ne!(addr.port(), 0); // OS assigned a real port
    }

    #[tokio::test]
    async fn try_bind_falls_back_when_taken() {
        let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let blocked_port = blocker.local_addr().unwrap().port();

        let (_, actual) = try_bind(blocked_port).await.unwrap();
        assert_ne!(actual, blocked_port);
        assert!(actual > blocked_port);
        assert!(actual <= blocked_port + PORT_FALLBACK_RANGE);
    }

    #[test]
    fn port_file_roundtrip() {
        write_port_file(7777);
        let content = std::fs::read_to_string(port_file_path()).unwrap();
        assert_eq!(content, "7777");
        remove_port_file();
        assert!(!port_file_path().exists());
    }
}
