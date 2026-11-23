//! Joins only current observations already correlated to the same persisted ItemId.
use super::*;
use crate::models::ItemId;
use std::collections::BTreeMap;
pub(super) struct Cache {
    limit: usize,
    items: BTreeMap<ItemId, Pair>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            limit: 100_000,
            items: BTreeMap::new(),
        }
    }
}
#[derive(Default)]
struct Pair {
    local: Option<(LocalState, ObservationCoverage)>,
    remote: Option<(RemoteState, ObservationCoverage)>,
}
impl Cache {
    pub(super) fn consolidate(&mut self, file: &mut File) {
        let Some(id) = &file.identity.item_id else {
            return;
        };
        if file.inventory_lookup != InventoryLookupStatus::Found {
            return;
        }
        if !self.items.contains_key(id) && self.items.len() >= self.limit {
            // Eviction invalidates cached counterparts; complete scope rounds re-enumerate all records.
            self.items.clear();
        }
        let pair = self.items.entry(id.clone()).or_default();
        // Each producer delivers serially. UTC wall-clock corrections are not sequence numbers.
        if file.observations.latest_origin == ObservationOrigin::Remote {
            pair.remote = Some((file.remote.clone(), file.observations.remote_coverage));
        } else {
            pair.local = Some((file.local.clone(), file.observations.local_coverage));
        }
        if let Some((local, coverage)) = &pair.local {
            file.local = local.clone();
            file.observations.local_coverage = *coverage;
        }
        if let Some((remote, coverage)) = &pair.remote {
            if remote.id == file.identity.remote_id {
                file.remote = remote.clone();
                file.observations.remote_coverage = *coverage;
            }
        }
        file.environment.local_root_available =
            matches!(file.local.state, FileState::Present | FileState::Absent);
        file.environment.remote_available = matches!(
            file.remote.state,
            FileState::Present | FileState::Absent | FileState::OutsideScope
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{context, local_observation};
    use super::*;
    #[test]
    fn eviction_requires_fresh_counterparts_and_never_discards_persisted_identity() {
        let (context, directory) = context();
        let mut local = context
            .build(Observation::Local {
                source: LocalObservationSource::Scanner,
                data: local_observation(),
            })
            .unwrap();
        local.inventory_lookup = InventoryLookupStatus::Found;
        local.identity.item_id = Some(ItemId("a".into()));
        local.identity.remote_id = Some(crate::models::RemoteId("remote".into()));
        let mut cache = Cache {
            limit: 1,
            items: BTreeMap::new(),
        };
        cache.consolidate(&mut local);
        let mut other = local.clone();
        other.identity.item_id = Some(ItemId("b".into()));
        cache.consolidate(&mut other);
        let mut remote = local.clone();
        remote.observations.latest_origin = ObservationOrigin::Remote;
        remote.local = unknown_local_state();
        remote.remote.state = FileState::Present;
        remote.remote.id = local.identity.remote_id.clone();
        cache.consolidate(&mut remote);
        assert_eq!(remote.local.state, FileState::Unknown);
        assert_eq!(remote.identity.item_id, local.identity.item_id);
        cache.consolidate(&mut local);
        assert_eq!(local.remote.state, FileState::Present);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
