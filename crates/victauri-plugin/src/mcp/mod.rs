mod backend_params;
mod helpers;
mod introspection_params;
mod other_params;
mod recording_params;
mod verification_params;
mod webview_params;
mod window_params;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolRequestParams, CallToolResult, Content, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, RawResource, ReadResourceRequestParams,
    ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo, SubscribeRequestParams,
    Tool, UnsubscribeRequestParams,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_router};
use tauri::Runtime;
use tokio::sync::Mutex;

use crate::VictauriState;
use crate::bridge::WebviewBridge;

use helpers::{js_string, sanitize_css_color, tool_disabled, tool_error, validate_url};

pub use backend_params::*;
pub use introspection_params::*;
pub use other_params::*;
pub use recording_params::*;
pub use verification_params::*;
pub use webview_params::*;
pub use window_params::*;

// ── MCP Handler ──────────────────────────────────────────────────────────────

/// Maximum number of in-flight JavaScript eval requests. Prevents unbounded
/// growth of the `pending_evals` map if callbacks are never resolved.
const MAX_PENDING_EVALS: usize = 100;

const RESOURCE_URI_IPC_LOG: &str = "victauri://ipc-log";
const RESOURCE_URI_WINDOWS: &str = "victauri://windows";
const RESOURCE_URI_STATE: &str = "victauri://state";

const BRIDGE_VERSION: &str = "0.2.0";

#[derive(Clone)]
pub struct VictauriMcpHandler {
    state: Arc<VictauriState>,
    bridge: Arc<dyn WebviewBridge>,
    subscriptions: Arc<Mutex<HashSet<String>>>,
    bridge_checked: Arc<AtomicBool>,
}

#[tool_router]
impl VictauriMcpHandler {
    #[tool(
        description = "Evaluate JavaScript in the Tauri webview and return the result. Async expressions are wrapped automatically."
    )]
    async fn eval_js(&self, Parameters(params): Parameters<EvalJsParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("eval_js") {
            return tool_disabled("eval_js");
        }
        match self
            .eval_with_return(&params.code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get the current DOM snapshot from the webview as a JSON accessibility tree with ref handles for interaction."
    )]
    async fn dom_snapshot(&self, Parameters(params): Parameters<SnapshotParams>) -> CallToolResult {
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
            "return window.__VICTAURI__?.click({})",
            js_string(&params.ref_id)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Set the value of an input element by ref handle ID. Dispatches input and change events."
    )]
    async fn fill(&self, Parameters(params): Parameters<FillParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("fill") {
            return tool_disabled("fill");
        }
        let code = format!(
            "return window.__VICTAURI__?.fill({}, {})",
            js_string(&params.ref_id),
            js_string(&params.value)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Type text character-by-character into an element, simulating real keyboard events."
    )]
    async fn type_text(&self, Parameters(params): Parameters<TypeTextParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("type_text") {
            return tool_disabled("type_text");
        }
        let code = format!(
            "return window.__VICTAURI__?.type({}, {})",
            js_string(&params.ref_id),
            js_string(&params.text)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get state of all Tauri windows: position, size, visibility, focus, and URL."
    )]
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

    #[tool(
        description = "Get recent IPC calls intercepted by the JS bridge. Returns command names, arguments, results, durations, and status (ok/error/pending)."
    )]
    async fn get_ipc_log(&self, Parameters(params): Parameters<IpcLogParams>) -> CallToolResult {
        let limit_arg = params.limit.map(|l| format!("{l}")).unwrap_or_default();
        let code = if limit_arg.is_empty() {
            "return window.__VICTAURI__?.getIpcLog()".to_string()
        } else {
            format!("return window.__VICTAURI__?.getIpcLog({limit_arg})")
        };
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "List or search all registered Tauri commands with their argument schemas."
    )]
    async fn get_registry(&self, Parameters(params): Parameters<RegistryParams>) -> CallToolResult {
        let commands = match params.query {
            Some(q) => self.state.registry.search(&q),
            None => self.state.registry.list(),
        };
        match serde_json::to_string_pretty(&commands) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Get real-time process memory statistics from the OS (working set, page file usage). On Windows returns detailed metrics; on Linux returns virtual/resident size."
    )]
    async fn get_memory_stats(&self) -> CallToolResult {
        let stats = crate::memory::current_stats();
        match serde_json::to_string_pretty(&stats) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Compare frontend state (evaluated via JS expression) against backend state to detect divergences. Returns a VerificationResult with any mismatches."
    )]
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
            Err(e) => {
                return tool_error(format!(
                    "frontend expression did not return valid JSON: {e}"
                ));
            }
        };

        let result = victauri_core::verify_state(frontend_state, params.backend_state);
        match serde_json::to_string_pretty(&result) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Detect ghost commands — commands invoked from the frontend that have no backend handler, or registered backend commands never called. Reads from the JS-side IPC interception log."
    )]
    async fn detect_ghost_commands(
        &self,
        Parameters(params): Parameters<GhostCommandParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.getIpcLog()";
        let ipc_json = match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(r) => r,
            Err(e) => return tool_error(format!("failed to read IPC log: {e}")),
        };

        let ipc_calls: Vec<serde_json::Value> = serde_json::from_str(&ipc_json).unwrap_or_default();
        let frontend_commands: Vec<String> = ipc_calls
            .iter()
            .filter_map(|c| c.get("command").and_then(|v| v.as_str()).map(String::from))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let report = victauri_core::detect_ghost_commands(&frontend_commands, &self.state.registry);
        match serde_json::to_string_pretty(&report) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Check IPC round-trip integrity: find stale (stuck) pending calls and errored calls. Returns health status and lists of problematic IPC calls."
    )]
    async fn check_ipc_integrity(
        &self,
        Parameters(params): Parameters<IpcIntegrityParams>,
    ) -> CallToolResult {
        let threshold = params.stale_threshold_ms.unwrap_or(5000);
        let code = format!(
            r#"return (function() {{
                var log = window.__VICTAURI__?.getIpcLog() || [];
                var now = Date.now();
                var threshold = {threshold};
                var pending = log.filter(function(c) {{ return c.status === 'pending'; }});
                var stale = pending.filter(function(c) {{ return (now - c.timestamp) > threshold; }});
                var errored = log.filter(function(c) {{ return c.status === 'error'; }});
                return {{
                    healthy: stale.length === 0 && errored.length === 0,
                    total_calls: log.length,
                    pending_count: pending.length,
                    stale_count: stale.length,
                    error_count: errored.length,
                    stale_calls: stale.slice(0, 20),
                    errored_calls: errored.slice(0, 20)
                }};
            }})()"#
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get a combined event stream from the webview: console logs, DOM mutations, sorted by timestamp. Use the 'since' parameter to poll only new events."
    )]
    async fn get_event_stream(
        &self,
        Parameters(params): Parameters<EventStreamParams>,
    ) -> CallToolResult {
        let since_arg = params.since.map(|ts| format!("{ts}")).unwrap_or_default();
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

    #[tool(
        description = "Resolve a natural language query to matching Tauri commands. Returns scored results ranked by relevance, using command names, descriptions, intents, categories, and examples."
    )]
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

    #[tool(
        description = "Run a semantic assertion: evaluate a JS expression and check the result against an expected condition. Conditions: equals, not_equals, contains, greater_than, less_than, truthy, falsy, exists, type_is."
    )]
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

    #[tool(
        description = "Start recording IPC events and state changes. Returns false if a recording is already active."
    )]
    async fn start_recording(
        &self,
        Parameters(params): Parameters<StartRecordingParams>,
    ) -> CallToolResult {
        let session_id = params
            .session_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let started = self.state.recorder.start(session_id.clone());
        let result = serde_json::json!({
            "started": started,
            "session_id": session_id,
        });
        CallToolResult::success(vec![Content::text(result.to_string())])
    }

    #[tool(
        description = "Stop the current recording and return the full recorded session with all events and checkpoints."
    )]
    async fn stop_recording(&self) -> CallToolResult {
        match self.state.recorder.stop() {
            Some(session) => match serde_json::to_string_pretty(&session) {
                Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                Err(e) => tool_error(e.to_string()),
            },
            None => tool_error("no recording is active"),
        }
    }

    #[tool(
        description = "Create a state checkpoint during recording. Associates the current event index with a state snapshot for later comparison."
    )]
    async fn checkpoint(&self, Parameters(params): Parameters<CheckpointParams>) -> CallToolResult {
        let created = self
            .state
            .recorder
            .checkpoint(params.id.clone(), params.label, params.state);
        if created {
            let result = serde_json::json!({
                "created": true,
                "checkpoint_id": params.id,
                "event_index": self.state.recorder.event_count(),
            });
            CallToolResult::success(vec![Content::text(result.to_string())])
        } else {
            tool_error("no recording is active — start one first")
        }
    }

    #[tool(description = "Get all checkpoints from the current recording session.")]
    async fn list_checkpoints(&self) -> CallToolResult {
        let checkpoints = self.state.recorder.get_checkpoints();
        match serde_json::to_string_pretty(&checkpoints) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Get the IPC replay sequence: all IPC calls recorded in order, suitable for replaying the session."
    )]
    async fn get_replay_sequence(&self) -> CallToolResult {
        let calls = self.state.recorder.ipc_replay_sequence();
        match serde_json::to_string_pretty(&calls) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Get recorded events since a specific event index. Useful for incremental replay."
    )]
    async fn get_recorded_events(
        &self,
        Parameters(params): Parameters<ReplayParams>,
    ) -> CallToolResult {
        let events = self
            .state
            .recorder
            .events_since(params.since_index.unwrap_or(0));
        match serde_json::to_string_pretty(&events) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(description = "Get all events that occurred between two checkpoints.")]
    async fn events_between_checkpoints(
        &self,
        Parameters(params): Parameters<EventsBetweenCheckpointsParams>,
    ) -> CallToolResult {
        match self
            .state
            .recorder
            .events_between_checkpoints(&params.from_checkpoint, &params.to_checkpoint)
        {
            Some(events) => match serde_json::to_string_pretty(&events) {
                Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                Err(e) => tool_error(e.to_string()),
            },
            None => tool_error("one or both checkpoint IDs not found"),
        }
    }

    #[tool(
        description = "Export the current recording session as a JSON string. The session can be saved externally and later imported with import_session. Does NOT stop the recording."
    )]
    async fn export_session(&self) -> CallToolResult {
        match self.state.recorder.export() {
            Some(s) => {
                let json = serde_json::to_string_pretty(&s)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                CallToolResult::success(vec![Content::text(json)])
            }
            None => tool_error("no recording is active — start one first"),
        }
    }

    #[tool(
        description = "Import a previously exported recording session from JSON. Useful for replaying sessions across restarts. The imported session can be queried with replay and checkpoint tools."
    )]
    async fn import_session(
        &self,
        Parameters(params): Parameters<ImportSessionParams>,
    ) -> CallToolResult {
        let session: victauri_core::RecordedSession =
            match serde_json::from_str(&params.session_json) {
                Ok(s) => s,
                Err(e) => return tool_error(format!("invalid session JSON: {e}")),
            };

        let result = serde_json::json!({
            "imported": true,
            "session_id": session.id,
            "event_count": session.events.len(),
            "checkpoint_count": session.checkpoints.len(),
            "started_at": session.started_at.to_rfc3339(),
        });
        self.state.recorder.import(session);
        CallToolResult::success(vec![Content::text(result.to_string())])
    }

    #[tool(
        description = "Find slow IPC calls that exceed a time threshold. Returns calls sorted by duration (slowest first). Useful for identifying performance bottlenecks."
    )]
    async fn slow_ipc_calls(
        &self,
        Parameters(params): Parameters<SlowIpcParams>,
    ) -> CallToolResult {
        let limit = params.limit.unwrap_or(20);
        let code = format!(
            r#"return (function() {{
                var log = window.__VICTAURI__?.getIpcLog() || [];
                var slow = log.filter(function(c) {{ return (c.duration_ms || 0) > {threshold}; }});
                slow.sort(function(a, b) {{ return (b.duration_ms || 0) - (a.duration_ms || 0); }});
                return {{ threshold_ms: {threshold}, count: Math.min(slow.length, {limit}), calls: slow.slice(0, {limit}) }};
            }})()"#,
            threshold = params.threshold_ms,
            limit = limit,
        );
        match self.eval_with_return(&code, None).await {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Invoke a registered Tauri command via IPC, just like the frontend would. Goes through the real IPC pipeline so calls are logged and verifiable. Returns the command's result. Subject to privacy command filtering."
    )]
    async fn invoke_command(
        &self,
        Parameters(params): Parameters<InvokeCommandParams>,
    ) -> CallToolResult {
        if !self.state.privacy.is_command_allowed(&params.command) {
            return tool_error(format!(
                "command '{}' is blocked by privacy configuration",
                params.command
            ));
        }
        let args_json = params.args.unwrap_or(serde_json::json!({}));
        let args_str = serde_json::to_string(&args_json).unwrap_or_else(|_| "{}".to_string());
        let code = format!(
            "return window.__TAURI__.core.invoke({}, {args_str})",
            js_string(&params.command)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(format!("invoke_command failed: {e}")),
        }
    }

    #[tool(
        description = "Capture a screenshot of a Tauri window as a base64-encoded PNG image. Currently supported on Windows; other platforms return an error."
    )]
    async fn screenshot(&self, Parameters(params): Parameters<ScreenshotParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("screenshot") {
            return tool_disabled("screenshot");
        }
        match self
            .bridge
            .get_native_handle(params.window_label.as_deref())
        {
            Ok(hwnd) => match crate::screenshot::capture_window(hwnd).await {
                Ok(png_bytes) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                    CallToolResult::success(vec![Content::image(b64, "image/png")])
                }
                Err(e) => tool_error(format!("screenshot capture failed: {e}")),
            },
            Err(e) => tool_error(format!("cannot get window handle: {e}")),
        }
    }

    #[tool(
        description = "Press a keyboard key on the currently focused element. Useful for triggering keyboard shortcuts, submitting forms (Enter), closing dialogs (Escape), or navigating (Tab, ArrowDown)."
    )]
    async fn press_key(&self, Parameters(params): Parameters<PressKeyParams>) -> CallToolResult {
        let code = format!(
            "return window.__VICTAURI__?.pressKey({})",
            js_string(&params.key)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get captured console logs (log, warn, error, info) from the webview. Useful for debugging and monitoring application behavior."
    )]
    async fn get_console_logs(
        &self,
        Parameters(params): Parameters<GetConsoleLogsParams>,
    ) -> CallToolResult {
        let since_arg = params.since.map(|ts| format!("{ts}")).unwrap_or_default();
        let code = if since_arg.is_empty() {
            "return window.__VICTAURI__?.getConsoleLogs()".to_string()
        } else {
            format!("return window.__VICTAURI__?.getConsoleLogs({since_arg})")
        };
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    // ── Extended Interactions ────────────────────────────────────────────────

    #[tool(description = "Double-click an element by its ref handle ID from a DOM snapshot.")]
    async fn double_click(
        &self,
        Parameters(params): Parameters<DoubleClickParams>,
    ) -> CallToolResult {
        let code = format!(
            "return window.__VICTAURI__?.doubleClick({})",
            js_string(&params.ref_id)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Hover over an element by its ref handle ID. Dispatches mouseenter and mouseover events."
    )]
    async fn hover(&self, Parameters(params): Parameters<HoverParams>) -> CallToolResult {
        let code = format!(
            "return window.__VICTAURI__?.hover({})",
            js_string(&params.ref_id)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Select one or more options in a <select> element by their option values."
    )]
    async fn select_option(
        &self,
        Parameters(params): Parameters<SelectOptionParams>,
    ) -> CallToolResult {
        let values_json =
            serde_json::to_string(&params.values).unwrap_or_else(|_| "[]".to_string());
        let code = format!(
            "return window.__VICTAURI__?.selectOption({}, {})",
            js_string(&params.ref_id),
            values_json
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Scroll to an element by ref handle ID (scrolls into view), or to absolute page coordinates if no ref given."
    )]
    async fn scroll_to(&self, Parameters(params): Parameters<ScrollToParams>) -> CallToolResult {
        let ref_arg = params
            .ref_id
            .as_ref()
            .map(|r| js_string(r))
            .unwrap_or_else(|| "null".to_string());
        let x = params.x.unwrap_or(0.0);
        let y = params.y.unwrap_or(0.0);
        let code = format!("return window.__VICTAURI__?.scrollTo({ref_arg}, {x}, {y})");
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Focus an element by its ref handle ID.")]
    async fn focus_element(
        &self,
        Parameters(params): Parameters<FocusElementParams>,
    ) -> CallToolResult {
        let code = format!(
            "return window.__VICTAURI__?.focusElement({})",
            js_string(&params.ref_id)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    // ── Network Monitoring ──────────────────────────────────────────────────

    #[tool(
        description = "Get intercepted network requests (fetch and XMLHttpRequest). Filter by URL substring and limit the number of results."
    )]
    async fn get_network_log(
        &self,
        Parameters(params): Parameters<NetworkLogParams>,
    ) -> CallToolResult {
        let filter_arg = params
            .filter
            .as_ref()
            .map(|f| js_string(f))
            .unwrap_or_else(|| "null".to_string());
        let limit_arg = params
            .limit
            .map(|l| l.to_string())
            .unwrap_or_else(|| "null".to_string());
        let code = format!("return window.__VICTAURI__?.getNetworkLog({filter_arg}, {limit_arg})");
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    // ── Storage ─────────────────────────────────────────────────────────────

    #[tool(
        description = "Read from localStorage or sessionStorage. Returns all entries if no key is specified, or the value for a specific key."
    )]
    async fn get_storage(
        &self,
        Parameters(params): Parameters<GetStorageParams>,
    ) -> CallToolResult {
        let method = match params.storage_type.as_str() {
            "session" => "getSessionStorage",
            _ => "getLocalStorage",
        };
        let key_arg = params
            .key
            .as_ref()
            .map(|k| js_string(k))
            .unwrap_or_default();
        let code = format!("return window.__VICTAURI__?.{method}({key_arg})");
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Set a value in localStorage or sessionStorage.")]
    async fn set_storage(
        &self,
        Parameters(params): Parameters<SetStorageParams>,
    ) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("set_storage") {
            return tool_disabled("set_storage");
        }
        let method = match params.storage_type.as_str() {
            "session" => "setSessionStorage",
            _ => "setLocalStorage",
        };
        let value_json =
            serde_json::to_string(&params.value).unwrap_or_else(|_| "null".to_string());
        let code = format!(
            "return window.__VICTAURI__?.{method}({}, {value_json})",
            js_string(&params.key)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Delete a key from localStorage or sessionStorage.")]
    async fn delete_storage(
        &self,
        Parameters(params): Parameters<DeleteStorageParams>,
    ) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("delete_storage") {
            return tool_disabled("delete_storage");
        }
        let method = match params.storage_type.as_str() {
            "session" => "deleteSessionStorage",
            _ => "deleteLocalStorage",
        };
        let code = format!(
            "return window.__VICTAURI__?.{method}({})",
            js_string(&params.key)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Get all cookies visible to the webview document.")]
    async fn get_cookies(
        &self,
        Parameters(params): Parameters<GetCookiesParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.getCookies()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => self.redact_result(result),
            Err(e) => tool_error(e),
        }
    }

    // ── Navigation ──────────────────────────────────────────────────────────

    #[tool(
        description = "Get the navigation history log — tracks pushState, replaceState, popstate, hashchange, and the initial page load."
    )]
    async fn get_navigation_log(
        &self,
        Parameters(params): Parameters<NavigationLogParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.getNavigationLog()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Navigate the webview to a URL. Blocks javascript:, data:, and vbscript: URLs."
    )]
    async fn navigate(&self, Parameters(params): Parameters<NavigateParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("navigate") {
            return tool_disabled("navigate");
        }
        if let Err(e) = validate_url(&params.url) {
            return tool_error(e);
        }
        let code = format!(
            "return window.__VICTAURI__?.navigate({})",
            js_string(&params.url)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Navigate back in the webview's browser history.")]
    async fn navigate_back(
        &self,
        Parameters(params): Parameters<NavigationLogParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.navigateBack()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    // ── Dialogs ─────────────────────────────────────────────────────────────

    #[tool(
        description = "Get captured dialog events (alert, confirm, prompt). Dialogs are auto-handled: alert is accepted, confirm returns true, prompt returns default value."
    )]
    async fn get_dialog_log(
        &self,
        Parameters(params): Parameters<DialogLogParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.getDialogLog()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Configure automatic responses for browser dialogs. Types: alert, confirm, prompt. Actions: accept, dismiss. For prompt dialogs, optionally set the response text."
    )]
    async fn set_dialog_response(
        &self,
        Parameters(params): Parameters<SetDialogResponseParams>,
    ) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("set_dialog_response") {
            return tool_disabled("set_dialog_response");
        }
        let text_arg = params
            .text
            .as_ref()
            .map(|t| js_string(t))
            .unwrap_or_else(|| "undefined".to_string());
        let code = format!(
            "return window.__VICTAURI__?.setDialogAutoResponse({}, {}, {text_arg})",
            js_string(&params.dialog_type),
            js_string(&params.action)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    // ── Wait ────────────────────────────────────────────────────────────────

    #[tool(
        description = "Wait for a condition to be met. Polls at regular intervals until satisfied or timeout. Conditions: text (text appears), text_gone (text disappears), selector (CSS selector matches), selector_gone, url (URL contains value), ipc_idle (no pending IPC calls), network_idle (no pending network requests)."
    )]
    async fn wait_for(&self, Parameters(params): Parameters<WaitForParams>) -> CallToolResult {
        let value = params
            .value
            .as_ref()
            .map(|v| js_string(v))
            .unwrap_or_else(|| "null".to_string());
        let timeout_ms = params.timeout_ms.unwrap_or(10000);
        let poll = params.poll_ms.unwrap_or(200);
        let code = format!(
            "return window.__VICTAURI__?.waitFor({{ condition: {}, value: {value}, timeout_ms: {timeout_ms}, poll_ms: {poll} }})",
            js_string(&params.condition)
        );
        // Give the Rust-side eval timeout extra headroom beyond the JS-side
        // polling timeout so the JS promise has time to resolve or reject
        // before we forcibly cancel it.
        let eval_timeout = std::time::Duration::from_millis(timeout_ms + 5000);
        match self
            .eval_with_return_timeout(&code, params.webview_label.as_deref(), eval_timeout)
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    // ── Window Management ───────────────────────────────────────────────────

    #[tool(
        description = "Manage a window: minimize, unminimize, maximize, unmaximize, close, focus, show, hide, fullscreen, unfullscreen, always_on_top, not_always_on_top."
    )]
    async fn manage_window(
        &self,
        Parameters(params): Parameters<ManageWindowParams>,
    ) -> CallToolResult {
        match self
            .bridge
            .manage_window(params.label.as_deref(), &params.action)
        {
            Ok(msg) => CallToolResult::success(vec![Content::text(msg)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Resize a window to the specified width and height in logical pixels.")]
    async fn resize_window(
        &self,
        Parameters(params): Parameters<ResizeWindowParams>,
    ) -> CallToolResult {
        match self
            .bridge
            .resize_window(params.label.as_deref(), params.width, params.height)
        {
            Ok(()) => {
                let result =
                    serde_json::json!({"ok": true, "width": params.width, "height": params.height});
                CallToolResult::success(vec![Content::text(result.to_string())])
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Move a window to the specified screen position (x, y) in logical pixels."
    )]
    async fn move_window(
        &self,
        Parameters(params): Parameters<MoveWindowParams>,
    ) -> CallToolResult {
        match self
            .bridge
            .move_window(params.label.as_deref(), params.x, params.y)
        {
            Ok(()) => {
                let result = serde_json::json!({"ok": true, "x": params.x, "y": params.y});
                CallToolResult::success(vec![Content::text(result.to_string())])
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Set the title of a window.")]
    async fn set_window_title(
        &self,
        Parameters(params): Parameters<SetWindowTitleParams>,
    ) -> CallToolResult {
        match self
            .bridge
            .set_window_title(params.label.as_deref(), &params.title)
        {
            Ok(()) => {
                let result = serde_json::json!({"ok": true, "title": params.title});
                CallToolResult::success(vec![Content::text(result.to_string())])
            }
            Err(e) => tool_error(e),
        }
    }

    // ── Phase 8: Deep Introspection ─────────────────────────────────────────

    #[tool(
        description = "Get computed CSS styles for an element. Returns key properties by default, or specific properties if listed."
    )]
    async fn get_styles(&self, Parameters(params): Parameters<GetStylesParams>) -> CallToolResult {
        let props_arg = match &params.properties {
            Some(props) => {
                let arr: Vec<String> = props.iter().map(|p| js_string(p)).collect();
                format!("[{}]", arr.join(","))
            }
            None => "null".to_string(),
        };
        let code = format!(
            "return window.__VICTAURI__?.getStyles({}, {})",
            js_string(&params.ref_id),
            props_arg
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get precise bounding boxes with CSS box model (margin, padding, border) for one or more elements."
    )]
    async fn get_bounding_boxes(
        &self,
        Parameters(params): Parameters<GetBoundingBoxesParams>,
    ) -> CallToolResult {
        let refs: Vec<String> = params.ref_ids.iter().map(|r| js_string(r)).collect();
        let code = format!(
            "return window.__VICTAURI__?.getBoundingBoxes([{}])",
            refs.join(",")
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Draw a colored overlay on an element for visual debugging. The overlay is fixed-position and non-interactive."
    )]
    async fn highlight_element(
        &self,
        Parameters(params): Parameters<HighlightElementParams>,
    ) -> CallToolResult {
        let color_arg = match &params.color {
            Some(c) => match sanitize_css_color(c) {
                Ok(safe) => format!("\"{}\"", safe),
                Err(e) => return tool_error(e),
            },
            None => "null".to_string(),
        };
        let label_arg = match &params.label {
            Some(l) => js_string(l),
            None => "null".to_string(),
        };
        let code = format!(
            "return window.__VICTAURI__?.highlightElement({}, {}, {})",
            js_string(&params.ref_id),
            color_arg,
            label_arg
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Remove all debug highlight overlays from the page.")]
    async fn clear_highlights(
        &self,
        Parameters(params): Parameters<ClearHighlightsParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.clearHighlights()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Inject custom CSS into the page. Replaces any previously injected CSS. Useful for debugging layout issues or prototyping style changes."
    )]
    async fn inject_css(&self, Parameters(params): Parameters<InjectCssParams>) -> CallToolResult {
        if !self.state.privacy.is_tool_enabled("inject_css") {
            return tool_disabled("inject_css");
        }
        let code = format!(
            "return window.__VICTAURI__?.injectCss({})",
            js_string(&params.css)
        );
        match self
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Remove previously injected CSS from the page.")]
    async fn remove_injected_css(
        &self,
        Parameters(params): Parameters<RemoveInjectedCssParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.removeInjectedCss()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Run a comprehensive accessibility audit. Checks for missing alt text, unlabeled form inputs, empty buttons/links, heading hierarchy, color contrast, ARIA role validity, and more. Returns violations and warnings with severity levels."
    )]
    async fn audit_accessibility(
        &self,
        Parameters(params): Parameters<AuditAccessibilityParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.auditAccessibility()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get performance metrics: navigation timing, resource loading summary, paint timing, JS heap usage, long task count, and DOM statistics."
    )]
    async fn get_performance_metrics(
        &self,
        Parameters(params): Parameters<GetPerformanceMetricsParams>,
    ) -> CallToolResult {
        let code = "return window.__VICTAURI__?.getPerformanceMetrics()";
        match self
            .eval_with_return(code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Inspect the Victauri plugin's own configuration: port, enabled/disabled tools, command filters, privacy settings, capacities, and version. Useful for agents to understand their capabilities before acting."
    )]
    async fn get_plugin_info(&self) -> CallToolResult {
        let disabled: Vec<&str> = self
            .state
            .privacy
            .disabled_tools
            .iter()
            .map(|s| s.as_str())
            .collect();
        let blocklist: Vec<&str> = self
            .state
            .privacy
            .command_blocklist
            .iter()
            .map(|s| s.as_str())
            .collect();
        let allowlist: Option<Vec<&str>> = self
            .state
            .privacy
            .command_allowlist
            .as_ref()
            .map(|s| s.iter().map(|s| s.as_str()).collect());
        let all_tools = Self::tool_router().list_all();
        let enabled_tools: Vec<&str> = all_tools
            .iter()
            .filter(|t| self.state.privacy.is_tool_enabled(t.name.as_ref()))
            .map(|t| t.name.as_ref())
            .collect();

        let result = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "bridge_version": BRIDGE_VERSION,
            "port": self.state.port,
            "tools": {
                "total": all_tools.len(),
                "enabled": enabled_tools.len(),
                "enabled_list": enabled_tools,
                "disabled_list": disabled,
            },
            "commands": {
                "allowlist": allowlist,
                "blocklist": blocklist,
            },
            "privacy": {
                "redaction_enabled": self.state.privacy.redaction_enabled,
            },
            "capacities": {
                "event_log": self.state.event_log.capacity(),
                "eval_timeout_secs": self.state.eval_timeout.as_secs(),
            },
            "registered_commands": self.state.registry.count(),
            "tool_invocations": self.state.tool_invocations.load(std::sync::atomic::Ordering::Relaxed),
            "uptime_secs": self.state.started_at.elapsed().as_secs(),
        });
        match serde_json::to_string_pretty(&result) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }
}

impl VictauriMcpHandler {
    pub fn new(state: Arc<VictauriState>, bridge: Arc<dyn WebviewBridge>) -> Self {
        Self {
            state,
            bridge,
            subscriptions: Arc::new(Mutex::new(HashSet::new())),
            bridge_checked: Arc::new(AtomicBool::new(false)),
        }
    }

    fn redact_result(&self, output: String) -> CallToolResult {
        let redacted = self.state.privacy.redact_output(&output);
        CallToolResult::success(vec![Content::text(redacted)])
    }

    async fn eval_with_return(
        &self,
        code: &str,
        webview_label: Option<&str>,
    ) -> Result<String, String> {
        self.eval_with_return_timeout(code, webview_label, self.state.eval_timeout)
            .await
    }

    async fn eval_with_return_timeout(
        &self,
        code: &str,
        webview_label: Option<&str>,
        timeout: std::time::Duration,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut pending = self.state.pending_evals.lock().await;
            if pending.len() >= MAX_PENDING_EVALS {
                return Err(format!(
                    "too many concurrent eval requests (limit: {MAX_PENDING_EVALS})"
                ));
            }
            pending.insert(id.clone(), tx);
        }

        // Auto-prepend `return` so bare expressions produce a value.
        // Only skip for code that starts with a statement keyword where
        // prepending `return` would be a syntax error.
        let code = code.trim();
        let needs_return = !code.starts_with("return ")
            && !code.starts_with("return;")
            && !code.starts_with('{')
            && !code.starts_with("if ")
            && !code.starts_with("if(")
            && !code.starts_with("for ")
            && !code.starts_with("for(")
            && !code.starts_with("while ")
            && !code.starts_with("while(")
            && !code.starts_with("switch ")
            && !code.starts_with("try ")
            && !code.starts_with("const ")
            && !code.starts_with("let ")
            && !code.starts_with("var ")
            && !code.starts_with("function ")
            && !code.starts_with("class ")
            && !code.starts_with("throw ");
        let code = if needs_return {
            format!("return {code}")
        } else {
            code.to_string()
        };

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

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => {
                self.check_bridge_version_once();
                Ok(result)
            }
            Ok(Err(_)) => Err("eval callback channel closed".to_string()),
            Err(_) => {
                self.state.pending_evals.lock().await.remove(&id);
                Err(format!("eval timed out after {}s", timeout.as_secs()))
            }
        }
    }

    fn check_bridge_version_once(&self) {
        if self.bridge_checked.swap(true, Ordering::Relaxed) {
            return;
        }
        let handler = self.clone();
        tokio::spawn(async move {
            match handler
                .eval_with_return_timeout(
                    "window.__VICTAURI__?.version",
                    None,
                    std::time::Duration::from_secs(5),
                )
                .await
            {
                Ok(v) => {
                    let v = v.trim_matches('"');
                    if v != BRIDGE_VERSION {
                        tracing::warn!(
                            "Bridge version mismatch: Rust expects {BRIDGE_VERSION}, JS reports {v}"
                        );
                    } else {
                        tracing::debug!("Bridge version verified: {v}");
                    }
                }
                Err(e) => tracing::debug!("Bridge version check skipped: {e}"),
            }
        });
    }
}

const SERVER_INSTRUCTIONS: &str = "Victauri gives you X-ray vision and hands inside a running Tauri application. \
You can evaluate JS, snapshot the DOM, interact with elements (click, double-click, \
hover, fill, type, select, scroll, focus), press keys, invoke Tauri commands, \
capture screenshots, manage windows (minimize, maximize, resize, move, close), \
view IPC and network traffic, read/write browser storage, track navigation history, \
handle dialogs, wait for conditions, search the command registry, monitor process memory, \
record and replay sessions, inspect computed CSS styles, measure element bounding boxes, \
draw debug overlays on elements, inject custom CSS, run accessibility audits (alt text, \
labels, contrast, ARIA, heading hierarchy), get performance metrics (navigation timing, \
resource loading, JS heap, long tasks, DOM stats), and subscribe to live resource \
streams — all through MCP.";

impl ServerHandler for VictauriMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .build(),
        )
        .with_instructions(SERVER_INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let all_tools = Self::tool_router().list_all();
        let filtered: Vec<Tool> = all_tools
            .into_iter()
            .filter(|t| self.state.privacy.is_tool_enabled(t.name.as_ref()))
            .collect();
        Ok(ListToolsResult {
            tools: filtered,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name: String = request.name.as_ref().to_owned();
        if !self.state.privacy.is_tool_enabled(&tool_name) {
            tracing::debug!(tool = %tool_name, "tool call blocked by privacy config");
            return Ok(tool_disabled(&tool_name));
        }
        self.state
            .tool_invocations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let start = std::time::Instant::now();
        tracing::debug!(tool = %tool_name, "tool invocation started");
        let ctx = ToolCallContext::new(self, request, context);
        let result = Self::tool_router().call(ctx).await;
        let elapsed = start.elapsed();
        tracing::debug!(
            tool = %tool_name,
            elapsed_ms = elapsed.as_millis() as u64,
            is_error = result.as_ref().map(|r| r.is_error.unwrap_or(false)).unwrap_or(true),
            "tool invocation completed"
        );
        result
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        if !self.state.privacy.is_tool_enabled(name) {
            return None;
        }
        Self::tool_router().get(name).cloned()
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new(RESOURCE_URI_IPC_LOG, "ipc-log")
                    .with_description(
                        "Live IPC call log — all commands invoked between frontend and backend",
                    )
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new(RESOURCE_URI_WINDOWS, "windows")
                    .with_description(
                        "Current state of all Tauri windows — position, size, visibility, focus",
                    )
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new(RESOURCE_URI_STATE, "state")
                    .with_description(
                        "Victauri plugin state — event count, registered commands, memory stats",
                    )
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
                match self
                    .eval_with_return("return window.__VICTAURI__?.getIpcLog()", None)
                    .await
                {
                    Ok(json) => json,
                    Err(_) => {
                        let calls = self.state.event_log.ipc_calls();
                        serde_json::to_string_pretty(&calls)
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                    }
                }
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

        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            json, uri,
        )]))
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

// ── Server startup ───────────────────────────────────────────────────────────

pub fn build_app(state: Arc<VictauriState>, bridge: Arc<dyn WebviewBridge>) -> axum::Router {
    build_app_with_options(state, bridge, None)
}

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
                        "port": s.port,
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

pub async fn start_server<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    start_server_with_options(app_handle, state, port, None, shutdown_rx).await
}

pub async fn start_server_with_options<R: Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: Arc<VictauriState>,
    port: u16,
    auth_token: Option<String>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let bridge: Arc<dyn WebviewBridge> = Arc::new(app_handle);
    let app = build_app_with_options(state, bridge, auth_token);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!("Victauri MCP server listening on 127.0.0.1:{port}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.wait_for(|&v| v).await;
            tracing::info!("Victauri MCP server shutting down gracefully");
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_simple() {
        assert_eq!(js_string("hello"), "\"hello\"");
    }

    #[test]
    fn js_string_single_quotes() {
        let result = js_string("it's a test");
        assert!(result.contains("it's a test"));
    }

    #[test]
    fn js_string_double_quotes() {
        let result = js_string(r#"say "hello""#);
        assert!(result.contains(r#"\""#));
    }

    #[test]
    fn js_string_backslashes() {
        let result = js_string(r"path\to\file");
        assert!(result.contains(r"\\"));
    }

    #[test]
    fn js_string_newlines_and_tabs() {
        let result = js_string("line1\nline2\ttab");
        assert!(result.contains(r"\n"));
        assert!(result.contains(r"\t"));
        assert!(!result.contains('\n'));
    }

    #[test]
    fn js_string_null_bytes() {
        let input = String::from_utf8(b"before\x00after".to_vec()).unwrap();
        let result = js_string(&input);
        // serde_json escapes null bytes as  
        assert!(result.contains("\\u0000"));
        assert!(!result.contains('\0'));
    }

    #[test]
    fn js_string_template_literal_injection() {
        let result = js_string("`${alert(1)}`");
        // Should not contain unescaped backticks that could break template literals
        // serde_json wraps in double quotes, so backticks are safe
        assert!(result.starts_with('"'));
        assert!(result.ends_with('"'));
    }

    #[test]
    fn js_string_unicode_separators() {
        // U+2028 (Line Separator) and U+2029 (Paragraph Separator) are valid in
        // JSON strings per RFC 8259, and serde_json passes them through literally.
        // Since js_string is used inside JS double-quoted strings (not template
        // literals), they are safe in modern JS engines (ES2019+).
        let result = js_string("a\u{2028}b\u{2029}c");
        // Verify the string is valid JSON that round-trips correctly
        let decoded: String = serde_json::from_str(&result).unwrap();
        assert_eq!(decoded, "a\u{2028}b\u{2029}c");
    }

    #[test]
    fn js_string_empty() {
        assert_eq!(js_string(""), "\"\"");
    }

    #[test]
    fn js_string_html_script_close() {
        // </script> in a JS string inside HTML could break out of script tags
        let result = js_string("</script><img onerror=alert(1)>");
        assert!(result.starts_with('"'));
        // The string is JSON-encoded; verify it round-trips safely
        let decoded: String = serde_json::from_str(&result).unwrap();
        assert_eq!(decoded, "</script><img onerror=alert(1)>");
    }

    #[test]
    fn js_string_very_long() {
        let long = "a".repeat(100_000);
        let result = js_string(&long);
        assert!(result.len() >= 100_002); // quotes + content
    }

    // ── URL validation tests ────────────────────────────────────────────────

    #[test]
    fn url_allows_http() {
        assert!(validate_url("http://example.com").is_ok());
    }

    #[test]
    fn url_allows_https() {
        assert!(validate_url("https://example.com/path?q=1").is_ok());
    }

    #[test]
    fn url_allows_file() {
        assert!(validate_url("file:///tmp/test.html").is_ok());
    }

    #[test]
    fn url_blocks_javascript() {
        assert!(validate_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn url_blocks_javascript_case_insensitive() {
        assert!(validate_url("JAVASCRIPT:alert(1)").is_err());
    }

    #[test]
    fn url_blocks_data_scheme() {
        assert!(validate_url("data:text/html,<script>alert(1)</script>").is_err());
    }

    #[test]
    fn url_blocks_vbscript() {
        assert!(validate_url("vbscript:MsgBox(1)").is_err());
    }

    #[test]
    fn url_rejects_invalid() {
        assert!(validate_url("not a url at all").is_err());
    }

    #[test]
    fn url_strips_control_chars() {
        // Control characters should be stripped, leaving a valid URL
        let input = format!("http://example{}com", '\0');
        assert!(validate_url(&input).is_ok());
    }
}
