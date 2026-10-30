//! Pure impact aggregation over confirmed identities, never deletion authority.
use crate::{state::Reconciliation, ChangeKind, EngineError};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{collections::BTreeMap, path::PathBuf};

/// Product policy, separated from aggregation for future configuration.
/// Invalid or disabling values are rejected rather than interpreted as opt-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkPolicy {
    pub absolute: usize,
    pub minimum: usize,
    pub percent: u8,
}
impl Default for BulkPolicy {
    fn default() -> Self {
        Self {
            absolute: 100,
            minimum: 10,
            percent: 20,
        }
    }
}
impl BulkPolicy {
    pub(crate) fn validate(self) -> Result<(), EngineError> {
        if self.absolute == 0
            || self.minimum == 0
            || self.minimum > self.absolute
            || !(1..=100).contains(&self.percent)
        {
            return Err(EngineError::InvalidPlan);
        }
        Ok(())
    }
}

/// One ancestor's unique confirmed population and potentially affected identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImpact {
    /// Empty means the entire selected root; otherwise a component-relative folder.
    pub folder: PathBuf,
    pub affected: usize,
    pub population: usize,
}

/// Non-persisted proposal bound to a complete observation and one client/root scope.
/// Obtain it from `EngineError::BulkReviewRequired`; only an explicit user action
/// may supply it to the next synchronization attempt. It cannot authorize deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkReview {
    fingerprint: String,
    impacts: Vec<BulkImpact>,
}
impl BulkReview {
    pub fn impacts(&self) -> &[BulkImpact] {
        &self.impacts
    }
}
impl std::fmt::Display for BulkReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Synchronization paused for bulk changes. Review the affected folders before continuing. Existing data was preserved.")
    }
}

/// Counts each confirmed identity once per ancestor, including uncertain evidence.
/// New items and proven unchanged/equivalent observations do not inflate impact.
/// Ordered JSON and sorted identity vectors make repeated scans deterministic.
/// No timestamps establish precedence; stamps only invalidate stale proposals.
pub(crate) fn assess(
    client: &str,
    record: &Reconciliation,
    policy: BulkPolicy,
) -> Result<Option<BulkReview>, EngineError> {
    policy.validate()?;
    let mut counts: BTreeMap<PathBuf, (usize, usize)> = BTreeMap::new();
    let mut diagnoses: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for entry in &record.entries {
        if let Some(id) = entry.remote_id.as_deref() {
            diagnoses.entry(id).or_default().push(entry);
        }
    }
    for (id, base) in &record.bulk_population {
        let path = base.location.as_ref().map(|l| l.relative_path.as_path());
        let changed = diagnoses.get(id.as_str()).is_none_or(|entries| {
            entries.iter().any(|e| {
                !matches!(e.kind, ChangeKind::Unchanged | ChangeKind::Equivalent)
                    || path.is_some_and(|p| e.relative_path.as_deref() != Some(p))
            })
        });
        let parent = path
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new(""));
        for folder in parent.ancestors() {
            let count = counts.entry(folder.into()).or_default();
            count.0 += 1;
            count.1 += usize::from(changed);
        }
    }
    let impacts: Vec<_> = counts
        .into_iter()
        .filter_map(|(folder, (population, affected))| {
            (affected >= policy.absolute
                || (affected >= policy.minimum
                    && (affected as u128) * 100
                        >= (population as u128) * u128::from(policy.percent)))
            .then_some(BulkImpact {
                folder,
                affected,
                population,
            })
        })
        .collect();
    if impacts.is_empty() {
        return Ok(None);
    }
    let mut remote = record.remote.clone();
    remote.entries.sort_by(|a, b| a.id.cmp(&b.id));
    let mut plan = record.plan.clone();
    plan.entries.sort_by(|a, b| a.remote_id.cmp(&b.remote_id));
    let mut entries = record.entries.clone();
    entries.sort_by(|a, b| (&a.remote_id, &a.relative_path).cmp(&(&b.remote_id, &b.relative_path)));
    let evidence = serde_json::to_value((
        client,
        policy,
        &record.local,
        remote,
        plan,
        &record.bases,
        &record.bulk_population,
        entries,
    ))
    .map_err(|_| EngineError::InvalidPlan)?;
    let bytes = serde_json::to_vec(&evidence).map_err(|_| EngineError::InvalidPlan)?;
    Ok(Some(BulkReview {
        fingerprint: format!("{:x}", sha2::Sha256::digest(bytes)),
        impacts,
    }))
}
