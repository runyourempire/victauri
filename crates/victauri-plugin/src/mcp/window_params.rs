use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowStateParams {
    /// Filter to a specific window label. Returns all windows if omitted.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Target window label. If omitted, captures the main/first visible window.
    pub window_label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManageWindowParams {
    /// Action: minimize, unminimize, maximize, unmaximize, close, focus, show, hide, fullscreen, unfullscreen, always_on_top, not_always_on_top.
    pub action: String,
    /// Target window label.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResizeWindowParams {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
    /// Target window label.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveWindowParams {
    /// X position in logical pixels.
    pub x: i32,
    /// Y position in logical pixels.
    pub y: i32,
    /// Target window label.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetWindowTitleParams {
    /// New window title.
    pub title: String,
    /// Target window label.
    pub label: Option<String>,
}
