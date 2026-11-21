//! Continuous local observation by verified periodic snapshots, without filesystem mutation.
use super::{
    remote::interfaces::CancellationToken,
    scanner::{now, ScanError, ScanResult, Scanner},
    LocalObservation, LocalObservationSource, Observation, ObservationCoverage,
};
use crate::models::*;
use std::collections::BTreeMap;
/// Retains only the last complete snapshot. Partial rounds cannot produce removal evidence.
pub(crate) struct LocalObserver {
    scanner: Scanner,
    previous: Option<BTreeMap<Path, LocalObservation>>,
    recheck: bool,
}
impl LocalObserver {
    pub(crate) fn new(scanner: Scanner) -> Self {
        Self {
            scanner,
            previous: None,
            recheck: false,
        }
    }
    /// Invalidates prior evidence on source loss; it never declares those entries absent.
    pub(crate) fn unavailable(&self) -> Vec<Observation> {
        self.previous
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
            .collect()
    }
    /// Produces initial Scanner snapshots, then Watcher changes from complete comparisons.
    pub(crate) fn poll(
        &mut self,
        cancel: &CancellationToken,
    ) -> Result<(Vec<Observation>, ObservationCoverage), ScanError> {
        let scan = match self.scanner.scan(cancel) {
            Ok(scan) => scan,
            Err(error) => {
                self.recheck = true;
                return Err(error);
            }
        };
        let coverage = scan.coverage;
        let observations = if let (Some(previous), true) = (&self.previous, self.recheck) {
            let mut observations: Vec<_> = scan
                .entries
                .values()
                .cloned()
                .map(|data| envelope(data, ObservationKind::RecheckRequired))
                .collect();
            if coverage == ObservationCoverage::Complete {
                observations.extend(diff(previous,&scan).into_iter().filter(|observation|matches!(observation,Observation::Local {data,..} if data.state==FileState::Absent)));
            }
            observations
        } else if let Some(previous) = &self.previous {
            if coverage == ObservationCoverage::Complete {
                diff(previous, &scan)
            } else {
                scan.observations(LocalObservationSource::Watcher)
            }
        } else {
            scan.observations(LocalObservationSource::Scanner)
        };
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
fn diff(previous: &BTreeMap<Path, LocalObservation>, scan: &ScanResult) -> Vec<Observation> {
    let mut result = Vec::new();
    for (path, current) in &scan.entries {
        let kind = match previous.get(path) {
            None => ObservationKind::Created,
            Some(old)
                if old.checksum != current.checksum
                    || old.size != current.size
                    || old.kind != current.kind =>
            {
                ObservationKind::Modified
            }
            Some(old)
                if old.modified_at != current.modified_at
                    || old.changed_at != current.changed_at
                    || old.identity != current.identity
                    || old.capabilities != current.capabilities =>
            {
                ObservationKind::MetadataChanged
            }
            _ => continue,
        };
        result.push(envelope(current.clone(), kind));
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
    }
    // Polling has no kernel move cookie: never assert a rename merely from recycled inode values.
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
