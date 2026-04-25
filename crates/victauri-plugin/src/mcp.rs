use std::sync::Arc;
use tauri::Runtime;

use crate::VictauriState;

pub async fn start_server<R: Runtime>(
    _app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
) -> anyhow::Result<()> {
    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .route("/info", axum::routing::get(info))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("Victauri MCP server listening on 127.0.0.1:{port}");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn info(
    axum::extract::State(state): axum::extract::State<Arc<VictauriState>>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "name": "victauri",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "mcp",
        "commands_registered": state.registry.count(),
        "events_captured": state.event_log.len(),
        "port": state.port,
    }))
}

// TODO: Phase 1 milestone — wire up full rmcp MCP server with:
//   - tools: snapshot, screenshot, click, type, fill, invoke, eval,
//            query_db, rewind, verify, memory_delta
//   - resources: tauri://app/commands, tauri://app/ipc-log,
//                tauri://app/windows, tauri://app/state
//   - prompts: /test-workflow, /debug-component, /regression-hunt
//   - sampling: run_test_scenario AI-driven loop
