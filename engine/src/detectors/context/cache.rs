//! Joins only current observations already correlated to the same persisted ItemId.
use super::*;
use crate::models::ItemId;
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct Cache {
    items: BTreeMap<ItemId, Pair>,
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
        if !self.items.contains_key(id) && self.items.len() >= 100_000 {
            self.items.pop_first();
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
            if matches!(local.identity, Evidence::Known(_)) {
                file.identity.local_identity = local.identity.clone();
            }
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
