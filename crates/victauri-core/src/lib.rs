//! Core types and protocol for Victauri — full-stack introspection for Tauri apps via MCP.
//!
//! This crate provides the shared type system used by all Victauri crates.
//! It has no Tauri dependency and can be used independently for testing.

pub mod event;
pub mod recording;
pub mod registry;
pub mod snapshot;
pub mod types;
pub mod verification;

pub use event::{AppEvent, EventLog, IpcCall, IpcResult};
pub use recording::{EventRecorder, RecordedEvent, RecordedSession, StateCheckpoint};
pub use registry::{CommandArg, CommandInfo, CommandRegistry, ScoredCommand};
pub use snapshot::{DomElement, DomSnapshot, WindowState};
pub use types::{MemoryDelta, RefHandle, VerificationResult};
pub use verification::{
    AssertionResult, GhostCommand, GhostCommandReport, GhostSource, IpcIntegrityReport,
    SemanticAssertion, check_ipc_integrity, detect_ghost_commands, evaluate_assertion,
    verify_state,
};
