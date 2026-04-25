use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, ListResourcesResult, PaginatedRequestParams,
    RawResource, ReadResourceRequestParams, ReadResourceResult, ResourceContents,
    ServerCapabilities, ServerInfo, SubscribeRequestParams, UnsubscribeRequestParams,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use tauri::Runtime;
use tokio::sync::Mutex;

use crate::bridge::WebviewBridge;
use crate::VictauriState;

const EVAL_TIMEOUT: Duration = Duration::from_secs(10);

// ── Parameter structs ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalJsParams {
    /// JavaScript code to evaluate in the webview. Async expressions supported.
    pub code: String,
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClickParams {
    /// Ref handle ID from a DOM snapshot (e.g. "e5").
    pub ref_id: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FillParams {
    /// Ref handle ID of the input element.
    pub ref_id: String,
    /// Value to set on the input.
    pub value: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeTextParams {
    /// Ref handle ID of the element to type into.
    pub ref_id: String,
    /// Text to type character by character.
    pub text: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnapshotParams {
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowStateParams {
    /// Filter to a specific window label. Returns all windows if omitted.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IpcLogParams {
    /// Maximum number of most recent entries to return.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegistryParams {
    /// Search query to filter commands by name or description.
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifyStateParams {
    /// JavaScript expression that returns the frontend state object to compare.
    pub frontend_expr: String,
    /// Backend state as a JSON object to compare against.
    pub backend_state: serde_json::Value,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GhostCommandParams {
    /// Optional filter: only consider IPC calls from this webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IpcIntegrityParams {
    /// Age in milliseconds after which a pending IPC call is considered stale. Default: 5000.
    pub stale_threshold_ms: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventStreamParams {
    /// Only return events after this Unix timestamp (milliseconds). If omitted, returns all events.
    pub since: Option<f64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveCommandParams {
    /// Natural language query describing what you want to do (e.g. "save the user's settings").
    pub query: String,
    /// Maximum number of results to return. Default: 5.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SemanticAssertParams {
    /// JavaScript expression to evaluate in the webview. The result is checked against the assertion.
    pub expression: String,
    /// Human-readable label for this assertion (e.g. "user is logged in").
    pub label: String,
    /// Condition: equals, not_equals, contains, greater_than, less_than, truthy, falsy, exists, type_is.
    pub condition: String,
    /// Expected value for the assertion.
    pub expected: serde_json::Value,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── MCP Handler ──────────────────────────────────────────────────────────────

const RESOURCE_URI_IPC_LOG: &str = "victauri://ipc-log";
const RESOURCE_URI_WINDOWS: &str = "victauri://windows";
const RESOURCE_URI_STATE: &str = "victauri://state";

#[derive(Clone)]
pub struct VictauriMcpHandler {
    state: Arc<VictauriState>,
    bridge: Arc<dyn WebviewBridge>,
    subscriptions: Arc<Mutex<HashSet<String>>>,
}

#[tool_router]
impl VictauriMcpHandler {
    #[tool(description = "Evaluate JavaScript in the Tauri webview and return the result. Async expressions are wrapped automatically.")]
    async fn eval_js(&self, Parameters(params): Parameters<EvalJsParams>) -> CallToolResult {
        match self
            .eval_with_return(&params.code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Get the current DOM snapshot from the webview as a JSON accessibility tree with ref handles for interaction.")]
    async fn dom_snapshot(
        &self,
        Parameters(params): Parameters<SnapshotParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.snapshot()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Click an element by its ref handle ID from a DOM snapshot.")]
    async fn click(&self, Parameters(params): Parameters<ClickParams>) -> CallToolResult {
        let code = format!(
            "return window.__VICTAURI__?.click('{}')",
            params.ref_id.replace('\'', "\\'")
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Set the value of an input element by ref handle ID. Dispatches input and change events.")]
    async fn fill(&self, Parameters(params): Parameters<FillParams>) -> CallToolResult {
        let escaped_value = params.value.replace('\\', "\\\\").replace('\'', "\\'");
        let code = format!(
            "return window.__VICTAURI__?.fill('{}', '{}')",
            params.ref_id.replace('\'', "\\'"),
            escaped_value
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Type text character-by-character into an element, simulating real keyboard events.")]
    async fn type_text(&self, Parameters(params): Parameters<TypeTextParams>) -> CallToolResult {
        let escaped_text = params.text.replace('\\', "\\\\").replace('\'', "\\'");
        let code = format!(
            "return window.__VICTAURI__?.type('{}', '{}')",
            params.ref_id.replace('\'', "\\'"),
            escaped_text
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Get state of all Tauri windows: position, size, visibility, focus, and URL.")]
    async fn get_window_state(
        &self,
        Parameters(params): Parameters<WindowStateParams>,
    ) -> CallToolResult {
        let states = self.bridge.get_window_states(params.label.as_deref());
        match serde_json::to_string_pretty(&states) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "List all Tauri window labels.")]
    async fn list_windows(&self) -> CallToolResult {
        let labels = self.bridge.list_window_labels();
        match serde_json::to_string_pretty(&labels) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Get recent IPC calls intercepted by Victauri's invoke handler wrapper.")]
    async fn get_ipc_log(
        &self,
        Parameters(params): Parameters<IpcLogParams>,
    ) -> CallToolResult {
        let mut calls = self.state.event_log.ipc_calls();
        if let Some(limit) = params.limit {
            let start = calls.len().saturating_sub(limit);
            calls = calls[start..].to_vec();
        }
        match serde_json::to_string_pretty(&calls) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "List or search all registered Tauri commands with their argument schemas.")]
    async fn get_registry(
        &self,
        Parameters(params): Parameters<RegistryParams>,
    ) -> CallToolResult {
        let commands = match params.query {
            Some(q) => self.state.registry.search(&q),
            None => self.state.registry.list(),
        };
        match serde_json::to_string_pretty(&commands) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Get current memory allocation statistics (allocated bytes, allocation count, deallocation count).")]
    async fn get_memory_stats(&self) -> CallToolResult {
        let stats = crate::memory::current_stats();
        match serde_json::to_string_pretty(&stats) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Compare frontend state (evaluated via JS expression) against backend state to detect divergences. Returns a VerificationResult with any mismatches.")]
    async fn verify_state(
        &self,
        Parameters(params): Parameters<VerifyStateParams>,
    ) -> CallToolResult {
        let code = format!("return ({})", params.frontend_expr);
        let frontend_json = match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => result,
            Err(e) => return tool_error(format!("failed to evaluate frontend expression: {e}")),
        };

        let frontend_state: serde_json::Value = match serde_json::from_str(&frontend_json) {
            Ok(v) => v,
            Err(e) => return tool_error(format!("frontend expression did not return valid JSON: {e}")),
        };

        let result = victauri_core::verify_state(frontend_state, params.backend_state);
        match serde_json::to_string_pretty(&result) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Detect ghost commands — commands invoked from the frontend that have no backend handler, or registered backend commands never called from the frontend. Scans the IPC log for frontend command names.")]
    async fn detect_ghost_commands(
        &self,
        Parameters(params): Parameters<GhostCommandParams>,
    ) -> CallToolResult {
        let ipc_calls = self.state.event_log.ipc_calls();
        let frontend_commands: Vec<String> = ipc_calls
            .iter()
            .filter(|c| {
                params
                    .webview_label
                    .as_ref()
                    .is_none_or(|label| c.webview_label == *label)
            })
            .map(|c| c.command.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let report = victauri_core::detect_ghost_commands(&frontend_commands, &self.state.registry);
        match serde_json::to_string_pretty(&report) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Check IPC round-trip integrity: find stale (stuck) pending calls and errored calls. Returns health status and lists of problematic IPC calls.")]
    async fn check_ipc_integrity(
        &self,
        Parameters(params): Parameters<IpcIntegrityParams>,
    ) -> CallToolResult {
        let threshold = params.stale_threshold_ms.unwrap_or(5000);
        let report = victauri_core::check_ipc_integrity(&self.state.event_log, threshold);
        match serde_json::to_string_pretty(&report) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Get a combined event stream from the webview: console logs, DOM mutations, sorted by timestamp. Use the 'since' parameter to poll only new events.")]
    async fn get_event_stream(
        &self,
        Parameters(params): Parameters<EventStreamParams>,
    ) -> CallToolResult {
        let since_arg = params
            .since
            .map(|ts| format!("{ts}"))
            .unwrap_or_default();
        let code = if since_arg.is_empty() {
            "return window.__VICTAURI__?.getEventStream()".to_string()
        } else {
            format!("return window.__VICTAURI__?.getEventStream({since_arg})")
        };
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Resolve a natural language query to matching Tauri commands. Returns scored results ranked by relevance, using command names, descriptions, intents, categories, and examples.")]
    async fn resolve_command(
        &self,
        Parameters(params): Parameters<ResolveCommandParams>,
    ) -> CallToolResult {
        let limit = params.limit.unwrap_or(5);
        let mut results = self.state.registry.resolve(&params.query);
        results.truncate(limit);
        match serde_json::to_string_pretty(&results) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Run a semantic assertion: evaluate a JS expression and check the result against an expected condition. Conditions: equals, not_equals, contains, greater_than, less_than, truthy, falsy, exists, type_is.")]
    async fn assert_semantic(
        &self,
        Parameters(params): Parameters<SemanticAssertParams>,
    ) -> CallToolResult {
        let code = format!("return ({})", params.expression);
        let actual_json = match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => result,
            Err(e) => return tool_error(format!("failed to evaluate expression: {e}")),
        };

        let actual: serde_json::Value = match serde_json::from_str(&actual_json) {
            Ok(v) => v,
            Err(e) => return tool_error(format!("expression did not return valid JSON: {e}")),
        };

        let assertion = victauri_core::SemanticAssertion {
            label: params.label,
            condition: params.condition,
            expected: params.expected,
        };

        let result = victauri_core::evaluate_assertion(actual, &assertion);
        match serde_json::to_string_pretty(&result) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }
}

impl VictauriMcpHandler {
    async fn eval_with_return(
        &self,
        code: &str,
        webview_label: Option<&str>,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.state.pending_evals.lock().await.insert(id.clone(), tx);

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

        if let Err(e) = self.bridge.eval_webview(webview_label, &inject) {
            self.state.pending_evals.lock().await.remove(&id);
            return Err(format!("eval injection failed: {e}"));
        }

        match tokio::time::timeout(EVAL_TIMEOUT, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err("eval callback channel closed".to_string()),
            Err(_) => {
                self.state.pending_evals.lock().await.remove(&id);
                Err("eval timed out after 10s".to_string())
            }
        }
    }
}

#[tool_handler(
    instructions = "Victauri gives you X-ray vision into a running Tauri application. \
                    You can evaluate JS, snapshot the DOM, click/fill/type UI elements, \
                    inspect window state, view IPC traffic, search the command registry, \
                    monitor memory usage, and subscribe to live resource streams — all through MCP."
)]
impl ServerHandler for VictauriMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new(RESOURCE_URI_IPC_LOG, "ipc-log")
                    .with_description("Live IPC call log — all commands invoked between frontend and backend")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new(RESOURCE_URI_WINDOWS, "windows")
                    .with_description("Current state of all Tauri windows — position, size, visibility, focus")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new(RESOURCE_URI_STATE, "state")
                    .with_description("Victauri plugin state — event count, registered commands, memory stats")
                    .with_mime_type("application/json")
                    .no_annotation(),
            ],
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let uri = &request.uri;
        let json = match uri.as_str() {
            RESOURCE_URI_IPC_LOG => {
                let calls = self.state.event_log.ipc_calls();
                serde_json::to_string_pretty(&calls)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            RESOURCE_URI_WINDOWS => {
                let states = self.bridge.get_window_states(None);
                serde_json::to_string_pretty(&states)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            RESOURCE_URI_STATE => {
                let state_json = serde_json::json!({
                    "events_captured": self.state.event_log.len(),
                    "commands_registered": self.state.registry.count(),
                    "memory": crate::memory::current_stats(),
                    "port": self.state.port,
                });
                serde_json::to_string_pretty(&state_json)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            _ => {
                return Err(ErrorData::resource_not_found(
                    format!("unknown resource: {uri}"),
                    None,
                ));
            }
        };

        Ok(ReadResourceResult::new(vec![ResourceContents::text(json, uri)]))
    }

    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        let uri = &request.uri;
        match uri.as_str() {
            RESOURCE_URI_IPC_LOG | RESOURCE_URI_WINDOWS | RESOURCE_URI_STATE => {
                self.subscriptions.lock().await.insert(uri.clone());
                tracing::info!("Client subscribed to resource: {uri}");
                Ok(())
            }
            _ => Err(ErrorData::resource_not_found(
                format!("unknown resource: {uri}"),
                None,
            )),
        }
    }

    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.subscriptions.lock().await.remove(&request.uri);
        tracing::info!("Client unsubscribed from resource: {}", request.uri);
        Ok(())
    }
}

fn tool_error(msg: impl Into<String>) -> CallToolResult {
    let mut result = CallToolResult::success(vec![Content::text(msg)]);
    result.is_error = Some(true);
    result
}

// ── Server startup ───────────────────────────────────────────────────────────

pub async fn start_server<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
) -> anyhow::Result<()> {
    let bridge: Arc<dyn WebviewBridge> = Arc::new(app_handle);
    let handler = VictauriMcpHandler {
        state: state.clone(),
        bridge,
        subscriptions: Arc::new(Mutex::new(HashSet::new())),
    };

    let mcp_service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let info_state = state.clone();
    let app = axum::Router::new()
        .route_service("/mcp", mcp_service)
        .route("/health", axum::routing::get(|| async { "ok" }))
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
                        "port": s.port,
                    }))
                }
            }),
        );

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("Victauri MCP server listening on 127.0.0.1:{port}");

    axum::serve(listener, app).await?;
    Ok(())
}
