use schemars::JsonSchema;
use serde::Deserialize;

// ── Enums ──────────────────────────────────────────────────────────────────

/// Web storage type for browser storage operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    /// Browser localStorage (persistent across sessions).
    Local,
    /// Browser sessionStorage (cleared when tab closes).
    Session,
}

impl StorageType {
    /// Returns the JavaScript property name for this storage type.
    #[must_use]
    pub fn js_property(self) -> &'static str {
        match self {
            Self::Local => "localStorage",
            Self::Session => "sessionStorage",
        }
    }
}

/// Browser dialog type for dialog response configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DialogType {
    /// JavaScript `alert()` dialog.
    Alert,
    /// JavaScript `confirm()` dialog.
    Confirm,
    /// JavaScript `prompt()` dialog.
    Prompt,
}

impl DialogType {
    /// Returns the lowercase string for JS bridge consumption.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Confirm => "confirm",
            Self::Prompt => "prompt",
        }
    }
}

/// Action to take on a browser dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DialogAction {
    /// Accept the dialog (click OK/Yes).
    Accept,
    /// Dismiss the dialog (click Cancel/No).
    Dismiss,
}

impl DialogAction {
    /// Returns the lowercase string for JS bridge consumption.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Dismiss => "dismiss",
        }
    }
}

// ── interact ────────────────────────────────────────────────────────────────

/// Parameters for the compound `interact` tool (click, hover, focus, scroll, select).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InteractParams {
    /// Action to perform: click, double_click, hover, focus, scroll_into_view, select_option.
    pub action: String,
    /// Ref handle ID from a DOM snapshot (e.g. "e5"). Required for click, double_click, hover, focus, select_option.
    pub ref_id: Option<String>,
    /// Option values for select_option action.
    pub values: Option<Vec<String>>,
    /// Horizontal scroll position (pixels). Used with scroll_into_view when ref_id is null.
    pub x: Option<f64>,
    /// Vertical scroll position (pixels). Used with scroll_into_view when ref_id is null.
    pub y: Option<f64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── input ───────────────────────────────────────────────────────────────────

/// Parameters for the compound `input` tool (fill, type_text, press_key).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InputParams {
    /// Action to perform: fill, type_text, press_key.
    pub action: String,
    /// Ref handle ID of the target element. Required for fill and type_text.
    pub ref_id: Option<String>,
    /// Value to set (for fill action).
    pub value: Option<String>,
    /// Text to type character-by-character (for type_text action).
    pub text: Option<String>,
    /// Key to press (for press_key action, e.g. "Enter", "Escape", "Tab", "ArrowDown").
    pub key: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── window ──────────────────────────────────────────────────────────────────

/// Parameters for the compound `window` tool (get_state, list, manage, resize, move, title).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowParams {
    /// Action to perform: get_state, list, manage, resize, move_to, set_title.
    pub action: String,
    /// Target window label.
    pub label: Option<String>,
    /// Window management action (for manage): minimize, unminimize, maximize, unmaximize, close, focus, show, hide, fullscreen, unfullscreen, always_on_top, not_always_on_top.
    pub manage_action: Option<String>,
    /// Width in logical pixels (for resize).
    pub width: Option<u32>,
    /// Height in logical pixels (for resize).
    pub height: Option<u32>,
    /// X position in logical pixels (for move_to).
    pub x: Option<i32>,
    /// Y position in logical pixels (for move_to).
    pub y: Option<i32>,
    /// New window title (for set_title).
    pub title: Option<String>,
}

// ── storage ─────────────────────────────────────────────────────────────────

/// Parameters for the compound `storage` tool (get, set, delete, get_cookies).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StorageParams {
    /// Action to perform: get, set, delete, get_cookies.
    pub action: String,
    /// Storage type for get/set/delete. Defaults to local if omitted.
    pub storage_type: Option<StorageType>,
    /// Key to read, write, or delete.
    pub key: Option<String>,
    /// Value to store (for set action). Will be JSON-serialized if not a string.
    pub value: Option<serde_json::Value>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── navigate ────────────────────────────────────────────────────────────────

/// Parameters for the compound `navigate` tool (go_to, go_back, history, dialogs).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NavigateParams {
    /// Action to perform: go_to, go_back, get_history, set_dialog_response, get_dialog_log.
    pub action: String,
    /// URL to navigate to (for go_to action).
    pub url: Option<String>,
    /// Dialog type (for set_dialog_response).
    pub dialog_type: Option<DialogType>,
    /// Dialog action (for set_dialog_response).
    pub dialog_action: Option<DialogAction>,
    /// Response text for prompt dialogs (for set_dialog_response).
    pub text: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── recording ───────────────────────────────────────────────────────────────

/// Parameters for the compound `recording` tool (start, stop, checkpoint, replay, export, import).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    /// Action to perform: start, stop, checkpoint, list_checkpoints, get_events, events_between, get_replay, export, import.
    pub action: String,
    /// Session ID (for start — optional, UUID generated if omitted).
    pub session_id: Option<String>,
    /// Checkpoint ID (for checkpoint, required).
    pub checkpoint_id: Option<String>,
    /// Checkpoint label (for checkpoint, optional).
    pub checkpoint_label: Option<String>,
    /// State snapshot as JSON (for checkpoint).
    pub state: Option<serde_json::Value>,
    /// Starting checkpoint ID (for events_between).
    pub from: Option<String>,
    /// Ending checkpoint ID (for events_between).
    pub to: Option<String>,
    /// Only return events after this index (for get_events).
    pub since_index: Option<usize>,
    /// JSON string of a previously exported RecordedSession (for import).
    pub session_json: Option<String>,
}

// ── inspect ─────────────────────────────────────────────────────────────────

/// Parameters for the compound `inspect` tool (styles, bounding boxes, highlight, a11y, perf).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectParams {
    /// Action to perform: get_styles, get_bounding_boxes, highlight, clear_highlights, audit_accessibility, get_performance.
    pub action: String,
    /// Ref handle ID (for get_styles, highlight).
    pub ref_id: Option<String>,
    /// List of ref handle IDs (for get_bounding_boxes).
    pub ref_ids: Option<Vec<String>>,
    /// CSS property names to return (for get_styles — if omitted, returns key properties).
    pub properties: Option<Vec<String>>,
    /// CSS color for the highlight overlay (for highlight, default: "rgba(255, 0, 0, 0.3)").
    pub color: Option<String>,
    /// Text label displayed above the highlight (for highlight).
    #[serde(rename = "highlight_label")]
    pub label: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── css ─────────────────────────────────────────────────────────────────────

/// Parameters for the compound `css` tool (inject, remove).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CssParams {
    /// Action to perform: inject, remove.
    pub action: String,
    /// CSS text to inject (for inject action).
    pub css: Option<String>,
    /// Target webview label.
    pub webview_label: Option<String>,
}

// ── logs ────────────────────────────────────────────────────────────────────

/// Parameters for the compound `logs` tool (console, network, ipc, navigation, dialogs, events).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogsParams {
    /// Action to perform: console, network, ipc, navigation, dialogs, events, slow_ipc.
    pub action: String,
    /// Only return entries after this Unix timestamp in milliseconds (for console, events).
    pub since: Option<f64>,
    /// Filter by URL substring (for network).
    pub filter: Option<String>,
    /// Maximum number of entries to return (for ipc, network, slow_ipc).
    pub limit: Option<usize>,
    /// Threshold in milliseconds for slow IPC calls (for slow_ipc).
    pub threshold_ms: Option<u64>,
    /// Target webview label.
    pub webview_label: Option<String>,
}
