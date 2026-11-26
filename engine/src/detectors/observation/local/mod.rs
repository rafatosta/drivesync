//! Continuous local observation by verified event-driven reads and periodic audits.
use super::{
    remote::interfaces::CancellationToken,
    scanner::{now, ScanError, ScanIssueKind, ScanResult, Scanner},
    LocalObservation, LocalObservationSource, Observation, ObservationCoverage,
};
use crate::{
    detectors::activity::{ActivityGuard, Reason},
    models::*,
};
use activity::{Batch, Step};
mod activity;
use std::collections::BTreeMap;
/// Retains only the last complete snapshot. Partial rounds cannot produce removal evidence.
pub(crate) struct LocalObserver {
    scanner: Scanner,
    previous: Option<BTreeMap<Path, LocalObservation>>,
    recheck: bool,
    latest: Vec<LocalObservation>,
}
impl LocalObserver {
    pub(crate) fn new(scanner: Scanner) -> Self {
        Self {
            scanner,
            previous: None,
            recheck: false,
            latest: Vec::new(),
        }
    }
    pub(crate) fn event_fd(&self) -> Option<std::fs::File> {
        self.scanner.event_fd()
    }

    /// Full latest read, including unchanged entries; coverage is supplied by poll.
    pub(crate) fn snapshot(&self, activity: &ActivityGuard) -> Vec<LocalObservation> {
        let mut batch = Batch::new(Some(activity), Step::Materialize, self.latest.len());
        let result = self
            .latest
            .iter()
            .cloned()
            .inspect(|_| batch.advance())
            .collect();
        batch.finish();
        result
    }
    /// Invalidates prior evidence on source loss; it never declares those entries absent.
    pub(crate) fn unavailable(&self, activity: Option<&ActivityGuard>) -> Vec<Observation> {
        let total = self.previous.as_ref().map_or(0, BTreeMap::len);
        let mut batch = Batch::new(activity, Step::Recover(Reason::CoverageLost), total);
        let result = self
            .previous
            .iter()
            .flat_map(|entries| entries.values())
            .cloned()
            .map(|mut data| {
                data.state = FileState::Unavailable;
                data.coverage = ObservationCoverage::Unavailable;
                data.observed_at = now();
                data.checksum = Evidence::Unavailable;
                data.size = Evidence::Unavailable;
                data.capabilities = Evidence::Unavailable;
                envelope(data, ObservationKind::RecheckRequired)
            })
            .inspect(|_| batch.advance())
            .collect();
        batch.finish();
        result
    }
    /// Produces initial Scanner snapshots, then Watcher changes from complete comparisons.
    pub(crate) fn poll(
        &mut self,
        cancel: &CancellationToken,
        activity: Option<&ActivityGuard>,
    ) -> Result<(Vec<Observation>, ObservationCoverage), ScanError> {
        self.scanner.set_progress(activity.map(activity::scanner));
        let prior = self.previous.as_ref().filter(|_| !self.recheck);
        let scan = self.scanner.poll(prior, cancel);
        self.scanner.set_progress(None);
        let scan = match scan {
            Ok(scan) => scan,
            Err(error) => {
                self.recheck = true;
                return Err(error);
            }
        };
        let budget_reached = scan
            .issues
            .iter()
            .any(|issue| issue.kind == ScanIssueKind::LimitReached);
        if budget_reached {
            self.scanner.expand_limit();
        }
        let coverage = scan.coverage;
        let mut batch = Batch::new(activity, Step::Materialize, scan.entries.len());
        self.latest = scan
            .entries
            .values()
            .cloned()
            .inspect(|_| batch.advance())
            .collect();
        batch.finish();
        let incomplete = if budget_reached {
            Reason::BudgetReached
        } else {
            Reason::IncompleteObservation
        };
        let observations = if let (Some(previous), true) = (&self.previous, self.recheck) {
            let mut batch = Batch::new(activity, Step::Recover(incomplete), scan.entries.len());
            let mut observations: Vec<_> = scan
                .entries
                .values()
                .cloned()
                .map(|data| envelope(data, ObservationKind::RecheckRequired))
                .inspect(|_| batch.advance())
                .collect();
            batch.finish();
            if coverage == ObservationCoverage::Complete {
                observations.extend(diff(previous, &scan, activity).into_iter().filter(|observation|matches!(observation,Observation::Local {data,..} if data.state==FileState::Absent)));
            }
            observations
        } else if let Some(previous) = &self.previous {
            if coverage == ObservationCoverage::Complete {
                diff(previous, &scan, activity)
            } else {
                observations(
                    &scan,
                    LocalObservationSource::Watcher,
                    activity,
                    Step::Recover(incomplete),
                )
            }
        } else {
            let step = if coverage == ObservationCoverage::Complete {
                Step::Materialize
            } else {
                Step::Recover(incomplete)
            };
            observations(&scan, LocalObservationSource::Scanner, activity, step)
        };
        let mut batch = Batch::new(
            activity,
            Step::Materialize,
            observations.len() + self.latest.len(),
        );
        let events: BTreeMap<_, _> = observations
            .iter()
            .filter_map(|observation| {
                batch.advance();
                match observation {
                    Observation::Local { data, .. } => {
                        data.path.clone().map(|path| (path, data.event.clone()))
                    }
                    _ => None,
                }
            })
            .collect();
        for entry in &mut self.latest {
            if let Some(event) = entry.path.as_ref().and_then(|path| events.get(path)) {
                entry.event = event.clone();
            } else if self.previous.is_some() {
                if let Some(event) = &mut entry.event {
                    event.origin = ObservationOrigin::Local;
                }
            }
            batch.advance();
        }
        batch.finish();
        if coverage == ObservationCoverage::Complete {
            self.previous = Some(scan.entries);
            self.recheck = false;
        }
        if coverage != ObservationCoverage::Complete {
            self.recheck = true;
        }
        Ok((observations, coverage))
    }
}
fn diff(
    previous: &BTreeMap<Path, LocalObservation>,
    scan: &ScanResult,
    activity: Option<&ActivityGuard>,
) -> Vec<Observation> {
    let mut batch = Batch::new(activity, Step::Compare, previous.len() + scan.entries.len());
    let mut result = Vec::new();
    for (path, current) in &scan.entries {
        let kind = match previous.get(path) {
            None => Some(ObservationKind::Created),
            Some(old)
                if old.checksum != current.checksum
                    || old.size != current.size
                    || old.kind != current.kind =>
            {
                Some(ObservationKind::Modified)
            }
            Some(old)
                if old.modified_at != current.modified_at
                    || old.changed_at != current.changed_at
                    || old.identity != current.identity
                    || old.capabilities != current.capabilities =>
            {
                Some(ObservationKind::MetadataChanged)
            }
            _ => None,
        };
        if let Some(kind) = kind {
            result.push(envelope(current.clone(), kind));
        }
        batch.advance();
    }
    for (path, old) in previous {
        if !scan.entries.contains_key(path) {
            let mut missing = old.clone();
            missing.state = FileState::Absent;
            missing.observed_at = now();
            missing.checksum = Evidence::NotCollected;
            missing.size = Evidence::NotCollected;
            result.push(envelope(missing, ObservationKind::Removed));
        }
        batch.advance();
    }
    batch.finish();
    // Polling has no kernel move cookie: never assert a rename merely from recycled inode values.
    result
}
fn observations(
    scan: &ScanResult,
    source: LocalObservationSource,
    activity: Option<&ActivityGuard>,
    step: Step,
) -> Vec<Observation> {
    let mut batch = Batch::new(activity, step, scan.entries.len());
    let result = scan
        .entries
        .values()
        .cloned()
        .map(|mut data| {
            if let Some(event) = &mut data.event {
                event.origin = if source == LocalObservationSource::Scanner {
                    ObservationOrigin::Scanner
                } else {
                    ObservationOrigin::Local
                };
            }
            Observation::Local { source, data }
        })
        .inspect(|_| batch.advance())
        .collect();
    batch.finish();
    result
}

fn envelope(mut data: LocalObservation, kind: ObservationKind) -> Observation {
    data.event = Some(ObservationEvent {
        origin: ObservationOrigin::Local,
        kind,
        observed_at: data.observed_at,
        old_path: if kind == ObservationKind::Removed {
            data.path.clone()
        } else {
            None
        },
        new_path: if kind == ObservationKind::Removed {
            None
        } else {
            data.path.clone()
        },
        remote_id: None,
        move_cookie: None,
    });
    Observation::Local {
        source: LocalObservationSource::Watcher,
        data,
    }
}
#[cfg(test)]
mod tests;
