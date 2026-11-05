//! Durable local identity/state and conservative investigation work, without sync.
mod models;
mod store;
pub use models::{
    InvestigationReason, ItemId, ItemSnapshot, ItemState, LocalConnectivity, LocalStateCheckpoint,
    LocalStateError, PendingInvestigation, TrackedItem, WorkPriority,
};
pub use store::LocalStateStore;
