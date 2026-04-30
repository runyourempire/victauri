//! Typed error enum for the `victauri-core` crate.

use std::fmt;

/// Errors that can occur within `victauri-core` operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VictauriError {
    /// A capacity limit (event log, checkpoints, etc.) was exceeded.
    #[error("capacity limit exceeded: {message}")]
    CapacityExceeded {
        /// Description of which capacity was exceeded.
        message: String,
    },

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An input parameter was invalid or out of range.
    #[error("invalid input: {message}")]
    InvalidInput {
        /// Description of what was invalid.
        message: String,
    },

    /// Attempted an operation that requires an active recording session.
    #[error("no active recording session")]
    NoActiveRecording,

    /// Attempted to start a recording when one is already active.
    #[error("recording session already active")]
    RecordingAlreadyActive,

    /// Referenced a checkpoint ID that does not exist.
    #[error("checkpoint not found: {id}")]
    CheckpointNotFound {
        /// The checkpoint ID that was not found.
        id: String,
    },

    /// Referenced a command name that is not registered.
    #[error("command not found: {name}")]
    CommandNotFound {
        /// The command name that was not found.
        name: String,
    },

    /// Referenced a ref handle that is invalid or expired.
    #[error("invalid ref handle: {ref_id}")]
    InvalidRefHandle {
        /// The ref handle ID that was invalid.
        ref_id: String,
    },

    /// Used an assertion condition that is not recognized.
    #[error("unknown assertion condition: {condition}")]
    UnknownCondition {
        /// The condition string that was not recognized.
        condition: String,
    },
}

/// Convenience alias for `std::result::Result<T, VictauriError>`.
pub type Result<T> = std::result::Result<T, VictauriError>;

impl VictauriError {
    /// Create a [`CapacityExceeded`](Self::CapacityExceeded) error.
    #[must_use]
    pub fn capacity_exceeded(message: impl fmt::Display) -> Self {
        Self::CapacityExceeded {
            message: message.to_string(),
        }
    }
    /// Create an [`InvalidInput`](Self::InvalidInput) error.
    #[must_use]
    pub fn invalid_input(message: impl fmt::Display) -> Self {
        Self::InvalidInput {
            message: message.to_string(),
        }
    }
}
