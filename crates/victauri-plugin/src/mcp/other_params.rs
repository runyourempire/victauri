use schemars::JsonSchema;
use serde::Deserialize;

// ── Streaming ──────────────────────────────────────────────────────────────

/// Parameters for the `get_event_stream` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventStreamParams {
    /// Only return events after this Unix timestamp (milliseconds). If omitted, returns all events.
    pub since: Option<f64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── Intent ─────────────────────────────────────────────────────────────────

/// Parameters for the `resolve_command` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveCommandParams {
    /// Natural language query describing what you want to do (e.g. "save the user's settings").
    pub query: String,
    /// Maximum number of results to return. Default: 5.
    pub limit: Option<usize>,
}

/// Parameters for the `semantic_assert` tool.
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

// ── Storage ────────────────────────────────────────────────────────────────

/// Parameters for the `get_storage` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStorageParams {
    /// Storage type: "local" or "session".
    pub storage_type: String,
    /// Specific key to read. If omitted, returns all entries.
    pub key: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `set_storage` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetStorageParams {
    /// Storage type: "local" or "session".
    pub storage_type: String,
    /// Key to set.
    pub key: String,
    /// Value to store (will be JSON-serialized if not a string).
    pub value: serde_json::Value,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `delete_storage` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteStorageParams {
    /// Storage type: "local" or "session".
    pub storage_type: String,
    /// Key to delete.
    pub key: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `get_cookies` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCookiesParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── Navigation ─────────────────────────────────────────────────────────────

/// Parameters for the `get_navigation_log` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NavigationLogParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `navigate` tool.
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NavigateParams {
    /// URL to navigate to.
    pub url: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── Dialogs ────────────────────────────────────────────────────────────────

/// Parameters for the `get_dialog_log` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DialogLogParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `set_dialog_response` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDialogResponseParams {
    /// Dialog type: "alert", "confirm", or "prompt".
    pub dialog_type: String,
    /// Action: "accept" or "dismiss".
    pub action: String,
    /// Response text for prompt dialogs.
    pub text: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── Wait ───────────────────────────────────────────────────────────────────

/// Parameters for the `wait_for` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitForParams {
    /// Condition to wait for: text, text_gone, selector, selector_gone, url, ipc_idle, network_idle.
    pub condition: String,
    /// Value for the condition (text to find, CSS selector, URL substring).
    pub value: Option<String>,
    /// Maximum time to wait in milliseconds. Default: 10000.
    pub timeout_ms: Option<u64>,
    /// Polling interval in milliseconds. Default: 200.
    pub poll_ms: Option<u64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── Find Elements ──────────────────────────────────────────────────────────

/// Parameters for the `find_elements` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindElementsParams {
    /// Text content to search for (case-insensitive substring match).
    pub text: Option<String>,
    /// ARIA role to match (exact match).
    pub role: Option<String>,
    /// data-testid attribute value to match (exact match).
    pub test_id: Option<String>,
    /// CSS selector to match.
    pub css: Option<String>,
    /// Accessible name to search for (aria-label, title, placeholder — case-insensitive substring).
    pub name: Option<String>,
    /// Maximum number of results to return. Default: 10.
    pub max_results: Option<u32>,
    /// Target webview label.
    pub webview_label: Option<String>,
}
