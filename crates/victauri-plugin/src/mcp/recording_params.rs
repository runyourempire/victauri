use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `start_recording` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartRecordingParams {
    /// Optional session ID. If omitted, a UUID is generated.
    pub session_id: Option<String>,
}

/// Parameters for the `checkpoint` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckpointParams {
    /// Unique ID for this checkpoint.
    pub id: String,
    /// Optional human-readable label for the checkpoint.
    pub label: Option<String>,
    /// State snapshot as JSON to associate with this checkpoint.
    pub state: serde_json::Value,
}

/// Parameters for the `get_replay` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayParams {
    /// Only return events after this index.
    pub since_index: Option<usize>,
}

/// Parameters for the `events_between_checkpoints` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventsBetweenCheckpointsParams {
    /// Checkpoint ID to start from.
    pub from_checkpoint: String,
    /// Checkpoint ID to end at.
    pub to_checkpoint: String,
}

/// Parameters for the `import_session` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportSessionParams {
    /// JSON string of a previously exported RecordedSession.
    pub session_json: String,
}
