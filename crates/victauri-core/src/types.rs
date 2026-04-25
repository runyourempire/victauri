use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefHandle {
    pub id: String,
    pub selector: String,
    pub role: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDelta {
    pub before_bytes: i64,
    pub after_bytes: i64,
    pub delta_bytes: i64,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub frontend_state: serde_json::Value,
    pub backend_state: serde_json::Value,
    pub divergences: Vec<Divergence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Divergence {
    pub path: String,
    pub frontend_value: serde_json::Value,
    pub backend_value: serde_json::Value,
    pub severity: DivergenceSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DivergenceSeverity {
    Info,
    Warning,
    Error,
}
