use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::event::{AppEvent, IpcCall};

/// A snapshot of application state taken at a specific point during recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Unique identifier for this checkpoint.
    pub id: String,
    /// Optional human-readable label for the checkpoint.
    pub label: Option<String>,
    /// When the checkpoint was created.
    pub timestamp: DateTime<Utc>,
    /// Serialized application state at the checkpoint.
    pub state: serde_json::Value,
    /// Index into the event stream at the time of this checkpoint.
    pub event_index: usize,
}

/// A complete recorded session with events and state checkpoints. Serializable for export/import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    /// Unique session identifier (UUID).
    pub id: String,
    /// When the recording session began.
    pub started_at: DateTime<Utc>,
    /// All events captured during the session, in order.
    pub events: Vec<RecordedEvent>,
    /// State checkpoints created during the session.
    pub checkpoints: Vec<StateCheckpoint>,
}

/// A single event captured during a recording session, with its sequence index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// Monotonically increasing sequence number within the recording session.
    pub index: usize,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// The captured application event.
    pub event: AppEvent,
}

/// Thread-safe session recorder for time-travel debugging. Records events and
/// state checkpoints during a recording session. Only one session can be active at a time.
#[derive(Debug, Clone)]
pub struct EventRecorder {
    recording: Arc<Mutex<Option<ActiveRecording>>>,
    max_events: usize,
}

#[derive(Debug, Clone)]
struct ActiveRecording {
    session_id: String,
    started_at: DateTime<Utc>,
    events: VecDeque<RecordedEvent>,
    checkpoints: VecDeque<StateCheckpoint>,
    event_counter: usize,
    max_events: usize,
    max_checkpoints: usize,
}

impl EventRecorder {
    /// Creates a new recorder with the given maximum event capacity.
    ///
    /// ```
    /// use victauri_core::EventRecorder;
    ///
    /// let recorder = EventRecorder::new(1000);
    /// assert!(!recorder.is_recording());
    /// assert_eq!(recorder.event_count(), 0);
    /// ```
    pub fn new(max_events: usize) -> Self {
        Self {
            recording: Arc::new(Mutex::new(None)),
            max_events,
        }
    }

    /// Starts a new recording session; returns false if one is already active.
    pub fn start(&self, session_id: String) -> bool {
        let mut rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        if rec.is_some() {
            return false;
        }
        *rec = Some(ActiveRecording {
            session_id,
            started_at: Utc::now(),
            events: VecDeque::new(),
            checkpoints: VecDeque::new(),
            event_counter: 0,
            max_events: self.max_events,
            max_checkpoints: 1000,
        });
        true
    }

    /// Stops the active recording and returns the completed session, or None if not recording.
    pub fn stop(&self) -> Option<RecordedSession> {
        let mut rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        rec.take().map(|r| RecordedSession {
            id: r.session_id,
            started_at: r.started_at,
            events: r.events.into_iter().collect(),
            checkpoints: r.checkpoints.into_iter().collect(),
        })
    }

    /// Returns true if a recording session is currently active.
    pub fn is_recording(&self) -> bool {
        self.recording
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Appends an event to the active recording, evicting the oldest if at capacity.
    pub fn record_event(&self, event: AppEvent) {
        let mut rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut active) = *rec {
            let timestamp = extract_timestamp(&event);
            let index = active.event_counter;
            active.event_counter += 1;

            if active.events.len() >= active.max_events {
                active.events.pop_front();
            }

            active.events.push_back(RecordedEvent {
                index,
                timestamp,
                event,
            });
        }
    }

    /// Creates a named state checkpoint at the current event index; returns false if not recording.
    pub fn checkpoint(&self, id: String, label: Option<String>, state: serde_json::Value) -> bool {
        let mut rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut active) = *rec {
            let event_index = active.event_counter;
            if active.checkpoints.len() >= active.max_checkpoints {
                active.checkpoints.pop_front();
            }
            active.checkpoints.push_back(StateCheckpoint {
                id,
                label,
                timestamp: Utc::now(),
                state,
                event_index,
            });
            true
        } else {
            false
        }
    }

    /// Returns the number of events recorded so far, or 0 if not recording.
    pub fn event_count(&self) -> usize {
        self.recording
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|r| r.events.len())
            .unwrap_or(0)
    }

    /// Returns the number of checkpoints created so far, or 0 if not recording.
    pub fn checkpoint_count(&self) -> usize {
        self.recording
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|r| r.checkpoints.len())
            .unwrap_or(0)
    }

    /// Returns all events with an index >= the given value.
    pub fn events_since(&self, index: usize) -> Vec<RecordedEvent> {
        let rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        match rec.as_ref() {
            Some(active) => active
                .events
                .iter()
                .filter(|e| e.index >= index)
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Returns events whose timestamps fall within the given inclusive range.
    pub fn events_between(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<RecordedEvent> {
        let rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        match rec.as_ref() {
            Some(active) => active
                .events
                .iter()
                .filter(|e| e.timestamp >= from && e.timestamp <= to)
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Returns all checkpoints from the active recording session.
    pub fn get_checkpoints(&self) -> Vec<StateCheckpoint> {
        let rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        match rec.as_ref() {
            Some(active) => active.checkpoints.iter().cloned().collect(),
            None => Vec::new(),
        }
    }

    /// Returns events recorded between two named checkpoints, or None if either ID is unknown.
    pub fn events_between_checkpoints(
        &self,
        from_checkpoint_id: &str,
        to_checkpoint_id: &str,
    ) -> Option<Vec<RecordedEvent>> {
        let rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        let active = rec.as_ref()?;

        let from_idx = active
            .checkpoints
            .iter()
            .find(|c| c.id == from_checkpoint_id)?
            .event_index;
        let to_idx = active
            .checkpoints
            .iter()
            .find(|c| c.id == to_checkpoint_id)?
            .event_index;

        let (start, end) = if from_idx <= to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };

        Some(
            active
                .events
                .iter()
                .filter(|e| e.index >= start && e.index < end)
                .cloned()
                .collect(),
        )
    }

    /// Extracts IPC calls in order from the recording for replay.
    pub fn ipc_replay_sequence(&self) -> Vec<IpcCall> {
        let rec = self.recording.lock().unwrap_or_else(|e| e.into_inner());
        match rec.as_ref() {
            Some(active) => active
                .events
                .iter()
                .filter_map(|re| match &re.event {
                    AppEvent::Ipc(call) => Some(call.clone()),
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        }
    }
}

impl Default for EventRecorder {
    fn default() -> Self {
        Self::new(50_000)
    }
}

fn extract_timestamp(event: &AppEvent) -> DateTime<Utc> {
    match event {
        AppEvent::Ipc(call) => call.timestamp,
        AppEvent::StateChange { timestamp, .. } => *timestamp,
        AppEvent::DomMutation { timestamp, .. } => *timestamp,
        AppEvent::WindowEvent { timestamp, .. } => *timestamp,
    }
}
