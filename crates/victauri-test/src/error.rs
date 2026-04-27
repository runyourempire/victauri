#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TestError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("MCP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("MCP returned error: {message}")]
    Mcp { code: i64, message: String },

    #[error("tool call failed: {0}")]
    ToolError(String),

    #[error("assertion failed: {0}")]
    Assertion(String),

    #[error("timeout waiting for condition")]
    Timeout,
}
