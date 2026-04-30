use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `screenshot` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Target window label. If omitted, captures the main/first visible window.
    pub window_label: Option<String>,
}
