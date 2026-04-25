pub mod event;
pub mod registry;
pub mod snapshot;
pub mod types;
pub mod verification;

pub use event::{AppEvent, EventLog, IpcCall};
pub use registry::{CommandArg, CommandInfo, CommandRegistry};
pub use snapshot::{DomElement, DomSnapshot, WindowState};
pub use types::{MemoryDelta, RefHandle, VerificationResult};
pub use verification::{
    GhostCommand, GhostCommandReport, GhostSource, IpcIntegrityReport, check_ipc_integrity,
    detect_ghost_commands, verify_state,
};
