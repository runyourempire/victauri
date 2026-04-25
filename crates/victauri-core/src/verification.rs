use crate::event::{EventLog, IpcResult};
use crate::registry::CommandRegistry;
use crate::types::{Divergence, DivergenceSeverity, VerificationResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Cross-boundary state verification ───────────────────────────────────────

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
                    (None, None) => unreachable!(),
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
            let f_f64 = f.as_f64().unwrap_or(0.0);
            let b_f64 = b.as_f64().unwrap_or(0.0);
            if (f_f64 - b_f64).abs() < f64::EPSILON {
                DivergenceSeverity::Info
            } else {
                DivergenceSeverity::Error
            }
        }
        _ => DivergenceSeverity::Error,
    }
}

// ── Ghost command detection ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostCommandReport {
    pub ghost_commands: Vec<GhostCommand>,
    pub total_frontend_commands: usize,
    pub total_registry_commands: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostCommand {
    pub name: String,
    pub source: GhostSource,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostSource {
    FrontendOnly,
    RegistryOnly,
}

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
        total_frontend_commands: frontend_commands.len(),
        total_registry_commands: registry_list.len(),
    }
}

// ── IPC round-trip integrity ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcIntegrityReport {
    pub total_calls: usize,
    pub completed: usize,
    pub pending: usize,
    pub errored: usize,
    pub stale_calls: Vec<StaleCall>,
    pub error_calls: Vec<ErrorCall>,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleCall {
    pub id: String,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub age_ms: i64,
    pub webview_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCall {
    pub id: String,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub error: String,
    pub webview_label: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAssertion {
    pub label: String,
    pub condition: String,
    pub expected: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub label: String,
    pub passed: bool,
    pub actual: serde_json::Value,
    pub expected: serde_json::Value,
    pub message: Option<String>,
}

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
        "truthy" => {
            matches!(
                &actual,
                serde_json::Value::Bool(true)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::String(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_)
            ) && actual != serde_json::Value::String(String::new())
        }
        "falsy" => {
            matches!(
                &actual,
                serde_json::Value::Null | serde_json::Value::Bool(false)
            ) || actual == serde_json::Value::String(String::new())
                || actual == serde_json::json!(0)
        }
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
        _ => false,
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
