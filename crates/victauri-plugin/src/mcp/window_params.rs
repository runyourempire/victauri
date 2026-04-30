use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `get_window_state` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowStateParams {
    /// Filter to a specific window label. Returns all windows if omitted.
    pub label: Option<String>,
}

/// Parameters for the `screenshot` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Target window label. If omitted, captures the main/first visible window.
    pub window_label: Option<String>,
}

/// Parameters for the `manage_window` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManageWindowParams {
    /// Action: minimize, unminimize, maximize, unmaximize, close, focus, show, hide, fullscreen, unfullscreen, always_on_top, not_always_on_top.
    pub action: String,
    /// Target window label.
    pub label: Option<String>,
}

/// Parameters for the `resize_window` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResizeWindowParams {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
    /// Target window label.
    pub label: Option<String>,
}

/// Parameters for the `move_window` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveWindowParams {
    /// X position in logical pixels.
    pub x: i32,
    /// Y position in logical pixels.
    pub y: i32,
    /// Target window label.
    pub label: Option<String>,
}

/// Parameters for the `set_window_title` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetWindowTitleParams {
    /// New window title.
    pub title: String,
    /// Target window label.
    pub label: Option<String>,
}
