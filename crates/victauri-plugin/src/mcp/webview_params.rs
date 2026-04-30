use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Parameters for the `eval_js` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalJsParams {
    /// JavaScript code to evaluate in the webview. Async expressions supported.
    pub code: String,
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
}

/// Output format for DOM snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotFormat {
    /// Compact accessible text — 70-80% fewer tokens than JSON.
    Compact,
    /// Full JSON tree with all element attributes.
    Json,
}

impl fmt::Display for SnapshotFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compact => f.write_str("compact"),
            Self::Json => f.write_str("json"),
        }
    }
}

/// Parameters for the `snapshot` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnapshotParams {
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
    /// Snapshot format: "compact" (default, accessible text) or "json" (full tree). Compact uses 70-80% fewer tokens.
    pub format: Option<SnapshotFormat>,
}
