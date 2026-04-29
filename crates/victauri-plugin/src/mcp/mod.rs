mod backend_params;
mod compound_params;
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
pub use compound_params::*;
pub use introspection_params::*;
pub use other_params::{DialogLogParams, DeleteStorageParams, EventStreamParams, GetCookiesParams, GetStorageParams, NavigationLogParams, ResolveCommandParams, SemanticAssertParams, SetDialogResponseParams, SetStorageParams, WaitForParams};
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
    // ── Standalone Tools ────────────────────────────────────────────────────

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
            .eval_with_return(&code, params.webview_label.as_deref())
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
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
        self.track_tool_call();
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
            .eval_with_return(&code, params.webview_label.as_deref())
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
        let eval_timeout = std::time::Duration::from_millis(timeout_ms + 5000);
        match self
            .eval_with_return_timeout(&code, params.webview_label.as_deref(), eval_timeout)
            .await
        {
            Ok(result) => CallToolResult::success(vec![Content::text(result)]),
            Err(e) => tool_error(e),
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
        description = "Resolve a natural language query to matching Tauri commands. Returns scored results ranked by relevance, using command names, descriptions, intents, categories, and examples."
    )]
    async fn resolve_command(
        &self,
        Parameters(params): Parameters<ResolveCommandParams>,
    ) -> CallToolResult {
        self.track_tool_call();
        let limit = params.limit.unwrap_or(5);
        let mut results = self.state.registry.resolve(&params.query);
        results.truncate(limit);
        match serde_json::to_string_pretty(&results) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "List or search all registered Tauri commands with their argument schemas."
    )]
    async fn get_registry(&self, Parameters(params): Parameters<RegistryParams>) -> CallToolResult {
        self.track_tool_call();
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
        self.track_tool_call();
        let stats = crate::memory::current_stats();
        match serde_json::to_string_pretty(&stats) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Inspect the Victauri plugin's own configuration: port, enabled/disabled tools, command filters, privacy settings, capacities, and version. Useful for agents to understand their capabilities before acting."
    )]
    async fn get_plugin_info(&self) -> CallToolResult {
        self.track_tool_call();
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
            "port": self.state.port.load(Ordering::Relaxed),
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

    // ── Compound Tools ──────────────────────────────────────────────────────

    #[tool(
        description = "DOM element interactions. Actions: click, double_click, hover, focus, scroll_into_view, select_option. Requires ref_id from a dom_snapshot for most actions."
    )]
    async fn interact(&self, Parameters(params): Parameters<InteractParams>) -> CallToolResult {
        match params.action.as_str() {
            "click" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for click"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.click({})",
                    js_string(ref_id)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "double_click" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for double_click"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.doubleClick({})",
                    js_string(ref_id)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "hover" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for hover"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.hover({})",
                    js_string(ref_id)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "focus" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for focus"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.focusElement({})",
                    js_string(ref_id)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "scroll_into_view" => {
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
            "select_option" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for select_option"),
                };
                let values = params.values.as_deref().unwrap_or(&[]);
                let values_json =
                    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string());
                let code = format!(
                    "return window.__VICTAURI__?.selectOption({}, {})",
                    js_string(ref_id),
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
            other => tool_error(format!(
                "unknown interact action '{other}'; expected: click, double_click, hover, focus, scroll_into_view, select_option"
            )),
        }
    }

    #[tool(
        description = "Text and keyboard input. Actions: fill (set input value), type_text (character-by-character typing), press_key (trigger a keyboard key). Subject to privacy controls."
    )]
    async fn input(&self, Parameters(params): Parameters<InputParams>) -> CallToolResult {
        match params.action.as_str() {
            "fill" => {
                if !self.state.privacy.is_tool_enabled("fill") {
                    return tool_disabled("fill");
                }
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for fill"),
                };
                let value = match &params.value {
                    Some(v) => v,
                    None => return tool_error("value is required for fill"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.fill({}, {})",
                    js_string(ref_id),
                    js_string(value)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "type_text" => {
                if !self.state.privacy.is_tool_enabled("type_text") {
                    return tool_disabled("type_text");
                }
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for type_text"),
                };
                let text = match &params.text {
                    Some(t) => t,
                    None => return tool_error("text is required for type_text"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.type({}, {})",
                    js_string(ref_id),
                    js_string(text)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "press_key" => {
                let key = match &params.key {
                    Some(k) => k,
                    None => return tool_error("key is required for press_key"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.pressKey({})",
                    js_string(key)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown input action '{other}'; expected: fill, type_text, press_key"
            )),
        }
    }

    #[tool(
        description = "Window management. Actions: get_state (window positions/sizes/visibility), list (all window labels), manage (minimize/maximize/close/focus/show/hide/fullscreen/always_on_top), resize, move_to, set_title."
    )]
    async fn window(&self, Parameters(params): Parameters<WindowParams>) -> CallToolResult {
        self.track_tool_call();
        match params.action.as_str() {
            "get_state" => {
                let states = self.bridge.get_window_states(params.label.as_deref());
                match serde_json::to_string_pretty(&states) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                }
            }
            "list" => {
                let labels = self.bridge.list_window_labels();
                match serde_json::to_string_pretty(&labels) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                }
            }
            "manage" => {
                let manage_action = match &params.manage_action {
                    Some(a) => a,
                    None => return tool_error("manage_action is required for manage"),
                };
                match self
                    .bridge
                    .manage_window(params.label.as_deref(), manage_action)
                {
                    Ok(msg) => CallToolResult::success(vec![Content::text(msg)]),
                    Err(e) => tool_error(e),
                }
            }
            "resize" => {
                let width = match params.width {
                    Some(w) => w,
                    None => return tool_error("width is required for resize"),
                };
                let height = match params.height {
                    Some(h) => h,
                    None => return tool_error("height is required for resize"),
                };
                match self
                    .bridge
                    .resize_window(params.label.as_deref(), width, height)
                {
                    Ok(()) => {
                        let result = serde_json::json!({"ok": true, "width": width, "height": height});
                        CallToolResult::success(vec![Content::text(result.to_string())])
                    }
                    Err(e) => tool_error(e),
                }
            }
            "move_to" => {
                let x = match params.x {
                    Some(v) => v,
                    None => return tool_error("x is required for move_to"),
                };
                let y = match params.y {
                    Some(v) => v,
                    None => return tool_error("y is required for move_to"),
                };
                match self
                    .bridge
                    .move_window(params.label.as_deref(), x, y)
                {
                    Ok(()) => {
                        let result = serde_json::json!({"ok": true, "x": x, "y": y});
                        CallToolResult::success(vec![Content::text(result.to_string())])
                    }
                    Err(e) => tool_error(e),
                }
            }
            "set_title" => {
                let title = match &params.title {
                    Some(t) => t,
                    None => return tool_error("title is required for set_title"),
                };
                match self
                    .bridge
                    .set_window_title(params.label.as_deref(), title)
                {
                    Ok(()) => {
                        let result = serde_json::json!({"ok": true, "title": title});
                        CallToolResult::success(vec![Content::text(result.to_string())])
                    }
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown window action '{other}'; expected: get_state, list, manage, resize, move_to, set_title"
            )),
        }
    }

    #[tool(
        description = "Browser storage operations. Actions: get (read localStorage/sessionStorage), set (write), delete (remove key), get_cookies. Subject to privacy controls for set and delete."
    )]
    async fn storage(&self, Parameters(params): Parameters<StorageParams>) -> CallToolResult {
        match params.action.as_str() {
            "get" => {
                let storage_type = params.storage_type.as_deref().unwrap_or("local");
                let method = match storage_type {
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
            "set" => {
                if !self.state.privacy.is_tool_enabled("set_storage") {
                    return tool_disabled("set_storage");
                }
                let storage_type = params.storage_type.as_deref().unwrap_or("local");
                let method = match storage_type {
                    "session" => "setSessionStorage",
                    _ => "setLocalStorage",
                };
                let key = match &params.key {
                    Some(k) => k,
                    None => return tool_error("key is required for set"),
                };
                let value = params.value.as_ref().cloned().unwrap_or(serde_json::Value::Null);
                let value_json =
                    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
                let code = format!(
                    "return window.__VICTAURI__?.{method}({}, {value_json})",
                    js_string(key)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "delete" => {
                if !self.state.privacy.is_tool_enabled("delete_storage") {
                    return tool_disabled("delete_storage");
                }
                let storage_type = params.storage_type.as_deref().unwrap_or("local");
                let method = match storage_type {
                    "session" => "deleteSessionStorage",
                    _ => "deleteLocalStorage",
                };
                let key = match &params.key {
                    Some(k) => k,
                    None => return tool_error("key is required for delete"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.{method}({})",
                    js_string(key)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "get_cookies" => {
                let code = "return window.__VICTAURI__?.getCookies()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => self.redact_result(result),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown storage action '{other}'; expected: get, set, delete, get_cookies"
            )),
        }
    }

    #[tool(
        description = "Navigation and dialog control. Actions: go_to (navigate to URL), go_back (browser back), get_history (navigation log), set_dialog_response (auto-respond to alert/confirm/prompt), get_dialog_log (captured dialog events). Subject to privacy controls for go_to and set_dialog_response."
    )]
    async fn navigate(&self, Parameters(params): Parameters<NavigateParams>) -> CallToolResult {
        match params.action.as_str() {
            "go_to" => {
                if !self.state.privacy.is_tool_enabled("navigate") {
                    return tool_disabled("navigate");
                }
                let url = match &params.url {
                    Some(u) => u,
                    None => return tool_error("url is required for go_to"),
                };
                if let Err(e) = validate_url(url) {
                    return tool_error(e);
                }
                let code = format!(
                    "return window.__VICTAURI__?.navigate({})",
                    js_string(url)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "go_back" => {
                let code = "return window.__VICTAURI__?.navigateBack()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "get_history" => {
                let code = "return window.__VICTAURI__?.getNavigationLog()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "set_dialog_response" => {
                if !self.state.privacy.is_tool_enabled("set_dialog_response") {
                    return tool_disabled("set_dialog_response");
                }
                let dialog_type = match &params.dialog_type {
                    Some(t) => t,
                    None => return tool_error("dialog_type is required for set_dialog_response"),
                };
                let dialog_action = match &params.dialog_action {
                    Some(a) => a,
                    None => return tool_error("dialog_action is required for set_dialog_response"),
                };
                let text_arg = params
                    .text
                    .as_ref()
                    .map(|t| js_string(t))
                    .unwrap_or_else(|| "undefined".to_string());
                let code = format!(
                    "return window.__VICTAURI__?.setDialogAutoResponse({}, {}, {text_arg})",
                    js_string(dialog_type),
                    js_string(dialog_action)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "get_dialog_log" => {
                let code = "return window.__VICTAURI__?.getDialogLog()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown navigate action '{other}'; expected: go_to, go_back, get_history, set_dialog_response, get_dialog_log"
            )),
        }
    }

    #[tool(
        description = "Time-travel recording. Actions: start (begin recording), stop (end and return session), checkpoint (save state snapshot), list_checkpoints, get_events (since index), events_between (two checkpoints), get_replay (IPC replay sequence), export (session as JSON), import (load session from JSON)."
    )]
    async fn recording(&self, Parameters(params): Parameters<RecordingParams>) -> CallToolResult {
        self.track_tool_call();
        match params.action.as_str() {
            "start" => {
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
            "stop" => match self.state.recorder.stop() {
                Some(session) => match serde_json::to_string_pretty(&session) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                },
                None => tool_error("no recording is active"),
            },
            "checkpoint" => {
                let id = match params.checkpoint_id {
                    Some(id) => id,
                    None => return tool_error("checkpoint_id is required for checkpoint"),
                };
                let state = params.state.unwrap_or(serde_json::Value::Null);
                let created = self
                    .state
                    .recorder
                    .checkpoint(id.clone(), params.checkpoint_label, state);
                if created {
                    let result = serde_json::json!({
                        "created": true,
                        "checkpoint_id": id,
                        "event_index": self.state.recorder.event_count(),
                    });
                    CallToolResult::success(vec![Content::text(result.to_string())])
                } else {
                    tool_error("no recording is active — start one first")
                }
            }
            "list_checkpoints" => {
                let checkpoints = self.state.recorder.get_checkpoints();
                match serde_json::to_string_pretty(&checkpoints) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                }
            }
            "get_events" => {
                let events = self
                    .state
                    .recorder
                    .events_since(params.since_index.unwrap_or(0));
                match serde_json::to_string_pretty(&events) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                }
            }
            "events_between" => {
                let from = match &params.from {
                    Some(f) => f,
                    None => return tool_error("from is required for events_between"),
                };
                let to = match &params.to {
                    Some(t) => t,
                    None => return tool_error("to is required for events_between"),
                };
                match self.state.recorder.events_between_checkpoints(from, to) {
                    Some(events) => match serde_json::to_string_pretty(&events) {
                        Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                        Err(e) => tool_error(e.to_string()),
                    },
                    None => tool_error("one or both checkpoint IDs not found"),
                }
            }
            "get_replay" => {
                let calls = self.state.recorder.ipc_replay_sequence();
                match serde_json::to_string_pretty(&calls) {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => tool_error(e.to_string()),
                }
            }
            "export" => match self.state.recorder.export() {
                Some(s) => {
                    let json = serde_json::to_string_pretty(&s)
                        .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                    CallToolResult::success(vec![Content::text(json)])
                }
                None => tool_error("no recording is active — start one first"),
            },
            "import" => {
                let session_json = match &params.session_json {
                    Some(j) => j,
                    None => return tool_error("session_json is required for import"),
                };
                let session: victauri_core::RecordedSession =
                    match serde_json::from_str(session_json) {
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
            other => tool_error(format!(
                "unknown recording action '{other}'; expected: start, stop, checkpoint, list_checkpoints, get_events, events_between, get_replay, export, import"
            )),
        }
    }

    #[tool(
        description = "CSS and visual inspection. Actions: get_styles (computed CSS for element), get_bounding_boxes (layout rects), highlight (debug overlay), clear_highlights, audit_accessibility (a11y audit), get_performance (timing/heap/DOM metrics)."
    )]
    async fn inspect(&self, Parameters(params): Parameters<InspectParams>) -> CallToolResult {
        match params.action.as_str() {
            "get_styles" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for get_styles"),
                };
                let props_arg = match &params.properties {
                    Some(props) => {
                        let arr: Vec<String> = props.iter().map(|p| js_string(p)).collect();
                        format!("[{}]", arr.join(","))
                    }
                    None => "null".to_string(),
                };
                let code = format!(
                    "return window.__VICTAURI__?.getStyles({}, {})",
                    js_string(ref_id),
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
            "get_bounding_boxes" => {
                let ref_ids = match &params.ref_ids {
                    Some(ids) => ids,
                    None => return tool_error("ref_ids is required for get_bounding_boxes"),
                };
                let refs: Vec<String> = ref_ids.iter().map(|r| js_string(r)).collect();
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
            "highlight" => {
                let ref_id = match &params.ref_id {
                    Some(r) => r,
                    None => return tool_error("ref_id is required for highlight"),
                };
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
                    js_string(ref_id),
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
            "clear_highlights" => {
                let code = "return window.__VICTAURI__?.clearHighlights()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "audit_accessibility" => {
                let code = "return window.__VICTAURI__?.auditAccessibility()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "get_performance" => {
                let code = "return window.__VICTAURI__?.getPerformanceMetrics()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown inspect action '{other}'; expected: get_styles, get_bounding_boxes, highlight, clear_highlights, audit_accessibility, get_performance"
            )),
        }
    }

    #[tool(
        description = "CSS injection. Actions: inject (add custom CSS to page), remove (remove previously injected CSS). Subject to privacy controls."
    )]
    async fn css(&self, Parameters(params): Parameters<CssParams>) -> CallToolResult {
        match params.action.as_str() {
            "inject" => {
                if !self.state.privacy.is_tool_enabled("inject_css") {
                    return tool_disabled("inject_css");
                }
                let css = match &params.css {
                    Some(c) => c,
                    None => return tool_error("css is required for inject"),
                };
                let code = format!(
                    "return window.__VICTAURI__?.injectCss({})",
                    js_string(css)
                );
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "remove" => {
                let code = "return window.__VICTAURI__?.removeInjectedCss()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown css action '{other}'; expected: inject, remove"
            )),
        }
    }

    #[tool(
        description = "Application logs and monitoring. Actions: console (captured console.log/warn/error), network (intercepted fetch/XHR), ipc (IPC call log), navigation (URL change history), dialogs (alert/confirm/prompt events), events (combined event stream), slow_ipc (find slow IPC calls)."
    )]
    async fn logs(&self, Parameters(params): Parameters<LogsParams>) -> CallToolResult {
        match params.action.as_str() {
            "console" => {
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
            "network" => {
                let filter_arg = params
                    .filter
                    .as_ref()
                    .map(|f| js_string(f))
                    .unwrap_or_else(|| "null".to_string());
                let limit_arg = params
                    .limit
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "null".to_string());
                let code =
                    format!("return window.__VICTAURI__?.getNetworkLog({filter_arg}, {limit_arg})");
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => self.redact_result(result),
                    Err(e) => tool_error(e),
                }
            }
            "ipc" => {
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
            "navigation" => {
                let code = "return window.__VICTAURI__?.getNavigationLog()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "dialogs" => {
                let code = "return window.__VICTAURI__?.getDialogLog()";
                match self
                    .eval_with_return(&code, params.webview_label.as_deref())
                    .await
                {
                    Ok(result) => CallToolResult::success(vec![Content::text(result)]),
                    Err(e) => tool_error(e),
                }
            }
            "events" => {
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
            "slow_ipc" => {
                let threshold = match params.threshold_ms {
                    Some(t) => t,
                    None => return tool_error("threshold_ms is required for slow_ipc"),
                };
                let limit = params.limit.unwrap_or(20);
                let code = format!(
                    r#"return (function() {{
                        var log = window.__VICTAURI__?.getIpcLog() || [];
                        var slow = log.filter(function(c) {{ return (c.duration_ms || 0) > {threshold}; }});
                        slow.sort(function(a, b) {{ return (b.duration_ms || 0) - (a.duration_ms || 0); }});
                        return {{ threshold_ms: {threshold}, count: Math.min(slow.length, {limit}), calls: slow.slice(0, {limit}) }};
                    }})()"#,
                );
                match self.eval_with_return(&code, None).await {
                    Ok(result) => self.redact_result(result),
                    Err(e) => tool_error(e),
                }
            }
            other => tool_error(format!(
                "unknown logs action '{other}'; expected: console, network, ipc, navigation, dialogs, events, slow_ipc"
            )),
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

    fn track_tool_call(&self) {
        self.state.tool_invocations.fetch_add(1, Ordering::Relaxed);
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
        self.track_tool_call();
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
Use compound tools with an 'action' parameter to interact with the app: \
'interact' (click, hover, focus, scroll, select), 'input' (fill, type_text, press_key), \
'window' (get_state, list, manage, resize, move_to, set_title), \
'storage' (get, set, delete, get_cookies), 'navigate' (go_to, go_back, get_history, \
set_dialog_response, get_dialog_log), 'recording' (start, stop, checkpoint, list_checkpoints, \
get_events, events_between, get_replay, export, import), 'inspect' (get_styles, \
get_bounding_boxes, highlight, clear_highlights, audit_accessibility, get_performance), \
'css' (inject, remove), 'logs' (console, network, ipc, navigation, dialogs, events, slow_ipc). \
Standalone tools: eval_js, dom_snapshot, invoke_command, screenshot, verify_state, \
detect_ghost_commands, check_ipc_integrity, wait_for, assert_semantic, resolve_command, \
get_registry, get_memory_stats, get_plugin_info.";

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
                    "port": self.state.port.load(Ordering::Relaxed),
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
            let ts = ev.get("timestamp").and_then(|t| t.as_f64()).unwrap_or(0.0);
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
                        .map(|s| s.to_string()),
                },
                "dom_mutation" => AppEvent::DomMutation {
                    webview_label: "main".to_string(),
                    timestamp: now,
                    mutation_count: ev.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
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
                            .and_then(|d| d.as_f64())
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
                        .map(|s| s.to_string()),
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
        // serde_json escapes null bytes as
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

    // ── Port fallback tests ────────────────────────────────────────────────

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

    // ── CSS color sanitization tests ───────────────────────────────────────

    #[test]
    fn css_color_valid_hex() {
        assert_eq!(sanitize_css_color("#ff0000").unwrap(), "#ff0000");
        assert_eq!(sanitize_css_color("#FFF").unwrap(), "#FFF");
        assert_eq!(sanitize_css_color("#12345678").unwrap(), "#12345678");
    }

    #[test]
    fn css_color_valid_rgb() {
        assert_eq!(
            sanitize_css_color("rgb(255, 0, 0)").unwrap(),
            "rgb(255, 0, 0)"
        );
        assert_eq!(
            sanitize_css_color("rgba(0, 0, 0, 0.5)").unwrap(),
            "rgba(0, 0, 0, 0.5)"
        );
    }

    #[test]
    fn css_color_valid_named() {
        assert_eq!(sanitize_css_color("red").unwrap(), "red");
        assert_eq!(sanitize_css_color("transparent").unwrap(), "transparent");
    }

    #[test]
    fn css_color_valid_hsl() {
        assert_eq!(
            sanitize_css_color("hsl(120, 50%, 50%)").unwrap(),
            "hsl(120, 50%, 50%)"
        );
    }

    #[test]
    fn css_color_rejects_too_long() {
        let long = "a".repeat(101);
        assert!(sanitize_css_color(&long).is_err());
    }

    #[test]
    fn css_color_rejects_backslash_escapes() {
        assert!(sanitize_css_color(r"red\00").is_err());
        assert!(sanitize_css_color(r"\72\65\64").is_err());
    }

    #[test]
    fn css_color_rejects_url_injection() {
        assert!(sanitize_css_color("url(http://evil.com)").is_err());
        assert!(sanitize_css_color("URL(http://evil.com)").is_err());
    }

    #[test]
    fn css_color_rejects_expression_injection() {
        assert!(sanitize_css_color("expression(alert(1))").is_err());
        assert!(sanitize_css_color("EXPRESSION(alert(1))").is_err());
    }

    #[test]
    fn css_color_rejects_import() {
        assert!(sanitize_css_color("@import url(evil.css)").is_err());
    }

    #[test]
    fn css_color_rejects_semicolons_and_braces() {
        assert!(sanitize_css_color("red; background: url(evil)").is_err());
        assert!(sanitize_css_color("red} body { color: blue").is_err());
    }

    #[test]
    fn css_color_rejects_special_chars() {
        assert!(sanitize_css_color("red<script>").is_err());
        assert!(sanitize_css_color("red\"onload=alert").is_err());
        assert!(sanitize_css_color("red'onclick=alert").is_err());
    }

    #[test]
    fn css_color_trims_whitespace() {
        assert_eq!(sanitize_css_color("  red  ").unwrap(), "red");
    }

    #[test]
    fn css_color_empty_string() {
        assert_eq!(sanitize_css_color("").unwrap(), "");
    }
}
