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
}

/// A snapshot supplied by a presentation adapter on state-change events.
/// `synchronized` must be explicitly confirmed by the producer and invalidated
/// whenever its knowledge becomes stale. It is ignored during activity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncStatusData {
    pub activity: SyncActivity,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub transferred_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub synchronized: bool,
}

impl SyncStatusData {
    /// Prefer byte progress, falling back to item progress. Unknown or zero
    /// denominators are indeterminate; over-reported numerators are clamped.
    pub fn fraction(&self) -> Option<f64> {
        if self.activity == SyncActivity::Idle {
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
    /// No accounts, unknown accounts or active accounts can imply global success.
    /// Progress belongs only to the selected activity, never unrelated totals.
    pub fn aggregate(accounts: &[Self]) -> Self {
        accounts
            .iter()
            .find(|s| s.activity != SyncActivity::Idle)
            .cloned()
            .unwrap_or_else(|| Self {
                synchronized: !accounts.is_empty() && accounts.iter().all(|s| s.synchronized),
                ..Self::default()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
