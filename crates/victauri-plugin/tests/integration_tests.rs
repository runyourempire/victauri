use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;

use victauri_core::{
    AppEvent, CommandInfo, CommandRegistry, EventLog, EventRecorder, IpcCall, IpcResult,
};
use victauri_plugin::VictauriState;
use victauri_plugin::bridge::WebviewBridge;
use victauri_plugin::mcp::{VictauriMcpHandler, build_app};

// ── Mock Bridge ─────────────────────────────────────────────────────────────

struct MockBridge {
    windows: Vec<victauri_core::WindowState>,
}

impl MockBridge {
    fn with_windows(labels: &[&str]) -> Self {
        Self {
            windows: labels
                .iter()
                .map(|label| victauri_core::WindowState {
                    label: label.to_string(),
                    title: format!("{label} Window"),
                    url: "http://localhost/".to_string(),
                    visible: true,
                    focused: labels.first() == Some(label),
                    maximized: false,
                    minimized: false,
                    fullscreen: false,
                    position: (100, 100),
                    size: (800, 600),
                })
                .collect(),
        }
    }
}

impl WebviewBridge for MockBridge {
    fn eval_webview(&self, _label: Option<&str>, _script: &str) -> Result<(), String> {
        Ok(())
    }

    fn get_window_states(&self, label: Option<&str>) -> Vec<victauri_core::WindowState> {
        match label {
            Some(l) => self
                .windows
                .iter()
                .filter(|w| w.label == l)
                .cloned()
                .collect(),
            None => self.windows.clone(),
        }
    }

    fn list_window_labels(&self) -> Vec<String> {
        self.windows.iter().map(|w| w.label.clone()).collect()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn test_state() -> Arc<VictauriState> {
    Arc::new(VictauriState {
        event_log: EventLog::new(1000),
        registry: CommandRegistry::new(),
        port: 0,
        pending_evals: Arc::new(Mutex::new(HashMap::new())),
        recorder: EventRecorder::new(1000),
    })
}

async fn start_test_server(state: Arc<VictauriState>, labels: &[&str]) -> String {
    let bridge: Arc<dyn WebviewBridge> = Arc::new(MockBridge::with_windows(labels));
    let app = build_app(state, bridge);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{addr}")
}

fn sample_command(name: &str) -> CommandInfo {
    CommandInfo {
        name: name.to_string(),
        plugin: None,
        description: Some(format!("{name} command")),
        args: vec![],
        return_type: Some("String".to_string()),
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    }
}

fn sample_ipc_call(command: &str, result: IpcResult) -> IpcCall {
    IpcCall {
        id: uuid::Uuid::new_v4().to_string(),
        command: command.to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result,
        arg_size_bytes: 42,
        webview_label: "main".to_string(),
    }
}

// ── Handler construction tests ──────────────────────────────────────────────

#[test]
fn handler_new_creates_instance() {
    let state = test_state();
    let bridge: Arc<dyn WebviewBridge> = Arc::new(MockBridge::with_windows(&["main"]));
    let _ = VictauriMcpHandler::new(state, bridge);
}

#[test]
fn handler_get_info_has_correct_capabilities() {
    use rmcp::ServerHandler;

    let state = test_state();
    let bridge: Arc<dyn WebviewBridge> = Arc::new(MockBridge::with_windows(&["main"]));
    let handler = VictauriMcpHandler::new(state, bridge);
    let info = handler.get_info();

    assert!(
        info.capabilities.tools.is_some(),
        "tools capability missing"
    );
    assert!(
        info.capabilities.resources.is_some(),
        "resources capability missing"
    );
}

// ── Mock bridge tests ───────────────────────────────────────────────────────

#[test]
fn mock_bridge_returns_window_states() {
    let bridge = MockBridge::with_windows(&["main", "settings"]);
    let states = bridge.get_window_states(None);
    assert_eq!(states.len(), 2);
}

#[test]
fn mock_bridge_filters_by_label() {
    let bridge = MockBridge::with_windows(&["main", "settings"]);
    let states = bridge.get_window_states(Some("settings"));
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].label, "settings");
}

#[test]
fn mock_bridge_returns_empty_for_unknown_label() {
    let bridge = MockBridge::with_windows(&["main"]);
    let states = bridge.get_window_states(Some("nonexistent"));
    assert!(states.is_empty());
}

#[test]
fn mock_bridge_list_labels() {
    let bridge = MockBridge::with_windows(&["alpha", "beta"]);
    let mut labels = bridge.list_window_labels();
    labels.sort();
    assert_eq!(labels, vec!["alpha", "beta"]);
}

// ── HTTP endpoint tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let base = start_test_server(test_state(), &["main"]).await;

    let resp = reqwest::get(format!("{base}/health")).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn info_endpoint_returns_valid_json() {
    let base = start_test_server(test_state(), &["main"]).await;

    let resp = reqwest::get(format!("{base}/info")).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(json["name"], "victauri");
    assert_eq!(json["protocol"], "mcp");
    assert!(json["version"].is_string());
    assert_eq!(json["commands_registered"], 0);
    assert_eq!(json["events_captured"], 0);
}

#[tokio::test]
async fn info_reflects_registered_commands() {
    let state = test_state();
    state.registry.register(sample_command("greet"));
    state.registry.register(sample_command("save_user"));
    state.registry.register(sample_command("get_config"));

    let base = start_test_server(state, &["main"]).await;
    let json: serde_json::Value = reqwest::get(format!("{base}/info"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(json["commands_registered"], 3);
}

#[tokio::test]
async fn info_reflects_event_count() {
    let state = test_state();
    state
        .event_log
        .push(AppEvent::Ipc(sample_ipc_call("cmd1", IpcResult::Pending)));
    state
        .event_log
        .push(AppEvent::Ipc(sample_ipc_call("cmd2", IpcResult::Pending)));

    let base = start_test_server(state, &["main"]).await;
    let json: serde_json::Value = reqwest::get(format!("{base}/info"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(json["events_captured"], 2);
}

// ── MCP protocol tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn mcp_endpoint_accepts_post() {
    let base = start_test_server(test_state(), &["main"]).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.1.0"}
            }
        }))
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_success(),
        "MCP initialize returned {}",
        resp.status()
    );
}

#[tokio::test]
async fn mcp_initialize_returns_session() {
    let base = start_test_server(test_state(), &["main"]).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.1.0"}
            }
        }))
        .send()
        .await
        .unwrap();

    let session_id = resp.headers().get("mcp-session-id");
    assert!(
        session_id.is_some(),
        "MCP response should include session ID header"
    );
}

#[tokio::test]
async fn mcp_full_session_lists_tools() {
    let base = start_test_server(test_state(), &["main"]).await;
    let client = reqwest::Client::new();

    let init_resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.1.0"}
            }
        }))
        .send()
        .await
        .unwrap();

    let session_id = init_resp
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Send initialized notification
    client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .send()
        .await
        .unwrap();

    // List tools
    let tools_resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("mcp-session-id", &session_id)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();

    assert!(tools_resp.status().is_success());
    let body = tools_resp.text().await.unwrap();

    assert!(body.contains("eval_js"), "tools should include eval_js");
    assert!(
        body.contains("dom_snapshot"),
        "tools should include dom_snapshot"
    );
    assert!(
        body.contains("start_recording"),
        "tools should include start_recording"
    );
    assert!(
        body.contains("verify_state"),
        "tools should include verify_state"
    );
    assert!(
        body.contains("resolve_command"),
        "tools should include resolve_command"
    );
}

#[tokio::test]
async fn mcp_full_session_lists_resources() {
    let base = start_test_server(test_state(), &["main"]).await;
    let client = reqwest::Client::new();

    let init_resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.1.0"}
            }
        }))
        .send()
        .await
        .unwrap();

    let session_id = init_resp
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("mcp-session-id", &session_id)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("mcp-session-id", &session_id)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("victauri://ipc-log"),
        "should list ipc-log resource"
    );
    assert!(
        body.contains("victauri://windows"),
        "should list windows resource"
    );
    assert!(
        body.contains("victauri://state"),
        "should list state resource"
    );
}

// ── State wiring tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn state_port_reflected_in_info() {
    let state = Arc::new(VictauriState {
        event_log: EventLog::new(1000),
        registry: CommandRegistry::new(),
        port: 9999,
        pending_evals: Arc::new(Mutex::new(HashMap::new())),
        recorder: EventRecorder::new(1000),
    });

    let bridge: Arc<dyn WebviewBridge> = Arc::new(MockBridge::with_windows(&["main"]));
    let app = build_app(state, bridge);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let json: serde_json::Value = reqwest::get(format!("http://{addr}/info"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(json["port"], 9999);
}

#[tokio::test]
async fn concurrent_requests_work() {
    let base = start_test_server(test_state(), &["main"]).await;

    let mut handles = vec![];
    for _ in 0..10 {
        let url = format!("{base}/health");
        handles.push(tokio::spawn(async move {
            reqwest::get(&url).await.unwrap().text().await.unwrap()
        }));
    }

    for handle in handles {
        assert_eq!(handle.await.unwrap(), "ok");
    }
}
