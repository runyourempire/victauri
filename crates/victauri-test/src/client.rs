use serde_json::{Value, json};

use crate::error::TestError;

/// Typed HTTP client for the Victauri MCP server.
///
/// Manages session lifecycle (initialize → tool calls → cleanup) and provides
/// convenient methods for common test operations.
pub struct VictauriClient {
    http: reqwest::Client,
    base_url: String,
    session_id: String,
    next_id: u64,
    auth_token: Option<String>,
}

impl VictauriClient {
    /// Connect to a Victauri MCP server on the given port.
    /// Sends `initialize` and `notifications/initialized` automatically.
    pub async fn connect(port: u16) -> Result<Self, TestError> {
        Self::connect_with_token(port, None).await
    }

    /// Connect with an optional Bearer auth token.
    ///
    /// Retries up to 3 times with exponential backoff on 429 (rate limited).
    pub async fn connect_with_token(port: u16, token: Option<&str>) -> Result<Self, TestError> {
        let base_url = format!("http://127.0.0.1:{port}");
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| TestError::Connection(e.to_string()))?;

        let init_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "victauri-test", "version": env!("CARGO_PKG_VERSION")}
            }
        });

        let mut init_resp = None;
        for attempt in 0..4 {
            let mut req = http
                .post(format!("{base_url}/mcp"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .json(&init_body);
            if let Some(t) = token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }

            let resp = req
                .send()
                .await
                .map_err(|e| TestError::Connection(e.to_string()))?;

            if resp.status() == 429 && attempt < 3 {
                let delay = std::time::Duration::from_millis(100 * (1 << attempt));
                tokio::time::sleep(delay).await;
                continue;
            }

            init_resp = Some(resp);
            break;
        }

        let init_resp = init_resp
            .ok_or_else(|| TestError::Connection("initialize failed after retries".into()))?;

        if !init_resp.status().is_success() {
            return Err(TestError::Connection(format!(
                "initialize returned {}",
                init_resp.status()
            )));
        }

        let session_id = init_resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| TestError::Connection("no mcp-session-id header".into()))?
            .to_string();

        let mut notify_req = http
            .post(format!("{base_url}/mcp"))
            .header("Content-Type", "application/json")
            .header("mcp-session-id", &session_id)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }));

        if let Some(t) = token {
            notify_req = notify_req.header("Authorization", format!("Bearer {t}"));
        }

        notify_req.send().await?;

        Ok(Self {
            http,
            base_url,
            session_id,
            next_id: 10,
            auth_token: token.map(String::from),
        })
    }

    /// Auto-discover a running Victauri server via temp files.
    ///
    /// Reads `<temp>/victauri.port` and `<temp>/victauri.token` written by the
    /// plugin on startup. Falls back to `VICTAURI_PORT` / `VICTAURI_AUTH_TOKEN`
    /// env vars, then defaults (port 7373, no auth).
    pub async fn discover() -> Result<Self, TestError> {
        let port = Self::discover_port();
        let token = Self::discover_token();
        Self::connect_with_token(port, token.as_deref()).await
    }

    fn discover_port() -> u16 {
        if let Ok(p) = std::env::var("VICTAURI_PORT")
            && let Ok(port) = p.parse::<u16>()
        {
            return port;
        }
        let path = std::env::temp_dir().join("victauri.port");
        if let Ok(contents) = std::fs::read_to_string(&path)
            && let Ok(port) = contents.trim().parse::<u16>()
        {
            return port;
        }
        7373
    }

    fn discover_token() -> Option<String> {
        if let Ok(token) = std::env::var("VICTAURI_AUTH_TOKEN") {
            return Some(token);
        }
        let path = std::env::temp_dir().join("victauri.token");
        let token = std::fs::read_to_string(&path).ok()?;
        let token = token.trim().to_string();
        if token.is_empty() { None } else { Some(token) }
    }

    /// Call an MCP tool by name and return the result content as JSON.
    ///
    /// Retries up to 3 times with exponential backoff on 429 (rate limited).
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, TestError> {
        let id = self.next_id;
        self.next_id += 1;

        let call_body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let mut resp = None;
        for attempt in 0..4 {
            let mut req = self
                .http
                .post(format!("{}/mcp", self.base_url))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("mcp-session-id", &self.session_id)
                .json(&call_body);
            if let Some(ref t) = self.auth_token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            let r = req.send().await?;

            if r.status() == 429 && attempt < 3 {
                let delay = std::time::Duration::from_millis(100 * (1 << attempt));
                tokio::time::sleep(delay).await;
                continue;
            }
            resp = Some(r);
            break;
        }

        let resp =
            resp.ok_or_else(|| TestError::Connection("tool call failed after retries".into()))?;
        let body: Value = resp.json().await?;

        if let Some(error) = body.get("error") {
            return Err(TestError::Mcp {
                code: error["code"].as_i64().unwrap_or(-1),
                message: error["message"].as_str().unwrap_or("unknown").to_string(),
            });
        }

        let content = &body["result"]["content"];
        if let Some(arr) = content.as_array()
            && let Some(first) = arr.first()
            && let Some(text) = first["text"].as_str()
        {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                return Ok(parsed);
            }
            return Ok(Value::String(text.to_string()));
        }

        Ok(body)
    }

    /// Evaluate JavaScript in the webview and return the result.
    pub async fn eval_js(&mut self, code: &str) -> Result<Value, TestError> {
        self.call_tool("eval_js", json!({"code": code})).await
    }

    /// Get a DOM snapshot of the current page.
    pub async fn dom_snapshot(&mut self) -> Result<Value, TestError> {
        self.call_tool("dom_snapshot", json!({})).await
    }

    /// Click an element by ref handle ID.
    pub async fn click(&mut self, ref_id: &str) -> Result<Value, TestError> {
        self.call_tool("interact", json!({"action": "click", "ref_id": ref_id}))
            .await
    }

    /// Fill an input element with a value.
    pub async fn fill(&mut self, ref_id: &str, value: &str) -> Result<Value, TestError> {
        self.call_tool(
            "input",
            json!({"action": "fill", "ref_id": ref_id, "value": value}),
        )
        .await
    }

    /// Type text into an element character by character.
    pub async fn type_text(&mut self, ref_id: &str, text: &str) -> Result<Value, TestError> {
        self.call_tool(
            "input",
            json!({"action": "type_text", "ref_id": ref_id, "text": text}),
        )
        .await
    }

    /// List all window labels.
    pub async fn list_windows(&mut self) -> Result<Value, TestError> {
        self.call_tool("window", json!({"action": "list"})).await
    }

    /// Get the state of a specific window (or all windows).
    pub async fn get_window_state(&mut self, label: Option<&str>) -> Result<Value, TestError> {
        let mut args = json!({"action": "get_state"});
        if let Some(l) = label {
            args["label"] = json!(l);
        }
        self.call_tool("window", args).await
    }

    /// Take a screenshot and return base64-encoded PNG.
    pub async fn screenshot(&mut self) -> Result<Value, TestError> {
        self.call_tool("screenshot", json!({})).await
    }

    /// Invoke a Tauri command by name with optional arguments.
    pub async fn invoke_command(
        &mut self,
        command: &str,
        args: Option<Value>,
    ) -> Result<Value, TestError> {
        let mut params = json!({"command": command});
        if let Some(a) = args {
            params["args"] = a;
        }
        self.call_tool("invoke_command", params).await
    }

    /// Get the IPC call log.
    pub async fn get_ipc_log(&mut self, limit: Option<usize>) -> Result<Value, TestError> {
        let mut args = json!({"action": "ipc"});
        if let Some(n) = limit {
            args["limit"] = json!(n);
        }
        self.call_tool("logs", args).await
    }

    /// Verify frontend state against backend state.
    pub async fn verify_state(
        &mut self,
        frontend_expr: &str,
        backend_state: Value,
    ) -> Result<Value, TestError> {
        self.call_tool(
            "verify_state",
            json!({
                "frontend_expr": frontend_expr,
                "backend_state": backend_state,
            }),
        )
        .await
    }

    /// Detect ghost commands (registered but never called, or called but not registered).
    pub async fn detect_ghost_commands(&mut self) -> Result<Value, TestError> {
        self.call_tool("detect_ghost_commands", json!({})).await
    }

    /// Check IPC call health (pending, stale, errored).
    pub async fn check_ipc_integrity(&mut self) -> Result<Value, TestError> {
        self.call_tool("check_ipc_integrity", json!({})).await
    }

    /// Run a semantic assertion against a JS expression.
    pub async fn assert_semantic(
        &mut self,
        expression: &str,
        label: &str,
        condition: &str,
        expected: Value,
    ) -> Result<Value, TestError> {
        self.call_tool(
            "assert_semantic",
            json!({
                "expression": expression,
                "label": label,
                "condition": condition,
                "expected": expected,
            }),
        )
        .await
    }

    /// Run an accessibility audit.
    pub async fn audit_accessibility(&mut self) -> Result<Value, TestError> {
        self.call_tool("inspect", json!({"action": "audit_accessibility"}))
            .await
    }

    /// Get performance metrics (timing, heap, resources).
    pub async fn get_performance_metrics(&mut self) -> Result<Value, TestError> {
        self.call_tool("inspect", json!({"action": "get_performance"}))
            .await
    }

    /// Get the command registry.
    pub async fn get_registry(&mut self) -> Result<Value, TestError> {
        self.call_tool("get_registry", json!({})).await
    }

    /// Get process memory statistics.
    pub async fn get_memory_stats(&mut self) -> Result<Value, TestError> {
        self.call_tool("get_memory_stats", json!({})).await
    }

    /// Read plugin info (version, uptime, tool count).
    pub async fn get_plugin_info(&mut self) -> Result<Value, TestError> {
        self.call_tool("get_plugin_info", json!({})).await
    }

    /// Wait for a condition to be met, polling at an interval.
    ///
    /// Conditions: `text`, `text_gone`, `selector`, `selector_gone`, `url`,
    /// `ipc_idle`, `network_idle`.
    pub async fn wait_for(
        &mut self,
        condition: &str,
        value: Option<&str>,
        timeout_ms: Option<u64>,
        poll_ms: Option<u64>,
    ) -> Result<Value, TestError> {
        let mut args = json!({"condition": condition});
        if let Some(v) = value {
            args["value"] = json!(v);
        }
        if let Some(t) = timeout_ms {
            args["timeout_ms"] = json!(t);
        }
        if let Some(p) = poll_ms {
            args["poll_ms"] = json!(p);
        }
        self.call_tool("wait_for", args).await
    }

    /// Start a time-travel recording session.
    pub async fn start_recording(&mut self, session_id: Option<&str>) -> Result<Value, TestError> {
        let mut args = json!({"action": "start"});
        if let Some(id) = session_id {
            args["session_id"] = json!(id);
        }
        self.call_tool("recording", args).await
    }

    /// Stop the recording and return the session.
    pub async fn stop_recording(&mut self) -> Result<Value, TestError> {
        self.call_tool("recording", json!({"action": "stop"})).await
    }

    /// Export the current recording session as JSON.
    pub async fn export_session(&mut self) -> Result<Value, TestError> {
        self.call_tool("recording", json!({"action": "export"}))
            .await
    }

    /// Search for elements by various criteria without a full snapshot.
    pub async fn find_elements(&mut self, query: Value) -> Result<Value, TestError> {
        self.call_tool("find_elements", query).await
    }

    /// Hover over an element by ref handle.
    pub async fn hover(&mut self, ref_id: &str) -> Result<Value, TestError> {
        self.call_tool("interact", json!({"action": "hover", "ref_id": ref_id}))
            .await
    }

    /// Focus an element by ref handle.
    pub async fn focus(&mut self, ref_id: &str) -> Result<Value, TestError> {
        self.call_tool("interact", json!({"action": "focus", "ref_id": ref_id}))
            .await
    }

    /// Press a keyboard key.
    pub async fn press_key(&mut self, key: &str) -> Result<Value, TestError> {
        self.call_tool("input", json!({"action": "press_key", "key": key}))
            .await
    }

    /// Navigate to a URL.
    pub async fn navigate(&mut self, url: &str) -> Result<Value, TestError> {
        self.call_tool("navigate", json!({"action": "go_to", "url": url}))
            .await
    }

    /// Get logs by type (console, network, ipc, navigation, dialogs).
    pub async fn logs(&mut self, action: &str, limit: Option<usize>) -> Result<Value, TestError> {
        self.call_tool("logs", json!({"action": action, "limit": limit}))
            .await
    }

    /// Scroll an element into view by ref handle.
    pub async fn scroll_to(&mut self, ref_id: &str) -> Result<Value, TestError> {
        self.call_tool(
            "interact",
            json!({"action": "scroll_into_view", "ref_id": ref_id}),
        )
        .await
    }

    /// Select option(s) in a `<select>` element.
    pub async fn select_option(
        &mut self,
        ref_id: &str,
        values: &[&str],
    ) -> Result<Value, TestError> {
        self.call_tool(
            "interact",
            json!({"action": "select_option", "ref_id": ref_id, "values": values}),
        )
        .await
    }

    /// Get the server base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the MCP session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

// ── Assertion Helpers ────────────────────────────────────────────────────────

/// Assert that a JSON value at the given pointer equals the expected value.
///
/// ```rust,ignore
/// let state = client.get_window_state(Some("main")).await?;
/// victauri_test::assert_json_eq(&state, "/visible", &json!(true));
/// ```
pub fn assert_json_eq(value: &Value, pointer: &str, expected: &Value) {
    let actual = value.pointer(pointer);
    assert!(
        actual == Some(expected),
        "JSON pointer {pointer}: expected {expected}, got {}",
        actual.map_or("missing".to_string(), std::string::ToString::to_string)
    );
}

/// Assert that a JSON value at the given pointer is truthy (not null/false/0/"").
pub fn assert_json_truthy(value: &Value, pointer: &str) {
    let actual = value.pointer(pointer);
    let is_truthy = match actual {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(_)) => true,
    };
    assert!(
        is_truthy,
        "JSON pointer {pointer}: expected truthy, got {}",
        actual.map_or("missing".to_string(), std::string::ToString::to_string)
    );
}

/// Assert that an accessibility audit has zero violations.
pub fn assert_no_a11y_violations(audit: &Value) {
    let violations = audit
        .pointer("/summary/violations")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    assert_eq!(
        violations, 0,
        "expected 0 accessibility violations, got {violations}"
    );
}

/// Assert that all performance metrics are within budget.
pub fn assert_performance_budget(metrics: &Value, max_load_ms: f64, max_heap_mb: f64) {
    if let Some(load) = metrics
        .pointer("/navigation/load_event_ms")
        .and_then(serde_json::Value::as_f64)
    {
        assert!(
            load <= max_load_ms,
            "load event took {load}ms, budget is {max_load_ms}ms"
        );
    }

    if let Some(heap) = metrics
        .pointer("/js_heap/used_mb")
        .and_then(serde_json::Value::as_f64)
    {
        assert!(
            heap <= max_heap_mb,
            "JS heap is {heap}MB, budget is {max_heap_mb}MB"
        );
    }
}

/// Assert that IPC integrity is healthy (no stale or errored calls).
pub fn assert_ipc_healthy(integrity: &Value) {
    let healthy = integrity
        .get("healthy")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    assert!(
        healthy,
        "IPC integrity check failed: {}",
        serde_json::to_string_pretty(integrity).unwrap_or_default()
    );
}

/// Assert that state verification passed with no divergences.
pub fn assert_state_matches(verification: &Value) {
    let passed = verification
        .get("passed")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    assert!(
        passed,
        "state verification failed: {}",
        serde_json::to_string_pretty(verification).unwrap_or_default()
    );
}
