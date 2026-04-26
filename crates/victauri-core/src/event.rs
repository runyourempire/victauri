use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A single Tauri IPC call with timing, result, and source webview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcCall {
    pub id: String,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<u64>,
    pub result: IpcResult,
    pub arg_size_bytes: usize,
    pub webview_label: String,
}

/// Outcome of an IPC call: pending, success with a JSON value, or error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResult {
    Pending,
    Ok(serde_json::Value),
    Err(String),
}

/// Application event captured by the introspection layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppEvent {
    Ipc(IpcCall),
    StateChange {
        key: String,
        timestamp: DateTime<Utc>,
        caused_by: Option<String>,
    },
    DomMutation {
        webview_label: String,
        timestamp: DateTime<Utc>,
        mutation_count: u32,
    },
    WindowEvent {
        label: String,
        event: String,
        timestamp: DateTime<Utc>,
    },
}

/// Thread-safe ring-buffer event log. Automatically evicts the oldest events
/// when capacity is reached. All operations recover from mutex poisoning.
#[derive(Debug, Clone)]
pub struct EventLog {
    events: Arc<Mutex<VecDeque<AppEvent>>>,
    max_capacity: usize,
}

impl EventLog {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_capacity))),
            max_capacity,
        }
    }

    pub fn push(&self, event: AppEvent) {
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() >= self.max_capacity {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub fn snapshot(&self) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn snapshot_range(&self, offset: usize, limit: usize) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn since(&self, timestamp: DateTime<Utc>) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|e| match e {
                AppEvent::Ipc(call) => call.timestamp >= timestamp,
                AppEvent::StateChange { timestamp: ts, .. } => *ts >= timestamp,
                AppEvent::DomMutation { timestamp: ts, .. } => *ts >= timestamp,
                AppEvent::WindowEvent { timestamp: ts, .. } => *ts >= timestamp,
            })
            .cloned()
            .collect()
    }

    pub fn ipc_calls(&self) -> Vec<IpcCall> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|e| match e {
                AppEvent::Ipc(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn ipc_calls_since(&self, timestamp: DateTime<Utc>) -> Vec<IpcCall> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|e| match e {
                AppEvent::Ipc(call) if call.timestamp >= timestamp => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    pub fn clear(&self) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}
