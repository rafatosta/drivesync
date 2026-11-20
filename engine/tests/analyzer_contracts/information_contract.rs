//! Schema tests independent of detector implementations.
use drivesync_engine::models::*;
use super::context_behavior::local_input;
#[test]
fn missing_fields_are_explicit() {
    let f = local_input(FileEventAction::Modified);
    assert_eq!(f.evidence.digest, MetadataValue::NotCollected);
    assert_eq!(f.evidence.capabilities, MetadataValue::NotCollected);
    assert!(f.validate().is_ok());
}
#[test]
fn invalid_digest_rejected() {
    let mut f = local_input(FileEventAction::Modified);
    f.evidence.digest = MetadataValue::Known(ContentDigest { algorithm: DigestAlgorithm::Sha256, hex: "invalid".into() });
    assert!(f.validate().is_err());
}
#[test]
fn measured_digest_preserved() {
    let mut f = local_input(FileEventAction::Modified);
    f.evidence.digest = MetadataValue::Known(ContentDigest { algorithm: DigestAlgorithm::Sha256, hex: "a".repeat(64) });
    assert!(f.validate().is_ok());
    assert_eq!(f.clone().evidence, f.evidence);
}
#[test]
fn contradictory_presence_rejected() {
    let mut f = local_input(FileEventAction::Modified);
    f.evidence.presence = MetadataValue::Known(ObservedPresence::AbsentConfirmed);
    assert!(f.validate().is_err());
}
#[test]
fn explicit_unavailability_preserved() {
    let mut f = local_input(FileEventAction::Modified);
    f.evidence.digest = MetadataValue::PermissionDenied;
    assert!(f.validate().is_ok());
    assert_eq!(f.clone().evidence.digest, MetadataValue::PermissionDenied);
}
