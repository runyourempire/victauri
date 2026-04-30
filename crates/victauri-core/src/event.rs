//! Application event types and a thread-safe ring-buffer event log.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A single Tauri IPC call with timing, result, and source webview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcCall {
    /// Unique call identifier for correlation.
    pub id: String,
    /// Name of the Tauri command that was invoked.
    pub command: String,
    /// When the call was initiated.
    pub timestamp: DateTime<Utc>,
    /// Round-trip duration in milliseconds, if completed.
    pub duration_ms: Option<u64>,
    /// Current outcome of the call (pending, ok, or error).
    pub result: IpcResult,
    /// Size of the serialized arguments in bytes.
    pub arg_size_bytes: usize,
    /// Label of the webview that initiated the call.
    pub webview_label: String,
}

/// Outcome of an IPC call: pending, success with a JSON value, or error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum IpcResult {
    /// Call is still in flight, awaiting a response.
    Pending,
    /// Call completed successfully with a JSON return value.
    Ok(serde_json::Value),
    /// Call failed with an error message.
    Err(String),
}

/// Application event captured by the introspection layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum AppEvent {
    /// An IPC call between webview and Rust backend.
    Ipc(IpcCall),
    /// A change to application state in the backend.
    StateChange {
        /// State key that changed.
        key: String,
        /// When the change occurred.
        timestamp: DateTime<Utc>,
        /// Command or action that triggered the change, if known.
        caused_by: Option<String>,
    },
    /// A batch of DOM mutations observed in a webview.
    DomMutation {
        /// Webview where the mutations were observed.
        webview_label: String,
        /// When the mutations were observed.
        timestamp: DateTime<Utc>,
        /// Number of individual DOM mutations in this batch.
        mutation_count: u32,
    },
    /// A native window lifecycle event (e.g. focus, resize, close).
    WindowEvent {
        /// Tauri window label that emitted the event.
        label: String,
        /// Event name (e.g. "focus", "resize").
        event: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
}

impl AppEvent {
    /// Returns the timestamp of this event, regardless of variant.
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Ipc(call) => call.timestamp,
            Self::StateChange { timestamp, .. }
            | Self::DomMutation { timestamp, .. }
            | Self::WindowEvent { timestamp, .. } => *timestamp,
        }
    }
}

/// Thread-safe ring-buffer event log. Automatically evicts the oldest events
/// when capacity is reached. All operations recover from mutex poisoning.
#[derive(Debug, Clone)]
pub struct EventLog {
    events: Arc<Mutex<VecDeque<AppEvent>>>,
    max_capacity: usize,
}

impl EventLog {
    /// Creates a new event log with the given maximum capacity.
    ///
    /// ```
    /// use victauri_core::EventLog;
    ///
    /// let log = EventLog::new(100);
    /// assert!(log.is_empty());
    /// assert_eq!(log.capacity(), 100);
    /// ```
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_capacity))),
            max_capacity,
        }
    }

    /// Returns the maximum number of events this log can hold.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_capacity
    }

    /// Appends an event, evicting the oldest if at capacity.
    pub fn push(&self, event: AppEvent) {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if events.len() >= self.max_capacity {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Returns a clone of all events currently in the log.
    #[must_use]
    pub fn snapshot(&self) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// Returns a paginated slice of events starting at `offset`, up to `limit` items.
    #[must_use]
    pub fn snapshot_range(&self, offset: usize, limit: usize) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns all events with a timestamp at or after the given time.
    #[must_use]
    pub fn since(&self, timestamp: DateTime<Utc>) -> Vec<AppEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|e| e.timestamp() >= timestamp)
            .cloned()
            .collect()
    }

    /// Returns all IPC call events, filtering out non-IPC events.
    #[must_use]
    pub fn ipc_calls(&self) -> Vec<IpcCall> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|e| match e {
                AppEvent::Ipc(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    /// Returns IPC calls with a timestamp at or after the given time.
    #[must_use]
    pub fn ipc_calls_since(&self, timestamp: DateTime<Utc>) -> Vec<IpcCall> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|e| match e {
                AppEvent::Ipc(call) if call.timestamp >= timestamp => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    /// Returns the number of events currently in the log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Returns true if the log contains no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    /// Removes all events from the log.
    pub fn clear(&self) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}
