use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `eval_js` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalJsParams {
    /// JavaScript code to evaluate in the webview. Async expressions supported.
    pub code: String,
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
}

/// Parameters for the `click` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClickParams {
    /// Ref handle ID from a DOM snapshot (e.g. "e5").
    pub ref_id: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `fill` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FillParams {
    /// Ref handle ID of the input element.
    pub ref_id: String,
    /// Value to set on the input.
    pub value: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `type_text` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeTextParams {
    /// Ref handle ID of the element to type into.
    pub ref_id: String,
    /// Text to type character by character.
    pub text: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `snapshot` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnapshotParams {
    /// Target webview label. If omitted, targets the first available webview.
    pub webview_label: Option<String>,
    /// Snapshot format: "compact" (default, accessible text) or "json" (full tree). Compact uses 70-80% fewer tokens.
    pub format: Option<String>,
}

/// Parameters for the `press_key` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PressKeyParams {
    /// Key to press (e.g. "Enter", "Escape", "Tab", "ArrowDown").
    pub key: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `get_console_logs` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetConsoleLogsParams {
    /// Only return logs after this Unix timestamp (milliseconds). If omitted, returns all captured logs.
    pub since: Option<f64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `double_click` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DoubleClickParams {
    /// Ref handle ID from a DOM snapshot.
    pub ref_id: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `hover` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HoverParams {
    /// Ref handle ID from a DOM snapshot.
    pub ref_id: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `select_option` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SelectOptionParams {
    /// Ref handle ID of the `<select>` element.
    pub ref_id: String,
    /// Values to select.
    pub values: Vec<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `scroll_to` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScrollToParams {
    /// Ref handle ID to scroll into view. If null, scrolls to absolute coordinates.
    pub ref_id: Option<String>,
    /// Horizontal scroll position (pixels). Used when ref_id is null.
    pub x: Option<f64>,
    /// Vertical scroll position (pixels). Used when ref_id is null.
    pub y: Option<f64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

/// Parameters for the `focus_element` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FocusElementParams {
    /// Ref handle ID of the element to focus.
    pub ref_id: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}
