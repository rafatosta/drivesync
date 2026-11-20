//! Raw observation and retained-state fixtures for pure Decision tests.
//! Historical content relations represent prior verified measurements, not
//! relations inferred from the File's size, timestamps or event label.
use super::{context_behavior::{local_metadata, remote_metadata, local_input, remote_input, scope_input, path, remote_ref}, support::{item_context, unknown_observation}};
use drivesync_engine::{
    analyzer::{AnalysisContext, AnalysisItemContext, ContextAssociation, ObservationFreshness},
    inventory::{AnalysisEvidence, BaselineParent, BaselineSide, EvidenceRelation, ItemKind, PresenceState, SyncBaseline, SyncItemId},
    models::{File, FileData, FileEventAction, FileScope, LocalPresenceEvidence, MetadataValue, Timestamp},
};

pub(super) fn known_subject(id: &str) -> AnalysisItemContext {
    let mut subject = item_context(7);
    let local = local_metadata(&format!("local-{id}"), "folder/file.txt");
    let remote = remote_metadata("account-1", &format!("remote-{id}"), "file.txt");
    subject.item.id = SyncItemId(id.into());
    subject.item.local_file_id = local.local_id.clone();
    subject.item.remote_ref = Some(remote.reference.clone());
    subject.item.local = Some(local.clone());
    subject.item.local_latest = Some(local);
    subject.item.remote = Some(remote.clone());
    subject.item.remote_latest = Some(remote);
    subject.item.local_presence = PresenceState::Present;
    subject.item.remote_presence = PresenceState::Present;
    subject.item.baseline = Some(SyncBaseline {
        local: BaselineSide {
            presence: PresenceState::Present, name: Some("file.txt".into()), local_name_bytes: Some(b"file.txt".to_vec()),
            parent_identity: Some(BaselineParent::Local(SyncItemId("parent-a".into()))),
            version: None, checksum: None, size: Some(23), modified_at: None,
        },
        remote: BaselineSide {
            presence: PresenceState::Present, name: Some("file.txt".into()), local_name_bytes: None,
            parent_identity: Some(BaselineParent::Remote(remote_ref("account-1", "root"))),
            version: Some("7".into()), checksum: None, size: Some(23), modified_at: None,
        },
        confirmed_at: Timestamp { unix_millis: 10 },
    });
    subject.evidence = AnalysisEvidence {
        baseline_confirmed: true, local_scope_complete: true, remote_scope_complete: true,
        local_content: EvidenceRelation::Equal, remote_content: EvidenceRelation::Equal,
        current_content: EvidenceRelation::Equal, local_parent: Some(SyncItemId("parent-a".into())),
        current_parents: EvidenceRelation::Equal,
    };
    subject
}

/// A scope does not assert a new per-item observation; use retained facts only.
pub(super) fn scope(items: Vec<AnalysisItemContext>) -> AnalysisContext {
    AnalysisContext { file: scope_input(), association: ContextAssociation::Scope(items), observation: unknown_observation() }
}

/// Matched reobservation uses the exact retained local payload (equal marker).
pub(super) fn matched(subject: AnalysisItemContext, freshness: ObservationFreshness) -> AnalysisContext {
    let mut file = local_input(FileEventAction::Modified);
    if let Some(local) = &subject.item.local {
        file.scope = FileScope::Local(local.relative_path.clone());
        file.event.current = Some(drivesync_engine::models::FileLocation::Local(local.relative_path.clone()));
        file.data = Some(FileData::Local(local.clone()));
    }
    let mut observation = unknown_observation();
    observation.local_presence = PresenceState::Present;
    let mut subject = subject;
    subject.freshness = freshness;
    AnalysisContext { file, association: ContextAssociation::Matched(Box::new(subject)), observation }
}

pub(super) fn unmatched(local: bool) -> AnalysisContext {
    let file = if local { local_input(FileEventAction::Created) } else { remote_input() };
    let mut observation = unknown_observation();
    if local { observation.local_presence = PresenceState::Present; } else { observation.remote_presence = PresenceState::Present; }
    AnalysisContext { file, association: ContextAssociation::Unmatched, observation }
}

pub(super) fn clear_baseline(subject: &mut AnalysisItemContext) {
    subject.item.baseline = None;
    subject.evidence.baseline_confirmed = false;
    subject.evidence.local_content = EvidenceRelation::Unknown;
    subject.evidence.remote_content = EvidenceRelation::Unknown;
}

/// Absence is an explicit retained fact from verified observation of an identified
/// destination, not an inference from Option::None. Its producer/read port is a
/// future integration requirement, especially for a new remote destination.
pub(super) fn one_sided(local: bool, kind: ItemKind) -> AnalysisItemContext {
    let mut s = known_subject("one-sided");
    clear_baseline(&mut s);
    s.item.kind = kind.clone();
    if local {
        s.item.remote = None;
        s.item.remote_latest = None;
        s.item.remote_ref = None;
        s.item.remote_presence = PresenceState::AbsentConfirmed;
    } else {
        s.item.local = None;
        s.item.local_latest = None;
        s.item.local_file_id = None;
        s.item.local_presence = PresenceState::AbsentConfirmed;
    }
    s.evidence.current_content = EvidenceRelation::Unknown;
    if kind == ItemKind::Directory {
        if let Some(local) = s.item.local.as_mut() {
            local.kind = MetadataValue::Known(drivesync_engine::models::LocalEntryKind::Directory);
            local.mode = MetadataValue::Known(drivesync_engine::models::LocalFileMode { raw_mode: 0o40750, permission_bits: 0o750 });
        }
        if let Some(remote) = s.item.remote.as_mut() {
            remote.kind = MetadataValue::Known(drivesync_engine::models::RemoteEntryKind::Directory);
        }
        s.item.local_latest = s.item.local.clone();
        s.item.remote_latest = s.item.remote.clone();
    }
    s
}

pub(super) fn content(s: &mut AnalysisItemContext, local: EvidenceRelation, remote: EvidenceRelation, current: EvidenceRelation) {
    s.evidence.local_content = local;
    s.evidence.remote_content = remote;
    s.evidence.current_content = current;
    if local == EvidenceRelation::Different {
        for metadata in [&mut s.item.local, &mut s.item.local_latest].into_iter().flatten() {
            metadata.storage.size_bytes = MetadataValue::Known(31);
        }
    }
    if remote == EvidenceRelation::Different {
        for metadata in [&mut s.item.remote, &mut s.item.remote_latest].into_iter().flatten() {
            metadata.size = MetadataValue::Known(if current == EvidenceRelation::Equal { 31 } else { 37 });
            metadata.version = MetadataValue::Known("9".into());
        }
    }
}

pub(super) fn rename(s: &mut AnalysisItemContext, local: Option<&str>, remote: Option<&str>) {
    if let Some(name) = local {
        for m in [&mut s.item.local, &mut s.item.local_latest].into_iter().flatten() {
            let old = std::str::from_utf8(m.relative_path.as_bytes()).unwrap();
            let parent = old.rsplit_once('/').map(|(p, _)| p.to_owned()).unwrap_or_default();
            m.relative_path = path(&format!("{parent}/{name}"));
        }
    }
    if let Some(name) = remote {
        for m in [&mut s.item.remote, &mut s.item.remote_latest].into_iter().flatten() {
            m.name = MetadataValue::Known(name.into());
            m.version = MetadataValue::Known("8".into());
        }
    }
}

pub(super) fn move_to(s: &mut AnalysisItemContext, local: Option<&str>, remote: Option<&str>, parents: EvidenceRelation) {
    if let Some(parent) = local {
        for m in [&mut s.item.local, &mut s.item.local_latest].into_iter().flatten() {
            let name = std::str::from_utf8(m.relative_path.as_bytes()).unwrap().rsplit('/').next().unwrap().to_owned();
            m.relative_path = path(&format!("{parent}/{name}"));
        }
        s.evidence.local_parent = Some(SyncItemId(parent.into()));
    }
    if let Some(parent) = remote {
        for m in [&mut s.item.remote, &mut s.item.remote_latest].into_iter().flatten() {
            m.parent_ids = MetadataValue::Known(vec![drivesync_engine::models::RemoteFileId(parent.into())]);
            m.version = MetadataValue::Known("8".into());
        }
    }
    s.evidence.current_parents = parents;
}

pub(super) fn absent(s: &mut AnalysisItemContext, local: bool) {
    if local {
        s.item.local_presence = PresenceState::AbsentConfirmed;
        s.item.local_latest = None;
        s.evidence.local_content = EvidenceRelation::Unknown;
    } else {
        s.item.remote_presence = PresenceState::AbsentConfirmed;
        for m in [&mut s.item.remote, &mut s.item.remote_latest].into_iter().flatten() {
            m.trashed = MetadataValue::Known(true);
            m.version = MetadataValue::Known("8".into());
        }
        s.evidence.remote_content = EvidenceRelation::Unknown;
    }
    s.evidence.current_content = EvidenceRelation::Unknown;
}

pub(super) fn deleted_signal() -> AnalysisContext {
    let mut file = local_input(FileEventAction::Deleted);
    file.event.local_presence = Some(LocalPresenceEvidence::Unknown);
    let mut subject = known_subject("1");
    subject.freshness = ObservationFreshness::Newer;
    AnalysisContext { file, association: ContextAssociation::Matched(Box::new(subject)), observation: unknown_observation() }
}

pub(super) fn remote_removal_signal() -> AnalysisContext {
    use drivesync_engine::models::{RemoteObservation, RemoteObservationSource};
    let mut file: File = super::support::file(FileEventAction::RemovedFromScope, FileScope::Remote(remote_ref("account-1", "remote-1")));
    file.event.remote = Some(RemoteObservation { source: RemoteObservationSource::Changes, removed: Some(true), time: None, change_type: None, drive_id: None });
    let mut s = known_subject("1");
    s.freshness = ObservationFreshness::Newer;
    AnalysisContext { file, association: ContextAssociation::Matched(Box::new(s)), observation: unknown_observation() }
}
