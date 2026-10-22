//! Unit tests for explicit unsupported engine operations.
use super::*;

#[test]
fn unimplemented_operations_are_explicit() {
    let engine = DriveSyncEngine::new();
    assert_eq!(engine.status(), EngineStatus::NotConfigured);
    assert!(matches!(
        engine.start_sync(SyncRootId(1)),
        Err(EngineError::NotImplemented(_))
    ));
}
