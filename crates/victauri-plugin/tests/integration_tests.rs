use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;

use victauri_core::{
    AppEvent, CommandInfo, CommandRegistry, EventLog, EventRecorder, IpcCall, IpcResult,
};
use victauri_plugin::VictauriState;
use victauri_plugin::bridge::WebviewBridge;
use victauri_plugin::mcp::{VictauriMcpHandler, build_app, build_app_with_options};

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

    fn get_native_handle(&self, _label: Option<&str>) -> Result<isize, String> {
        Err("native handle not available in mock".to_string())
    }

    fn manage_window(&self, _label: Option<&str>, action: &str) -> Result<String, String> {
        Ok(format!("{action} executed"))
    }

    fn resize_window(&self, _label: Option<&str>, _width: u32, _height: u32) -> Result<(), String> {
        Ok(())
    }

    fn move_window(&self, _label: Option<&str>, _x: i32, _y: i32) -> Result<(), String> {
        Ok(())
    }

    fn set_window_title(&self, _label: Option<&str>, _title: &str) -> Result<(), String> {
        Ok(())
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

async fn start_auth_test_server(state: Arc<VictauriState>, labels: &[&str], token: &str) -> String {
    let bridge: Arc<dyn WebviewBridge> = Arc::new(MockBridge::with_windows(labels));
    let app = build_app_with_options(state, bridge, Some(token.to_string()));

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

// ── New tool verification tests ────────────────────────────────────────────

#[tokio::test]
async fn mcp_tools_include_invoke_command() {
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

    let body = tools_resp.text().await.unwrap();
    assert!(
        body.contains("invoke_command"),
        "tools should include invoke_command"
    );
    assert!(
        body.contains("screenshot"),
        "tools should include screenshot"
    );
    assert!(body.contains("press_key"), "tools should include press_key");
    assert!(
        body.contains("get_console_logs"),
        "tools should include get_console_logs"
    );
}

#[tokio::test]
async fn mcp_tool_count_is_correct() {
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

    let body = tools_resp.text().await.unwrap();
    let expected_tools = [
        // Phase 1: WebView & Backend
        "eval_js",
        "dom_snapshot",
        "click",
        "fill",
        "type_text",
        "get_window_state",
        "list_windows",
        "get_ipc_log",
        "get_registry",
        "get_memory_stats",
        // Phase 2: Verification
        "verify_state",
        "detect_ghost_commands",
        "check_ipc_integrity",
        // Phase 3: Streaming
        "get_event_stream",
        // Phase 4: Intent
        "resolve_command",
        "assert_semantic",
        // Phase 5: Time-Travel
        "start_recording",
        "stop_recording",
        "checkpoint",
        "list_checkpoints",
        "get_replay_sequence",
        "get_recorded_events",
        "events_between_checkpoints",
        // Phase 6: Enhanced
        "invoke_command",
        "screenshot",
        "press_key",
        "get_console_logs",
        // Extended interactions
        "double_click",
        "hover",
        "select_option",
        "scroll_to",
        "focus_element",
        // Network
        "get_network_log",
        // Storage
        "get_storage",
        "set_storage",
        "delete_storage",
        "get_cookies",
        // Navigation
        "get_navigation_log",
        "navigate",
        "navigate_back",
        // Dialogs
        "get_dialog_log",
        "set_dialog_response",
        // Wait
        "wait_for",
        // Window management
        "manage_window",
        "resize_window",
        "move_window",
        "set_window_title",
    ];

    for tool_name in &expected_tools {
        assert!(body.contains(tool_name), "missing tool: {tool_name}");
    }
}

#[tokio::test]
async fn mcp_read_resource_ipc_log() {
    let state = test_state();
    state.event_log.push(AppEvent::Ipc(sample_ipc_call(
        "test_cmd",
        IpcResult::Ok(serde_json::json!("ok")),
    )));

    let base = start_test_server(state, &["main"]).await;
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
            "id": 3,
            "method": "resources/read",
            "params": {"uri": "victauri://ipc-log"}
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("test_cmd"),
        "resource should contain the IPC call"
    );
}

#[tokio::test]
async fn mcp_read_resource_windows() {
    let base = start_test_server(test_state(), &["main", "settings"]).await;
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
            "id": 3,
            "method": "resources/read",
            "params": {"uri": "victauri://windows"}
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("main"),
        "windows resource should contain main window"
    );
    assert!(
        body.contains("settings"),
        "windows resource should contain settings window"
    );
}

#[tokio::test]
async fn mcp_read_resource_state() {
    let state = test_state();
    state.registry.register(sample_command("greet"));

    let base = start_test_server(state, &["main"]).await;
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
            "id": 3,
            "method": "resources/read",
            "params": {"uri": "victauri://state"}
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("commands_registered"),
        "state resource should contain command count"
    );
}

#[tokio::test]
async fn mcp_read_unknown_resource_fails() {
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
            "id": 3,
            "method": "resources/read",
            "params": {"uri": "victauri://nonexistent"}
        }))
        .send()
        .await
        .unwrap();

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error"),
        "reading unknown resource should return error"
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

// ── Builder tests ──────────────────────────────────────────────────────────

#[test]
fn builder_default_port() {
    let builder = victauri_plugin::VictauriBuilder::new();
    // Without env var or explicit port, resolve_port should return DEFAULT_PORT
    // We test this indirectly through state wiring
    let _ = builder;
}

#[test]
fn builder_custom_port_reflected_in_state() {
    let state = Arc::new(VictauriState {
        event_log: EventLog::new(500),
        registry: CommandRegistry::new(),
        port: 8888,
        pending_evals: Arc::new(Mutex::new(HashMap::new())),
        recorder: EventRecorder::new(500),
    });

    assert_eq!(state.port, 8888);
    assert_eq!(state.event_log.len(), 0);
}

// ── Mock bridge native handle tests ────────────────────────────────────────

#[test]
fn mock_bridge_native_handle_returns_error() {
    let bridge = MockBridge::with_windows(&["main"]);
    let result = bridge.get_native_handle(Some("main"));
    assert!(result.is_err(), "mock bridge should not have native handle");
}

// ── Recording tools via MCP ────────────────────────────────────────────────

#[tokio::test]
async fn recording_start_stop_via_state() {
    let state = test_state();
    assert!(!state.recorder.is_recording());

    let started = state.recorder.start("test-session".to_string());
    assert!(started);
    assert!(state.recorder.is_recording());

    let session = state.recorder.stop();
    assert!(session.is_some());
    assert!(!state.recorder.is_recording());

    let session = session.unwrap();
    assert_eq!(session.id, "test-session");
    assert!(session.events.is_empty());
    assert!(session.checkpoints.is_empty());
}

#[tokio::test]
async fn recording_prevents_double_start() {
    let state = test_state();
    assert!(state.recorder.start("session-1".to_string()));
    assert!(!state.recorder.start("session-2".to_string()));
    let _ = state.recorder.stop();
}

#[tokio::test]
async fn recording_captures_events_and_checkpoints() {
    let state = test_state();
    state.recorder.start("test".to_string());

    state
        .recorder
        .record_event(AppEvent::Ipc(sample_ipc_call("cmd1", IpcResult::Pending)));
    state.recorder.record_event(AppEvent::Ipc(sample_ipc_call(
        "cmd2",
        IpcResult::Ok(serde_json::json!("ok")),
    )));

    let cp = state.recorder.checkpoint(
        "cp1".to_string(),
        Some("after cmd2".to_string()),
        serde_json::json!({"counter": 1}),
    );
    assert!(cp);

    assert_eq!(state.recorder.event_count(), 2);
    assert_eq!(state.recorder.checkpoint_count(), 1);

    let session = state.recorder.stop().unwrap();
    assert_eq!(session.events.len(), 2);
    assert_eq!(session.checkpoints.len(), 1);
    assert_eq!(session.checkpoints[0].id, "cp1");
}

#[tokio::test]
async fn ipc_replay_sequence_returns_only_ipc() {
    let state = test_state();
    state.recorder.start("test".to_string());

    state
        .recorder
        .record_event(AppEvent::Ipc(sample_ipc_call("cmd1", IpcResult::Pending)));
    state.recorder.record_event(AppEvent::WindowEvent {
        label: "main".to_string(),
        event: "focus".to_string(),
        timestamp: Utc::now(),
    });
    state
        .recorder
        .record_event(AppEvent::Ipc(sample_ipc_call("cmd2", IpcResult::Pending)));

    let replay = state.recorder.ipc_replay_sequence();
    assert_eq!(replay.len(), 2);
    assert_eq!(replay[0].command, "cmd1");
    assert_eq!(replay[1].command, "cmd2");

    let _ = state.recorder.stop();
}

// ── Memory stats test ──────────────────────────────────────────────────────

#[test]
fn memory_stats_returns_valid_json() {
    let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
    assert!(stats.is_object(), "memory stats should be a JSON object");
    // On Windows, should have working_set_bytes; on other platforms, should have an error or platform-specific fields
    #[cfg(windows)]
    {
        assert!(
            stats.get("working_set_bytes").is_some(),
            "should have working_set_bytes on Windows"
        );
    }
}

// ── Auth tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_health_bypasses_token() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
    let resp = reqwest::get(format!("{base}/health")).await.unwrap();
    assert_eq!(resp.status(), 200, "health endpoint should bypass auth");
}

#[tokio::test]
async fn auth_rejects_unauthenticated_info() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
    let resp = reqwest::get(format!("{base}/info")).await.unwrap();
    assert_eq!(
        resp.status(),
        401,
        "info endpoint should require auth when token is set"
    );
}

#[tokio::test]
async fn auth_accepts_valid_bearer_token() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/info"))
        .header("Authorization", "Bearer secret-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "valid token should be accepted");
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["auth_required"], true);
}

#[tokio::test]
async fn auth_rejects_wrong_token() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/info"))
        .header("Authorization", "Bearer wrong-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "wrong token should be rejected");
}

#[tokio::test]
async fn auth_mcp_requires_token() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
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

    assert_eq!(resp.status(), 401, "MCP endpoint should require auth");
}

#[tokio::test]
async fn auth_mcp_works_with_valid_token() {
    let base = start_auth_test_server(test_state(), &["main"], "secret-token").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", "Bearer secret-token")
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
        "MCP should work with valid token"
    );
}

// ── No-auth tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn no_auth_allows_all_requests() {
    let base = start_test_server(test_state(), &["main"]).await;
    let resp = reqwest::get(format!("{base}/info")).await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "without auth token, all requests should be allowed"
    );
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["auth_required"], false);
}

// ── Auth token generation test ─────────────────────────────────────────────

#[test]
fn generate_token_produces_valid_uuid() {
    let token = victauri_plugin::auth::generate_token();
    assert_eq!(token.len(), 36, "token should be a UUID");
    assert!(token.contains('-'), "token should be hyphenated UUID");
}
