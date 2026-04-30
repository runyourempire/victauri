use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `get_registry` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegistryParams {
    /// Search query to filter commands by name or description.
    pub query: Option<String>,
}

/// Parameters for the `invoke_command` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InvokeCommandParams {
    /// The Tauri command name to invoke (e.g. "greet", "`save_settings`").
    pub command: String,
    /// Arguments as a JSON object. Keys are parameter names. Omit for commands with no arguments.
    pub args: Option<serde_json::Value>,
    /// Target webview label.
    pub webview_label: Option<String>,
}
