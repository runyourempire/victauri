use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartRecordingParams {
    /// Optional session ID. If omitted, a UUID is generated.
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckpointParams {
    /// Unique ID for this checkpoint.
    pub id: String,
    /// Optional human-readable label for the checkpoint.
    pub label: Option<String>,
    /// State snapshot as JSON to associate with this checkpoint.
    pub state: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayParams {
    /// Only return events after this index.
    pub since_index: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventsBetweenCheckpointsParams {
    /// Checkpoint ID to start from.
    pub from_checkpoint: String,
    /// Checkpoint ID to end at.
    pub to_checkpoint: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportSessionParams {
    /// JSON string of a previously exported RecordedSession.
    pub session_json: String,
}
