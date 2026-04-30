//! Cross-boundary verification, ghost command detection, IPC integrity
//! checks, and semantic test assertions.

use crate::event::{EventLog, IpcResult};
use crate::registry::CommandRegistry;
use crate::types::{Divergence, DivergenceSeverity, VerificationResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Cross-boundary state verification ───────────────────────────────────────

/// Compares frontend and backend state trees, returning all divergences found.
///
/// ```
/// use victauri_core::verification::verify_state;
/// use serde_json::json;
///
/// let result = verify_state(json!({"title": "App"}), json!({"title": "App"}));
/// assert!(result.passed);
/// assert!(result.divergences.is_empty());
/// ```
#[must_use]
pub fn verify_state(
    frontend_state: serde_json::Value,
    backend_state: serde_json::Value,
) -> VerificationResult {
    let mut divergences = Vec::new();
    compare_values("", &frontend_state, &backend_state, &mut divergences);
    let passed = divergences.is_empty();
    VerificationResult {
        passed,
        frontend_state,
        backend_state,
        divergences,
    }
}

fn compare_values(
    path: &str,
    frontend: &serde_json::Value,
    backend: &serde_json::Value,
    divergences: &mut Vec<Divergence>,
) {
    if frontend == backend {
        return;
    }

    match (frontend, backend) {
        (serde_json::Value::Object(f_map), serde_json::Value::Object(b_map)) => {
            for (key, f_val) in f_map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match b_map.get(key) {
                    Some(b_val) => compare_values(&child_path, f_val, b_val, divergences),
                    None => divergences.push(Divergence {
                        path: child_path,
                        frontend_value: f_val.clone(),
                        backend_value: serde_json::Value::Null,
                        severity: DivergenceSeverity::Warning,
                    }),
                }
            }
            for key in b_map.keys() {
                if !f_map.contains_key(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    divergences.push(Divergence {
                        path: child_path,
                        frontend_value: serde_json::Value::Null,
                        backend_value: b_map[key].clone(),
                        severity: DivergenceSeverity::Warning,
                    });
                }
            }
        }
        (serde_json::Value::Array(f_arr), serde_json::Value::Array(b_arr)) => {
            let max_len = f_arr.len().max(b_arr.len());
            for i in 0..max_len {
                let child_path = if path.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{path}[{i}]")
                };
                match (f_arr.get(i), b_arr.get(i)) {
                    (Some(f_val), Some(b_val)) => {
                        compare_values(&child_path, f_val, b_val, divergences);
                    }
                    (Some(f_val), None) => divergences.push(Divergence {
                        path: child_path,
                        frontend_value: f_val.clone(),
                        backend_value: serde_json::Value::Null,
                        severity: DivergenceSeverity::Warning,
                    }),
                    (None, Some(b_val)) => divergences.push(Divergence {
                        path: child_path,
                        frontend_value: serde_json::Value::Null,
                        backend_value: b_val.clone(),
                        severity: DivergenceSeverity::Warning,
                    }),
                    (None, None) => {}
                }
            }
        }
        _ => {
            let severity = classify_severity(frontend, backend);
            divergences.push(Divergence {
                path: if path.is_empty() {
                    "$".to_string()
                } else {
                    path.to_string()
                },
                frontend_value: frontend.clone(),
                backend_value: backend.clone(),
                severity,
            });
        }
    }
}

fn classify_severity(
    frontend: &serde_json::Value,
    backend: &serde_json::Value,
) -> DivergenceSeverity {
    match (frontend, backend) {
        (serde_json::Value::Null, _) | (_, serde_json::Value::Null) => DivergenceSeverity::Warning,
        (serde_json::Value::Number(f), serde_json::Value::Number(b)) => {
            match (f.as_f64(), b.as_f64()) {
                (Some(fv), Some(bv)) if (fv - bv).abs() < 1e-9 => DivergenceSeverity::Info,
                _ => DivergenceSeverity::Error,
            }
        }
        _ => DivergenceSeverity::Error,
    }
}

// ── Ghost command detection ─────────────────────────────────────────────────

/// Report of ghost commands -- commands that exist on only one side of the IPC boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostCommandReport {
    /// Commands found on only the frontend or only the backend.
    pub ghost_commands: Vec<GhostCommand>,
    /// Total unique commands observed from the frontend.
    pub total_frontend_commands: usize,
    /// Total commands registered in the backend registry.
    pub total_registry_commands: usize,
}

/// A command that exists on only one side of the frontend/backend boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostCommand {
    /// Command name as invoked or registered.
    pub name: String,
    /// Which side the command was found on.
    pub source: GhostSource,
    /// Optional description, if available from the registry.
    pub description: Option<String>,
}

/// Indicates which side of the IPC boundary a ghost command was found on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GhostSource {
    /// Command invoked from the frontend but not registered in the backend.
    FrontendOnly,
    /// Command registered in the backend but never invoked from the frontend.
    RegistryOnly,
}

/// Detects commands that exist on only one side of the IPC boundary (frontend vs registry).
#[must_use]
pub fn detect_ghost_commands(
    frontend_commands: &[String],
    registry: &CommandRegistry,
) -> GhostCommandReport {
    let registry_list = registry.list();
    let registry_names: std::collections::HashSet<&str> =
        registry_list.iter().map(|c| c.name.as_str()).collect();
    let frontend_set: std::collections::HashSet<&str> =
        frontend_commands.iter().map(|s| s.as_str()).collect();

    let mut ghost_commands = Vec::new();

    for name in &frontend_set {
        if !registry_names.contains(name) {
            ghost_commands.push(GhostCommand {
                name: name.to_string(),
                source: GhostSource::FrontendOnly,
                description: Some(
                    "Command invoked from frontend but not registered in backend".to_string(),
                ),
            });
        }
    }

    for cmd in &registry_list {
        if !frontend_set.contains(cmd.name.as_str()) {
            ghost_commands.push(GhostCommand {
                name: cmd.name.clone(),
                source: GhostSource::RegistryOnly,
                description: cmd.description.clone(),
            });
        }
    }

    ghost_commands.sort_by(|a, b| a.name.cmp(&b.name));

    GhostCommandReport {
        ghost_commands,
        total_frontend_commands: frontend_set.len(),
        total_registry_commands: registry_list.len(),
    }
}

// ── IPC round-trip integrity ────────────────────────────────────────────────

/// Summary of IPC round-trip health: completed, pending, errored, and stale calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcIntegrityReport {
    /// Total number of IPC calls analyzed.
    pub total_calls: usize,
    /// Calls that completed successfully.
    pub completed: usize,
    /// Calls still awaiting a response.
    pub pending: usize,
    /// Calls that returned an error.
    pub errored: usize,
    /// Pending calls that have exceeded the staleness threshold.
    pub stale_calls: Vec<StaleCall>,
    /// Calls that resulted in errors.
    pub error_calls: Vec<ErrorCall>,
    /// True if there are no stale or errored calls.
    pub healthy: bool,
}

/// An IPC call that has been pending longer than the staleness threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleCall {
    /// Unique call identifier.
    pub id: String,
    /// Name of the invoked command.
    pub command: String,
    /// When the call was initiated.
    pub timestamp: DateTime<Utc>,
    /// How long the call has been pending, in milliseconds.
    pub age_ms: i64,
    /// Webview that initiated the call.
    pub webview_label: String,
}

/// An IPC call that returned an error result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCall {
    /// Unique call identifier.
    pub id: String,
    /// Name of the invoked command.
    pub command: String,
    /// When the call was initiated.
    pub timestamp: DateTime<Utc>,
    /// Error message returned by the backend.
    pub error: String,
    /// Webview that initiated the call.
    pub webview_label: String,
}

/// Analyzes the event log for IPC health, flagging stale and errored calls.
#[must_use]
pub fn check_ipc_integrity(event_log: &EventLog, stale_threshold_ms: i64) -> IpcIntegrityReport {
    let now = Utc::now();
    let calls = event_log.ipc_calls();
    let total_calls = calls.len();
    let mut completed = 0usize;
    let mut pending = 0usize;
    let mut errored = 0usize;
    let mut stale_calls = Vec::new();
    let mut error_calls = Vec::new();

    for call in &calls {
        match &call.result {
            IpcResult::Ok(_) => completed += 1,
            IpcResult::Pending => {
                pending += 1;
                let age_ms = (now - call.timestamp).num_milliseconds();
                if age_ms >= stale_threshold_ms {
                    stale_calls.push(StaleCall {
                        id: call.id.clone(),
                        command: call.command.clone(),
                        timestamp: call.timestamp,
                        age_ms,
                        webview_label: call.webview_label.clone(),
                    });
                }
            }
            IpcResult::Err(e) => {
                errored += 1;
                error_calls.push(ErrorCall {
                    id: call.id.clone(),
                    command: call.command.clone(),
                    timestamp: call.timestamp,
                    error: e.clone(),
                    webview_label: call.webview_label.clone(),
                });
            }
        }
    }

    let healthy = stale_calls.is_empty() && errored == 0;

    IpcIntegrityReport {
        total_calls,
        completed,
        pending,
        errored,
        stale_calls,
        error_calls,
        healthy,
    }
}

// ── Semantic test assertions ────────────────────────────────────────────────

/// A declarative assertion to evaluate against a runtime value (e.g. "equals", "truthy").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAssertion {
    /// Human-readable label describing what is being asserted.
    pub label: String,
    /// Condition operator: "equals", "contains", "greater_than", "truthy", etc.
    pub condition: String,
    /// Expected value to compare against (interpretation depends on condition).
    pub expected: serde_json::Value,
}

/// Outcome of evaluating a semantic assertion against an actual value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Label from the original assertion.
    pub label: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// The actual value that was evaluated.
    pub actual: serde_json::Value,
    /// The expected value from the assertion.
    pub expected: serde_json::Value,
    /// Failure message explaining why the assertion failed, if it did.
    pub message: Option<String>,
}

/// Evaluates a semantic assertion against an actual runtime value.
///
/// ```
/// use victauri_core::verification::{evaluate_assertion, SemanticAssertion};
/// use serde_json::json;
///
/// let assertion = SemanticAssertion {
///     label: "check count".to_string(),
///     condition: "equals".to_string(),
///     expected: json!(42),
/// };
/// let result = evaluate_assertion(json!(42), &assertion);
/// assert!(result.passed);
/// ```
#[must_use]
pub fn evaluate_assertion(
    actual: serde_json::Value,
    assertion: &SemanticAssertion,
) -> AssertionResult {
    let passed = match assertion.condition.as_str() {
        "equals" => actual == assertion.expected,
        "not_equals" => actual != assertion.expected,
        "contains" => match (&actual, &assertion.expected) {
            (serde_json::Value::String(a), serde_json::Value::String(e)) => a.contains(e.as_str()),
            (serde_json::Value::Array(arr), val) => arr.contains(val),
            _ => false,
        },
        "greater_than" => match (actual.as_f64(), assertion.expected.as_f64()) {
            (Some(a), Some(e)) => a > e,
            _ => false,
        },
        "less_than" => match (actual.as_f64(), assertion.expected.as_f64()) {
            (Some(a), Some(e)) => a < e,
            _ => false,
        },
        "truthy" => match &actual {
            serde_json::Value::Null | serde_json::Value::Bool(false) => false,
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            _ => true,
        },
        "falsy" => match &actual {
            serde_json::Value::Null | serde_json::Value::Bool(false) => true,
            serde_json::Value::String(s) => s.is_empty(),
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) == 0.0,
            _ => false,
        },
        "exists" => actual != serde_json::Value::Null,
        "type_is" => {
            let type_name = assertion.expected.as_str().unwrap_or("");
            match type_name {
                "string" => actual.is_string(),
                "number" => actual.is_number(),
                "boolean" => actual.is_boolean(),
                "array" => actual.is_array(),
                "object" => actual.is_object(),
                "null" => actual.is_null(),
                _ => false,
            }
        }
        unknown => {
            return AssertionResult {
                label: assertion.label.clone(),
                passed: false,
                actual,
                expected: assertion.expected.clone(),
                message: Some(format!(
                    "Unknown assertion condition '{}' in '{}'",
                    unknown, assertion.label
                )),
            };
        }
    };

    let message = if !passed {
        Some(format!(
            "Assertion '{}' failed: expected {} {:?}, got {:?}",
            assertion.label, assertion.condition, assertion.expected, actual
        ))
    } else {
        None
    };

    AssertionResult {
        label: assertion.label.clone(),
        passed,
        actual,
        expected: assertion.expected.clone(),
        message,
    }
}
