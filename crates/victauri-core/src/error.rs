//! Typed error enum for the `victauri-core` crate.

use std::fmt;

/// Errors that can occur within `victauri-core` operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VictauriError {
    #[error("capacity limit exceeded: {message}")]
    CapacityExceeded { message: String },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid input: {message}")]
    InvalidInput { message: String },

    #[error("no active recording session")]
    NoActiveRecording,

    #[error("recording session already active")]
    RecordingAlreadyActive,

    #[error("checkpoint not found: {id}")]
    CheckpointNotFound { id: String },

    #[error("command not found: {name}")]
    CommandNotFound { name: String },

    #[error("invalid ref handle: {ref_id}")]
    InvalidRefHandle { ref_id: String },

    #[error("unknown assertion condition: {condition}")]
    UnknownCondition { condition: String },
}

pub type Result<T> = std::result::Result<T, VictauriError>;

impl VictauriError {
    pub fn capacity_exceeded(message: impl fmt::Display) -> Self {
        Self::CapacityExceeded {
            message: message.to_string(),
        }
    }
    pub fn invalid_input(message: impl fmt::Display) -> Self {
        Self::InvalidInput {
            message: message.to_string(),
        }
    }
}
