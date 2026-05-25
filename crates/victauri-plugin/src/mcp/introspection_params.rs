use schemars::JsonSchema;
use serde::Deserialize;

/// Actions available in the `introspect` compound tool.
#[derive(Debug, Copy, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntrospectAction {
    /// Get per-command execution timing statistics (min/max/avg/p95).
    CommandTimings,
    /// Report which registered commands have been called during this session.
    Coverage,
    /// Record the current response shape of a command as a baseline contract.
    ContractRecord,
    /// Check all recorded contracts for schema drift.
    ContractCheck,
    /// List all recorded contract baselines.
    ContractList,
    /// Clear all recorded contract baselines.
    ContractClear,
    /// Report plugin startup phase timing breakdown.
    StartupTiming,
    /// Audit Tauri v2 capabilities and permissions.
    Capabilities,
    /// `SQLite` database health diagnostics (journal mode, WAL, page stats).
    DbHealth,
}

/// Parameters for the `introspect` compound tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IntrospectParams {
    /// Which introspection action to perform.
    pub action: IntrospectAction,

    /// For `command_timings`: only show commands slower than this threshold (ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow_threshold_ms: Option<f64>,

    /// For `contract_record`: the command to record a baseline for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// For `contract_record`: optional arguments to pass when invoking the command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,

    /// For `db_health`: path to the `SQLite` database file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<String>,

    /// Target webview for actions that need JS eval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webview_label: Option<String>,
}

/// Actions available in the `fault` compound tool.
#[derive(Debug, Copy, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FaultAction {
    /// Inject a fault rule for a specific command.
    Inject,
    /// List all active fault injection rules.
    List,
    /// Remove a specific fault rule.
    Clear,
    /// Remove all fault rules.
    ClearAll,
}

/// The type of fault to inject.
#[derive(Debug, Copy, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    /// Add artificial delay before command execution.
    Delay,
    /// Return an error without executing the command.
    Error,
    /// Drop the response (return empty result).
    Drop,
    /// Execute normally but corrupt the response structure.
    Corrupt,
}

/// Parameters for the `fault` compound tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FaultParams {
    /// Which fault action to perform.
    pub action: FaultAction,

    /// Target command name (required for `inject` and `clear`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Type of fault to inject (required for `inject`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault_type: Option<FaultKind>,

    /// For `delay` faults: delay in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,

    /// For `error` faults: error message to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// Maximum number of times to trigger (0 or omit for unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_triggers: Option<u64>,
}
