//! Read-only presentation of durable observations; no saved entry authorizes writes.
use drivesync_engine::{ChangeKind, ConflictOutcome, ConflictRecord, ConflictStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Category {
    Conflicts,
    Incompatibilities,
    Errors,
}
impl Category {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Conflicts => "Conflicts",
            Self::Incompatibilities => "Incompatibilities",
            Self::Errors => "Errors",
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Item {
    pub category: Category,
    pub path: String,
    pub details: String,
}
/// Unknown metadata and historical preservation are explicit; no filesystem probe
/// follows an untrusted saved path and no timestamp selects a winning version.
pub(super) fn pending(records: &[ConflictRecord]) -> Vec<Item> {
    records.iter().filter(|r| r.status == ConflictStatus::Pending).map(|r| {
        let category = match r.diagnosis {
            ChangeKind::ConcurrentChanges | ChangeKind::LocationConflict | ChangeKind::EditDeleteConflict => Category::Conflicts,
            ChangeKind::Unsupported | ChangeKind::LocalIdentityChanged | ChangeKind::ConcurrentIdentityChanges | ChangeKind::ExistingLocalData | ChangeKind::Conciliable => Category::Incompatibilities,
            _ => Category::Errors,
        };
        let path = r.relative_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Path unavailable".into());
        let mut details = format!("{}\nLocal version: {}\nGoogle Drive version: {}\nDate and size: unavailable in the saved observation.\nAccess: not checked now; loss of access does not prove deletion.\nPreservation: available originals were left in place; their current availability requires a new check.", r.diagnosis.description(), if r.local_fingerprint.is_some() { "observed" } else { "unverified" }, r.remote_version.map(|v| v.to_string()).unwrap_or_else(|| "unverified".into()));
        for outcome in &r.outcomes {
            match outcome {
                ConflictOutcome::CopyPreserved { copy_path, remote_version } => details.push_str(&format!("\nVerified preservation at the time of copying: Drive version {remote_version} at {}. Current copy availability is not checked.", copy_path.display())),
                ConflictOutcome::RetryBlocked { reason } => details.push_str(&format!("\nLast blocked attempt: {reason:?}. No resolution confirmed.")),
                ConflictOutcome::ResolutionAborted { .. } => details.push_str("\nA previous decision was aborted because evidence changed. Compare again."),
            }
        }
        details.push_str("\nNext step: restore access if needed, synchronize again and compare available versions. Keeping local, remote or both through DriveSync is not available yet. Deciding later leaves this item pending and selects no version.");
        Item { category, path, details }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(kind: ChangeKind) -> ConflictRecord {
        ConflictRecord {
            conflict_id: "item".into(),
            remote_id: None,
            remote_parent_id: None,
            remote_name: None,
            relative_path: Some("<report>&.txt".into()),
            diagnosis: kind,
            status: ConflictStatus::Pending,
            local_fingerprint: None,
            remote_version: None,
            outcomes: vec![],
        }
    }
    #[test]
    fn p03_defer_is_read_only_complete_and_repeatable() {
        let mut records = vec![record(ChangeKind::ConcurrentChanges); 12];
        records[0].outcomes.push(ConflictOutcome::CopyPreserved {
            copy_path: "copy.txt".into(),
            remote_version: 3,
        });
        let before = records.clone();
        let items = pending(&records);
        assert_eq!(items.len(), 12);
        assert_eq!(items, pending(&records));
        assert_eq!(records, before);
        assert_eq!(items[0].path, "<report>&.txt");
        assert!(items[0].details.contains("version 3 at copy.txt"));
        assert!(items[1].details.contains("unverified"));
        assert!(!items[1].details.contains("version 3"));
        records[0].status = ConflictStatus::NoLongerConflicting;
        assert_eq!(pending(&records).len(), 11);
    }
    #[test]
    fn c22_c25_unavailable_is_an_error_never_a_deletion_claim() {
        for kind in [
            ChangeKind::RemoteUnavailable,
            ChangeKind::LocalChangedRemoteUnavailable,
            ChangeKind::Unknown,
        ] {
            let item = pending(&[record(kind)]).remove(0);
            assert_eq!(item.category, Category::Errors);
            assert!(item.details.contains("does not prove deletion"));
            assert!(item.details.contains("restore access"));
        }
        assert_eq!(
            pending(&[record(ChangeKind::Unsupported)])[0].category,
            Category::Incompatibilities
        );
    }
}
