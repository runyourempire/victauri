use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResult {
    Pending,
    Ok(serde_json::Value),
    Err(String),
}

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
        let mut events = self.events.lock().unwrap();
        if events.len() >= self.max_capacity {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub fn snapshot(&self) -> Vec<AppEvent> {
        self.events.lock().unwrap().iter().cloned().collect()
    }

    pub fn since(&self, timestamp: DateTime<Utc>) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap()
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
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                AppEvent::Ipc(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}
