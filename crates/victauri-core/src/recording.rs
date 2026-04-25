use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::event::{AppEvent, IpcCall};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    pub id: String,
    pub label: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub state: serde_json::Value,
    pub event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub events: Vec<RecordedEvent>,
    pub checkpoints: Vec<StateCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    pub index: usize,
    pub timestamp: DateTime<Utc>,
    pub event: AppEvent,
}

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
    checkpoints: Vec<StateCheckpoint>,
    event_counter: usize,
    max_events: usize,
}

impl EventRecorder {
    pub fn new(max_events: usize) -> Self {
        Self {
            recording: Arc::new(Mutex::new(None)),
            max_events,
        }
    }

    pub fn start(&self, session_id: String) -> bool {
        let mut rec = self.recording.lock().unwrap();
        if rec.is_some() {
            return false;
        }
        *rec = Some(ActiveRecording {
            session_id,
            started_at: Utc::now(),
            events: VecDeque::new(),
            checkpoints: Vec::new(),
            event_counter: 0,
            max_events: self.max_events,
        });
        true
    }

    pub fn stop(&self) -> Option<RecordedSession> {
        let mut rec = self.recording.lock().unwrap();
        rec.take().map(|r| RecordedSession {
            id: r.session_id,
            started_at: r.started_at,
            events: r.events.into_iter().collect(),
            checkpoints: r.checkpoints,
        })
    }

    pub fn is_recording(&self) -> bool {
        self.recording.lock().unwrap().is_some()
    }

    pub fn record_event(&self, event: AppEvent) {
        let mut rec = self.recording.lock().unwrap();
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

    pub fn checkpoint(&self, id: String, label: Option<String>, state: serde_json::Value) -> bool {
        let mut rec = self.recording.lock().unwrap();
        if let Some(ref mut active) = *rec {
            let event_index = active.event_counter;
            active.checkpoints.push(StateCheckpoint {
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

    pub fn event_count(&self) -> usize {
        self.recording
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.events.len())
            .unwrap_or(0)
    }

    pub fn checkpoint_count(&self) -> usize {
        self.recording
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.checkpoints.len())
            .unwrap_or(0)
    }

    pub fn events_since(&self, index: usize) -> Vec<RecordedEvent> {
        let rec = self.recording.lock().unwrap();
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

    pub fn events_between(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<RecordedEvent> {
        let rec = self.recording.lock().unwrap();
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

    pub fn get_checkpoints(&self) -> Vec<StateCheckpoint> {
        let rec = self.recording.lock().unwrap();
        match rec.as_ref() {
            Some(active) => active.checkpoints.clone(),
            None => Vec::new(),
        }
    }

    pub fn events_between_checkpoints(
        &self,
        from_checkpoint_id: &str,
        to_checkpoint_id: &str,
    ) -> Option<Vec<RecordedEvent>> {
        let rec = self.recording.lock().unwrap();
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

    pub fn ipc_replay_sequence(&self) -> Vec<IpcCall> {
        let rec = self.recording.lock().unwrap();
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

fn extract_timestamp(event: &AppEvent) -> DateTime<Utc> {
    match event {
        AppEvent::Ipc(call) => call.timestamp,
        AppEvent::StateChange { timestamp, .. } => *timestamp,
        AppEvent::DomMutation { timestamp, .. } => *timestamp,
        AppEvent::WindowEvent { timestamp, .. } => *timestamp,
    }
}
