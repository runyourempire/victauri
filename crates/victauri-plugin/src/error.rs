//! Typed error enum for the `victauri-plugin` crate.

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PluginError {
    #[error("eval timed out after {timeout_secs}s")]
    EvalTimeout { timeout_secs: u64 },

    #[error("eval failed: {message}")]
    EvalFailed { message: String },

    #[error("too many concurrent eval requests (limit: {limit})")]
    EvalConcurrencyExceeded { limit: usize },

    #[error("bridge error: {message}")]
    BridgeError { message: String },

    #[error("screenshot failed: {message}")]
    ScreenshotFailed { message: String },

    #[error("authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("rate limit exceeded")]
    RateLimitExceeded,

    #[error("tool '{tool_name}' is disabled by privacy configuration")]
    ToolDisabled { tool_name: String },

    #[error("command '{command}' is blocked by privacy configuration")]
    CommandBlocked { command: String },

    #[error("window not found: {label}")]
    WindowNotFound { label: String },

    #[error("MCP server failed to start: {message}")]
    ServerStartFailed { message: String },

    #[error("port {port} is already in use")]
    PortInUse { port: u16 },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid URL: {message}")]
    InvalidUrl { message: String },

    #[error(transparent)]
    Core(#[from] victauri_core::VictauriError),
}

pub type Result<T> = std::result::Result<T, PluginError>;
