//! Explicit Analyzer decisions and raw execution-side facts.
use super::decision_fixtures::{known_subject, scope, one_sided, content, rename, move_to, absent, unmatched};
use drivesync_engine::{
    analyzer::{AnalysisContext, AnalysisDecision, AnalysisItemDecision, AnalysisItemResult, AnalysisResult, ApplyOutcome, ContextAssociation, DecisionOutcome, DecisionTarget, NonEmpty},
    analyzer::apply::{ApplySnapshot, ParentBinding},
    inventory::{AnalysisIntent, AnalysisRequirement, EvidenceRelation, ItemKind, OperationDirection, OperationKind, SyncItemId},
    models::{AccountId, FileData, MetadataValue, ProviderId, RemoteFileId},
    operations::*,
};

pub(super) fn action(direction: OperationDirection, kind: OperationKind) -> DecisionOutcome {
    DecisionOutcome::ActionSelected { intents: NonEmpty::new(AnalysisIntent { direction, kind }, vec![]) }
}
pub(super) fn need(requirement: AnalysisRequirement) -> DecisionOutcome {
    DecisionOutcome::InsufficientEvidence { missing: NonEmpty::new(requirement, vec![]) }
}
pub(super) fn decision(context: AnalysisContext, outcome: DecisionOutcome) -> AnalysisDecision {
    let target = match &context.association {
        ContextAssociation::Matched(s) => DecisionTarget::Item { id: s.item.id.clone(), revision: s.item.revision },
        ContextAssociation::Scope(items) if items.len() == 1 => DecisionTarget::Item { id: items[0].item.id.clone(), revision: items[0].item.revision },
        ContextAssociation::Scope(_) => DecisionTarget::Synchronization,
        _ => DecisionTarget::Observation(context.file.id.clone()),
    };
    AnalysisDecision { context, items: vec![AnalysisItemDecision { target, findings: vec![], outcome }] }
}
pub(super) fn basic(outcome: DecisionOutcome) -> AnalysisDecision {
    decision(scope(vec![known_subject("1")]), outcome)
}
pub(super) fn creation(local: bool, kind: ItemKind) -> AnalysisDecision {
    let direction = if local { OperationDirection::Upload } else { OperationDirection::Download };
    decision(scope(vec![one_sided(local, kind)]), action(direction, OperationKind::Create))
}
pub(super) fn mutation(kind: OperationKind, direction: OperationDirection) -> AnalysisDecision {
    use EvidenceRelation::{Different, Equal};
    let mut s = known_subject("1");
    let upload = direction == OperationDirection::Upload;
    match kind {
        OperationKind::UpdateContent => if upload { content(&mut s, Different, Equal, Different); } else { content(&mut s, Equal, Different, Different); },
        OperationKind::Rename => if upload { rename(&mut s, Some("new.txt"), None); } else { rename(&mut s, None, Some("new.txt")); },
        OperationKind::Move => if upload { move_to(&mut s, Some("parent-b"), None, Different); } else { move_to(&mut s, None, Some("remote-b"), Different); },
        OperationKind::Trash => absent(&mut s, upload),
        _ => {},
    }
    decision(scope(vec![s]), action(direction, kind))
}
pub(super) fn multi() -> AnalysisDecision {
    let mut s = known_subject("1");
    content(&mut s, EvidenceRelation::Different, EvidenceRelation::Equal, EvidenceRelation::Different);
    rename(&mut s, Some("new.txt"), None);
    decision(scope(vec![s]), DecisionOutcome::ActionSelected { intents: NonEmpty::new(
        AnalysisIntent { direction: OperationDirection::Upload, kind: OperationKind::UpdateContent },
        vec![AnalysisIntent { direction: OperationDirection::Upload, kind: OperationKind::Rename }],
    ) })
}
pub(super) fn independent() -> AnalysisDecision {
    let mut a = known_subject("a");
    rename(&mut a, Some("local.txt"), Some("remote.txt"));
    let mut b = known_subject("b");
    content(&mut b, EvidenceRelation::Different, EvidenceRelation::Equal, EvidenceRelation::Different);
    let mut d = decision(scope(vec![a, b]), DecisionOutcome::Conflict { requirements: vec![] });
    d.items[0].target = DecisionTarget::Item { id: SyncItemId("a".into()), revision: 7 };
    d.items.push(AnalysisItemDecision { target: DecisionTarget::Item { id: SyncItemId("b".into()), revision: 7 }, findings: vec![], outcome: action(OperationDirection::Upload, OperationKind::UpdateContent) });
    d
}
pub(super) fn new_observation() -> AnalysisDecision {
    decision(unmatched(true), need(AnalysisRequirement::ObserveRemote))
}
pub(super) fn snapshot(d: &AnalysisDecision) -> ApplySnapshot {
    let dir = |id: &str, local: &str, remote: &str| ParentBinding {
        item_id: SyncItemId(id.into()),
        local: Some(DirectoryRef::Local { relative_path: super::context_behavior::path(local), local_id: Some(drivesync_engine::models::LocalFileId(format!("dir-{id}"))) }),
        remote: Some(DirectoryRef::Remote { provider_id: ProviderId("google".into()), reference: super::context_behavior::remote_ref("account-1", remote) }),
    };
    ApplySnapshot {
        scope: ExecutionScope { sync_id: d.context.file.sync_id.clone(), account_id: AccountId("account-1".into()), local_root: b"/mirror".to_vec(), remote_root_id: RemoteFileId("root".into()) },
        current_item: d.context.association.items().first().map(|s| s.item.clone()),
        related_parents: vec![dir("parent-a", "folder", "root"), dir("parent-b", "parent-b", "remote-b")],
        deletion_authorized: true,
    }
}
pub(super) fn outcome(result: AnalysisResult) -> ApplyOutcome {
    match result {
        AnalysisResult::Completed(mut items) if items.len() == 1 => items.remove(0).outcome,
        other => panic!("expected one completed target, got {other:?}"),
    }
}
pub(super) fn applied_result(local: bool, verified: bool) -> ItemResult {
    let verification = if verified { VerificationStatus::Verified } else { VerificationStatus::Unverified };
    if local {
        let metadata = super::context_behavior::local_metadata("new-local", "folder/file.txt");
        ItemResult { resource: ResourceRef::Local { relative_path: metadata.relative_path.clone(), local_id: metadata.local_id.clone() }, metadata: Some(FileData::Local(metadata)), verification }
    } else {
        let mut metadata = super::context_behavior::remote_metadata("account-1", "new-remote", "file.txt");
        metadata.version = MetadataValue::Known("10".into());
        ItemResult { resource: ResourceRef::Remote { provider_id: metadata.provider.clone(), reference: metadata.reference.clone() }, metadata: Some(FileData::Remote(metadata)), verification }
    }
}
