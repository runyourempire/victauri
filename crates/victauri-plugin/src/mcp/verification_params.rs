use schemars::JsonSchema;
use serde::Deserialize;

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
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SlowIpcParams {
    /// Threshold in milliseconds. Returns IPC calls slower than this value.
    pub threshold_ms: u64,
    /// Maximum number of results. Default: 20.
    pub limit: Option<usize>,
}
