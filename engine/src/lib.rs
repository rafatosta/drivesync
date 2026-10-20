//! Independent DriveSync engine library.
//! Domain modules remain private; user interfaces depend on this facade.
//! Sync functionality is intentionally not implemented yet.

mod auth;
mod accounts;
mod remote;
mod local;
mod state;
mod changes;
mod planner;
mod conflicts;
mod download;
mod upload;
mod queue;
mod coordinator;
mod notifications;
mod ipc;
mod configuration;
mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus { NotConfigured }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError { NotImplemented(&'static str) }

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::NotImplemented(operation) => write!(f, "{operation} is not implemented"), }
    }
}
impl std::error::Error for EngineError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyncRootId(pub u64);

#[derive(Debug, Default)]
pub struct DriveSyncEngine;

impl DriveSyncEngine {
    pub fn new() -> Self { Self }
    pub fn status(&self) -> EngineStatus { EngineStatus::NotConfigured }
    pub fn add_account(&self) -> Result<AccountId, EngineError> {
        Err(EngineError::NotImplemented("add_account"))
    }
    pub fn start_sync(&self, _root: SyncRootId) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented("start_sync"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unimplemented_operations_are_explicit() {
        let engine = DriveSyncEngine::new();
        assert_eq!(engine.status(), EngineStatus::NotConfigured);
        assert!(matches!(engine.add_account(), Err(EngineError::NotImplemented(_))));
        assert!(matches!(engine.start_sync(SyncRootId(1)), Err(EngineError::NotImplemented(_))));
    }
}
