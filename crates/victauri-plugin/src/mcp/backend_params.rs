use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IpcLogParams {
    /// Maximum number of most recent entries to return.
    pub limit: Option<usize>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegistryParams {
    /// Search query to filter commands by name or description.
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InvokeCommandParams {
    /// The Tauri command name to invoke (e.g. "greet", "save_settings").
    pub command: String,
    /// Arguments as a JSON object. Keys are parameter names. Omit for commands with no arguments.
    pub args: Option<serde_json::Value>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetworkLogParams {
    /// Filter by URL substring.
    pub filter: Option<String>,
    /// Maximum number of entries to return.
    pub limit: Option<usize>,
    /// Target webview label.
    pub webview_label: Option<String>,
}
