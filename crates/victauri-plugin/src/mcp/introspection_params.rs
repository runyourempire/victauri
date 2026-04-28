use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStylesParams {
    /// Ref handle ID of the element to inspect.
    pub ref_id: String,
    /// Optional list of CSS property names to return. If omitted, returns key properties.
    pub properties: Option<Vec<String>>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBoundingBoxesParams {
    /// List of ref handle IDs to measure.
    pub ref_ids: Vec<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HighlightElementParams {
    /// Ref handle ID of the element to highlight.
    pub ref_id: String,
    /// CSS color for the overlay (default: "rgba(255, 0, 0, 0.3)").
    pub color: Option<String>,
    /// Optional text label to display above the highlight.
    pub label: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClearHighlightsParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InjectCssParams {
    /// CSS text to inject into the page.
    pub css: String,
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveInjectedCssParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditAccessibilityParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPerformanceMetricsParams {
    /// Target webview label.
    pub webview_label: Option<String>,
}
