//! Pure dashboard decisions. Configuration, operation and pending observations
//! remain independent; a running job never hides an unresolved observation.
use crate::features::synchronization::automatic::{Phase, Snapshot};
use drivesync_engine::{ChangeKind, DownloadReport, EngineError};
use std::collections::HashSet;

/// Counts observed pending paths, not transfers or durable resolution decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Attention {
    pub conflicts: usize,
    pub problems: usize,
}
impl Attention {
    pub fn from_report(report: &DownloadReport) -> Self {
        let mut seen = HashSet::new();
        let pending: Vec<_> = report
            .attention()
            .into_iter()
            .filter(|item| seen.insert(item.relative_path.clone()))
            .collect();
        let conflicts: HashSet<_> = report
            .reconciliation
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    ChangeKind::ConcurrentChanges
                        | ChangeKind::LocationConflict
                        | ChangeKind::EditDeleteConflict
                        | ChangeKind::LocalIdentityChanged
                        | ChangeKind::ConcurrentIdentityChanges
                )
            })
            .map(|e| e.relative_path.clone())
            .chain(
                report
                    .issues
                    .iter()
                    .filter(|e| e.error == EngineError::DownloadConflict)
                    .filter(|e| {
                        !report
                            .reconciliation
                            .iter()
                            .any(|r| r.relative_path == e.relative_path)
                    })
                    .map(|e| e.relative_path.clone()),
            )
            .collect();
        let conflict_count = pending
            .iter()
            .filter(|e| conflicts.contains(&e.relative_path))
            .count();
        Self {
            conflicts: conflict_count,
            problems: pending.len() - conflict_count,
        }
    }
    pub fn any(self) -> bool {
        self.conflicts + self.problems > 0
    }
}

/// Describes the latest result in the shared feedback card after processing stops.
pub(crate) fn attention_text(report: &DownloadReport) -> String {
    let counts = Attention::from_report(report);
    let conflicts = gettextrs::ngettext(
        "{count} conflict awaiting a decision",
        "{count} conflicts awaiting a decision",
        counts.conflicts.try_into().unwrap_or(u32::MAX),
    )
    .replace("{count}", &counts.conflicts.to_string());
    let problems = gettextrs::ngettext(
        "{count} problem to review",
        "{count} problems to review",
        counts.problems.try_into().unwrap_or(u32::MAX),
    )
    .replace("{count}", &counts.problems.to_string());
    match (counts.conflicts, counts.problems) {
        (0, _) => problems,
        (_, 0) => conflicts,
        _ => format!("{conflicts} · {problems}"),
    }
}

/// Completion requires a successful result and finished item progress. A scheduler
/// label alone is not proof of synchronization or permission to hide problems.
pub(crate) fn confirmed(snapshot: &Snapshot) -> bool {
    matches!(&snapshot.result, Some(Ok(report)) if report.attention().is_empty() && report.skipped == 0)
        && snapshot.progress.as_ref().is_none_or(|p| {
            p.preparation.is_none() && p.current.is_none() && p.completed_items >= p.total_items
        })
}

pub(super) fn status(snapshot: &Snapshot) -> &'static str {
    match snapshot.phase {
        Phase::Unconfigured => "Not configured",
        Phase::Waiting => "Waiting",
        Phase::Running if snapshot.pausing => "Pausing",
        Phase::Running => "Syncing",
        Phase::UpToDate if confirmed(snapshot) => "Up to date",
        Phase::UpToDate | Phase::Attention => "Needs attention",
        Phase::Blocked if matches!(&snapshot.result, Some(Err(EngineError::RootSuspended(_)))) => {
            "Folder unavailable"
        }
        Phase::Blocked => "Needs attention",
        Phase::Offline => "Offline",
        Phase::Retrying => "Retrying",
        Phase::Paused => "Paused",
    }
}

/// Describes observed preparation work without inventing a total.
pub(crate) fn preparation_text(
    stage: &drivesync_engine::PreparationProgress,
) -> (&'static str, String) {
    use drivesync_engine::PreparationProgress;
    match stage {
        PreparationProgress::ScanningUploads => (
            "Looking for new local files",
            "Checking which new files and folders can be uploaded…".into(),
        ),
        PreparationProgress::PreparingUpload { .. } => (
            "Preparing a new upload",
            gettextrs::gettext("Reading the file before uploading…"),
        ),
        PreparationProgress::Connecting => (
            "Connecting to Google Drive",
            "Verifying account access and the selected folder…".into(),
        ),
        PreparationProgress::ReadingChanges => (
            "Checking recent Drive changes",
            "Reading changes reported by Google Drive…".into(),
        ),
        PreparationProgress::Scanning { files, folders, .. } => (
            "Analyzing Google Drive",
            format!("{files} files and {folders} folders found so far…"),
        ),
        PreparationProgress::CheckingLocal { checked } => (
            "Checking files on this computer",
            format!("{checked} local paths checked…"),
        ),
        PreparationProgress::SavingState => (
            "Preparing downloads",
            "Saving the analysis before checking and downloading files…".into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{DownloadIssue, ReconciliationEntry};
    #[test]
    fn conflicts_and_errors_are_counted_once_per_observed_path() {
        let report = DownloadReport {
            issues: vec![
                DownloadIssue {
                    relative_path: Some("a".into()),
                    error: EngineError::DownloadConflict,
                },
                DownloadIssue {
                    relative_path: Some("b".into()),
                    error: EngineError::InvalidFolder,
                },
            ],
            reconciliation: vec![ReconciliationEntry {
                relative_path: Some("a".into()),
                remote_id: None,
                kind: ChangeKind::ConcurrentChanges,
            }],
            ..Default::default()
        };
        assert_eq!(
            Attention::from_report(&report),
            Attention {
                conflicts: 1,
                problems: 1
            }
        );
    }
    #[test]
    fn existing_local_data_is_a_problem_even_when_download_reports_conflict() {
        let report = DownloadReport {
            issues: vec![DownloadIssue {
                relative_path: Some("a".into()),
                error: EngineError::DownloadConflict,
            }],
            reconciliation: vec![ReconciliationEntry {
                relative_path: Some("a".into()),
                remote_id: None,
                kind: ChangeKind::ExistingLocalData,
            }],
            ..Default::default()
        };
        assert_eq!(
            Attention::from_report(&report),
            Attention {
                conflicts: 0,
                problems: 1
            }
        );
    }
    #[test]
    fn replacement_decisions_are_conflicts_and_zero_categories_are_omitted() {
        for kind in [
            ChangeKind::LocalIdentityChanged,
            ChangeKind::ConcurrentIdentityChanges,
        ] {
            let report = DownloadReport {
                reconciliation: vec![ReconciliationEntry {
                    relative_path: Some("note.txt".into()),
                    remote_id: Some("file".into()),
                    kind,
                }],
                ..Default::default()
            };
            assert_eq!(
                Attention::from_report(&report),
                Attention {
                    conflicts: 1,
                    problems: 0
                }
            );
            let text = attention_text(&report);
            assert!(!text.contains("0 problems"));
        }
        let report = DownloadReport {
            reconciliation: vec![ReconciliationEntry {
                relative_path: Some("note.txt".into()),
                remote_id: Some("file".into()),
                kind: ChangeKind::Unknown,
            }],
            ..Default::default()
        };
        assert!(!attention_text(&report).contains("0 conflicts"));
    }

    #[test]
    fn a_scheduler_success_does_not_override_pending_results() {
        let mut snapshot = Snapshot {
            revision: 1,
            phase: Phase::UpToDate,
            pausing: false,
            progress: None,
            result: None,
        };
        assert_eq!(status(&snapshot), "Needs attention");
        snapshot.result = Some(Ok(DownloadReport::default()));
        assert_eq!(status(&snapshot), "Up to date");
        snapshot.result = Some(Ok(DownloadReport {
            skipped: 1,
            ..Default::default()
        }));
        assert_eq!(status(&snapshot), "Needs attention");
    }
}
