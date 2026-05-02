/// Errors that can occur when interacting with the Victauri MCP server from tests.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TestError {
    /// Failed to connect to the Victauri MCP server at the expected port.
    #[error("connection failed: {0}")]
    Connection(String),

    /// An HTTP-level error occurred during an MCP request.
    #[error("MCP request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// The MCP server returned a JSON-RPC error response.
    #[error("MCP returned error: {message}")]
    Mcp {
        /// JSON-RPC error code.
        code: i64,
        /// Human-readable error message from the server.
        message: String,
    },

    /// A tool call returned `isError: true` in its result.
    #[error("tool call failed: {0}")]
    ToolError(String),

    /// A test assertion evaluated to false.
    #[error("assertion failed: {0}")]
    Assertion(String),

    /// A `wait_for` condition did not become true within the allowed time.
    #[error("timeout: {0}")]
    Timeout(String),

    /// An element matching the given criteria was not found in the DOM snapshot.
    #[error("element not found: {0}")]
    ElementNotFound(String),
}
