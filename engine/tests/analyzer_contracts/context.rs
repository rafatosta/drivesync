//! Artifact shape fixtures only; behavioral oracles live in context_behavior.rs.
use super::support::{file, item_context, unknown_observation};
use drivesync_engine::{
    analyzer::{AnalysisContext, ContextAssociation},
    models::{AccountId, FileEventAction, FileScope, RemoteFileId, RemoteFileRef},
};

#[test]
fn remote_creation_without_inventory_entry_has_empty_item_context() {
    let scope = FileScope::Remote(RemoteFileRef {
        account_id: AccountId("account-1".into()),
        file_id: RemoteFileId("remote-1".into()),
    });
    let input = file(FileEventAction::Created, scope);
    let expected = AnalysisContext {
        file: input.clone(),
        association: ContextAssociation::Unmatched,
        observation: unknown_observation(), facts: Default::default(),
    };

    let context = AnalysisContext {
        file: input,
        association: ContextAssociation::Unmatched,
        observation: unknown_observation(), facts: Default::default(),
    };

    assert_eq!(context, expected);
    assert!(context.association.items().is_empty());
    assert_eq!(context.file.event.action, FileEventAction::Created);
}

#[test]
fn context_keeps_inventory_snapshot_and_its_evidence_together() {
    let input = file(FileEventAction::Modified, FileScope::WholeSync);
    let subject = item_context(12);
    let expected = AnalysisContext {
        file: input.clone(),
        association: ContextAssociation::Scope(vec![subject.clone()]),
        observation: unknown_observation(), facts: Default::default(),
    };

    let context = AnalysisContext {
        file: input,
        association: ContextAssociation::Scope(vec![subject]),
        observation: unknown_observation(), facts: Default::default(),
    };

    assert_eq!(context, expected);
    assert_eq!(context.association.items()[0].item.revision, 12);
    assert!(!context.association.items()[0].evidence.baseline_confirmed);
}

#[test]
fn whole_synchronization_scope_can_carry_multiple_inventory_snapshots() {
    let mut first = item_context(3);
    first.item.id.0 = "item-first".into();
    let mut second = item_context(8);
    second.item.id.0 = "item-second".into();
    let expected = AnalysisContext {
        file: file(FileEventAction::RecheckRequired, FileScope::WholeSync),
        association: ContextAssociation::Scope(vec![first, second]),
        observation: unknown_observation(), facts: Default::default(),
    };

    assert_eq!(expected.association.items().len(), 2);
    assert_eq!(expected.association.items()[0].item.revision, 3);
    assert_eq!(expected.association.items()[1].item.revision, 8);
    assert_eq!(expected.file.event.action, FileEventAction::RecheckRequired);
}
