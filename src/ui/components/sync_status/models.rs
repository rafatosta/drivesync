//! Engine-independent presentation values; idle is never evidence of synchronization.

/// The operation currently executing, without queues, failures or history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncActivity {
    #[default]
    Idle,
    Upload,
    Download,
    Scanning,
    CheckingRemote,
    Renaming,
    Moving,
    Deleting,
    Reconciling,
    Waiting,
    Stopped,
    Failed,
}

/// A snapshot supplied by a presentation adapter on state-change events.
/// `synchronized` must be explicitly confirmed by the producer and invalidated
/// whenever its knowledge becomes stale. It is ignored during activity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncStatusData {
    pub activity: SyncActivity,
    /// Localized description of the current item and phase, when available.
    pub detail: Option<String>,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub transferred_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub synchronized: bool,
    /// Conflicts awaiting a user decision; supplied by the presentation producer.
    pub conflicts: u64,
    /// Problems preventing completion; no technical error details are retained.
    pub problems: u64,
}

impl SyncStatusData {
    /// Whether separate conflict or problem cards should be shown.
    pub fn needs_attention(&self) -> bool {
        self.conflicts > 0 || self.problems > 0
    }

    /// Prefer byte progress, falling back to item progress. Unknown or zero
    /// denominators are indeterminate; over-reported numerators are clamped.
    pub fn fraction(&self) -> Option<f64> {
        if !self.activity.is_active() {
            return None;
        }
        [
            (self.transferred_bytes, self.total_bytes),
            (self.completed, self.total),
        ]
        .into_iter()
        .find_map(|(done, total)| match (done, total) {
            (Some(done), Some(total)) if total > 0 => Some(done.min(total) as f64 / total as f64),
            _ => None,
        })
    }

    /// Aggregate one current snapshot per configured account. The first active
    /// account in the caller's stable ordering supplies the predominant activity.
    /// Attention counts are summed separately, so they never replace that activity.
    /// No accounts, unknown accounts or active accounts can imply global success.
    /// Progress belongs only to the selected activity, never unrelated totals.
    pub fn aggregate(accounts: &[Self]) -> Self {
        let conflicts = accounts
            .iter()
            .fold(0_u64, |n, s| n.saturating_add(s.conflicts));
        let problems = accounts
            .iter()
            .fold(0_u64, |n, s| n.saturating_add(s.problems));
        let mut aggregate = accounts
            .iter()
            .find(|s| s.activity.is_active())
            .cloned()
            .or_else(|| {
                accounts
                    .iter()
                    .find(|s| s.activity != SyncActivity::Idle)
                    .cloned()
            })
            .unwrap_or_else(|| Self {
                synchronized: !accounts.is_empty() && accounts.iter().all(|s| s.synchronized),
                ..Self::default()
            });
        aggregate.conflicts = conflicts;
        aggregate.problems = problems;
        if aggregate.needs_attention() {
            aggregate.synchronized = false;
        }
        aggregate
    }
}

impl SyncActivity {
    /// Whether the card should show an active spinner and accept progress.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Upload
                | Self::Download
                | Self::Scanning
                | Self::CheckingRemote
                | Self::Renaming
                | Self::Moving
                | Self::Deleting
                | Self::Reconciling
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attention_is_separate_from_activity_and_sums_across_accounts() {
        let attention = SyncStatusData {
            conflicts: 2,
            problems: 1,
            synchronized: true,
            activity: SyncActivity::Upload,
            completed: Some(1),
            total: Some(2),
            ..Default::default()
        };
        assert!(attention.needs_attention());
        assert_eq!(attention.fraction(), Some(0.5));
        let merged = SyncStatusData::aggregate(&[
            attention,
            SyncStatusData {
                conflicts: u64::MAX,
                problems: 2,
                ..Default::default()
            },
        ]);
        assert_eq!(merged.conflicts, u64::MAX);
        assert_eq!(merged.problems, 3);
        assert_eq!(merged.activity, SyncActivity::Upload);
        assert_eq!(merged.fraction(), Some(0.5));
        assert!(!merged.synchronized);
    }

    #[test]
    fn progress_requires_a_valid_pair_and_clamps() {
        let mut s = SyncStatusData {
            activity: SyncActivity::Upload,
            completed: Some(3),
            total: Some(12),
            ..Default::default()
        };
        assert_eq!(s.fraction(), Some(0.25));
        s.transferred_bytes = Some(200);
        s.total_bytes = Some(100);
        assert_eq!(s.fraction(), Some(1.0));
        s.total_bytes = Some(0);
        assert_eq!(s.fraction(), Some(0.25));
        s.total = None;
        assert_eq!(s.fraction(), None);
        s.activity = SyncActivity::Idle;
        s.total_bytes = Some(100);
        assert_eq!(s.fraction(), None);
    }
    #[test]
    fn global_success_requires_every_account_confirmation() {
        let confirmed = SyncStatusData {
            synchronized: true,
            ..Default::default()
        };
        assert!(!SyncStatusData::aggregate(&[]).synchronized);
        assert!(
            !SyncStatusData::aggregate(&[confirmed.clone(), SyncStatusData::default()])
                .synchronized
        );
        assert!(SyncStatusData::aggregate(&[confirmed.clone(), confirmed.clone()]).synchronized);
        let active = SyncStatusData {
            activity: SyncActivity::Scanning,
            ..Default::default()
        };
        assert_eq!(
            SyncStatusData::aggregate(&[confirmed, active.clone()]),
            active
        );
    }
}
