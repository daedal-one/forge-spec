//! Typed, validated workspace mutation protocol.

mod engine;
mod operation;

pub use engine::{
    atomic_write_files, content_fingerprint, ChangePlan, MutationEngine, MutationOutcome,
    MutationTextEdit,
};
pub use operation::{ChangeRequest, Operation, CHANGE_SCHEMA};
