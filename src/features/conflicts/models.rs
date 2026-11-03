//! Read-only presentation of durable observations; no saved entry authorizes writes.
use drivesync_engine::{
    ChangeKind, ConflictOutcome, ConflictRecord, ConflictStatus, ResolutionChoice,
    ResolutionRequest,
};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    pub category: Category,
    pub path: String,
    pub details: String,
    pub record: Option<ConflictRecord>,
}
/// Unknown metadata and historical preservation are explicit; no filesystem probe
/// follows an untrusted saved path and no timestamp selects a winning version.
pub(super) fn pending(records: &[ConflictRecord]) -> Vec<Item> {
    records.iter().filter(|r| r.status == ConflictStatus::Pending).map(|r| {
        let category = match r.diagnosis {
            ChangeKind::ConcurrentChanges | ChangeKind::LocationConflict | ChangeKind::EditDeleteConflict | ChangeKind::LocalIdentityChanged | ChangeKind::ConcurrentIdentityChanges => Category::Conflicts,
            ChangeKind::MetadataUnavailable | ChangeKind::Unsupported | ChangeKind::ExistingLocalData | ChangeKind::Conciliable => Category::Incompatibilities,
            _ => Category::Errors,
        };
        let path = r.relative_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| gettextrs::gettext("Path unavailable"));
        let mut details = gettextrs::gettext(r.diagnosis.description());
        details.push_str(&gettextrs::gettext("\nGoogle Drive version: {version}.").replace("{version}", &r.remote_version.map(|v| v.to_string()).unwrap_or_else(|| gettextrs::gettext("unavailable"))));
        details.push_str(&gettextrs::gettext("\nDates and sizes describe saved observations, not current files. Access is not checked now; loss of access does not prove deletion. Available originals were left in place; check current versions before choosing."));
        let size = |value: Option<u64>| value.map(|n| format!("{n} bytes")).unwrap_or_else(|| gettextrs::gettext("unavailable"));
        let modified = r.local_modified_unix.and_then(|seconds| adw::gtk::glib::DateTime::from_unix_utc(seconds).ok()).and_then(|date| date.format("%Y-%m-%d %H:%M:%S UTC").ok()).map(|text| text.to_string()).unwrap_or_else(|| gettextrs::gettext("unavailable"));
        details.push_str(&gettextrs::gettext("\nObserved local size: {local_size}. Local modified: {modified}.\nObserved Drive size: {remote_size}. Drive modified date: unavailable.")
            .replace("{local_size}", &size(r.local_size)).replace("{modified}", &modified).replace("{remote_size}", &size(r.remote_size)));
        if r.local_fingerprint.is_none() || r.remote_version.is_none() {
            details.push_str(&gettextrs::gettext("\nOne or both versions are unverified. Check current versions before choosing."));
        }
        for outcome in &r.outcomes {
            match outcome {
                ConflictOutcome::ResolutionApplied {..} => details.push_str(&gettextrs::gettext("\nAn earlier decision reached verified effects. A newer observation may still need review.")),
                ConflictOutcome::CopyPreserved { copy_path, remote_version } => details.push_str(&gettextrs::gettext("\nVerified preservation at the time of copying: Drive version {version} at {path}. Current copy availability is not checked.").replace("{version}", &remote_version.to_string()).replace("{path}", &copy_path.display().to_string())),
                ConflictOutcome::RetryBlocked { .. } => details.push_str(&gettextrs::gettext("\nThe last attempt was blocked. No resolution confirmed. Check current versions and retry; accepted decisions may require recovery.")),
                ConflictOutcome::ResolutionAborted { .. } => details.push_str(&gettextrs::gettext("\nA previous decision was aborted because evidence changed. Compare again.")),
            }
        }
        let record = ResolutionRequest::from_record(r, ResolutionChoice::KeepBoth).ok().map(|_| r.clone());
        details.push_str(&gettextrs::gettext(if record.is_some() {
            "\nChoose a version below. Confirmation checks both versions again. If either changed, check current versions and review again. Decide later keeps the item pending."
        } else {
            "\nNext step: restore access if needed and use Check current versions. Version choices require verified supported files at the confirmed location. Moves, unsupported types or missing evidence remain pending."
        }));
        Item { category, path, details, record }
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
            local_size: None,
            local_modified_unix: None,
            remote_size: None,
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
        records[0].local_size = Some(42);
        records[0].remote_size = Some(99);
        records[0].local_modified_unix = Some(0);
        let before = records.clone();
        let items = pending(&records);
        assert_eq!(items.len(), 12);
        assert_eq!(items, pending(&records));
        assert_eq!(records, before);
        assert_eq!(items[0].path, "<report>&.txt");
        assert!(items[0].details.contains("42 bytes"));
        assert!(items[0].details.contains("99 bytes"));
        assert!(items[0].details.contains("1970-01-01 00:00:00 UTC"));
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
    #[test]
    fn version_choices_require_supported_complete_pending_evidence() {
        let mut item = record(ChangeKind::ConcurrentChanges);
        assert!(pending(&[item.clone()])[0].record.is_none());
        item.remote_id = Some("file".into());
        item.remote_parent_id = Some("root".into());
        item.remote_name = Some("report.txt".into());
        item.remote_version = Some(2);
        item.local_fingerprint = Some(format!("{}:1:2", "a".repeat(64)));
        assert_eq!(pending(&[item.clone()])[0].record, Some(item.clone()));
        for diagnosis in [
            ChangeKind::LocalIdentityChanged,
            ChangeKind::ConcurrentIdentityChanges,
        ] {
            item.diagnosis = diagnosis;
            let rows = pending(&[item.clone()]);
            assert!(rows[0].record.is_some());
            assert_eq!(rows[0].category, Category::Conflicts);
        }
        item.diagnosis = ChangeKind::Unknown;
        assert!(pending(&[item.clone()])[0].record.is_none());
        item.diagnosis = ChangeKind::Conciliable;
        assert!(pending(&[item.clone()])[0].record.is_none());
        item.diagnosis = ChangeKind::Unsupported;
        assert!(pending(&[item.clone()])[0].record.is_none());
        for diagnosis in [
            ChangeKind::AccessDenied,
            ChangeKind::LocalAccessDenied,
            ChangeKind::MetadataUnavailable,
        ] {
            item.diagnosis = diagnosis;
            let rows = pending(&[item.clone()]);
            assert!(rows[0].record.is_none());
            assert_ne!(rows[0].category, Category::Conflicts);
        }
        item.status = ConflictStatus::Resolved;
        assert!(pending(&[item]).is_empty());
    }
}
