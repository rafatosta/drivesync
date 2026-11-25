//! Scope invalidation derived from verified Operator receipts.
use crate::{
    detectors::context::File,
    inventory::models::AppliedEffect,
    models::{Evidence, ItemId, LocalIdentity, Path, RemoteId},
};

#[derive(Default)]
pub(super) struct Refresh {
    floor: u64,
    affected: Vec<(u64, Dependencies)>,
}

#[derive(Default)]
pub(super) struct Dependencies {
    item: Option<ItemId>,
    local_identity: Option<LocalIdentity>,
    local_paths: Vec<Path>,
    remote_id: Option<RemoteId>,
    remote_slots: Vec<(RemoteId, String)>,
    remote_parents: Vec<RemoteId>,
}

fn known_identity(evidence: &Evidence<LocalIdentity>) -> Option<LocalIdentity> {
    match evidence {
        Evidence::Known(identity) => Some(identity.clone()),
        _ => None,
    }
}

impl Dependencies {
    fn for_file(file: &File) -> Self {
        let mut result = Self {
            item: file.identity.item_id.clone(),
            local_identity: known_identity(&file.identity.local_identity),
            remote_id: file
                .identity
                .remote_id
                .clone()
                .or_else(|| file.remote.id.clone()),
            ..Self::default()
        };
        result.local_paths.extend(file.local.path.clone());
        if let Some(state) = &file.baseline.local {
            result.local_paths.extend(state.path.clone());
        }
        if let (Evidence::Known(parent), Evidence::Known(name), Evidence::Known(mime)) = (
            &file.environment.destination_local_parent,
            &file.remote.name,
            &file.remote.mime_type,
        ) {
            if let Some(name) = crate::projections::local_name(name, mime) {
                result.local_paths.push(parent.join(name));
            }
        }
        if let Evidence::Known(parents) = &file.remote.parents {
            result.remote_parents.extend(parents.iter().cloned());
            if let Evidence::Known(name) = &file.remote.name {
                result
                    .remote_slots
                    .extend(parents.iter().map(|parent| (parent.clone(), name.clone())));
            }
        }
        if let Evidence::Known(parent) = &file.environment.destination_remote_parent_id {
            result.remote_parents.push(parent.clone());
            for name in [&file.local.name, &file.remote.name] {
                if let Evidence::Known(name) = name {
                    result.remote_slots.push((parent.clone(), name.clone()));
                }
            }
        }
        result
    }

    pub(super) fn for_effect(item: ItemId, effect: &AppliedEffect) -> Option<Self> {
        let local = effect.local.as_ref()?;
        let remote = effect.remote.as_ref()?;
        let path = local.path.clone()?;
        let id = effect.remote_id.clone().or_else(|| remote.id.clone())?;
        let (Evidence::Known(parents), Evidence::Known(name)) = (&remote.parents, &remote.name)
        else {
            return None;
        };
        if parents.is_empty() || path.as_os_str().is_empty() {
            return None;
        }
        Some(Self {
            item: Some(item),
            local_identity: known_identity(&local.identity),
            local_paths: vec![path],
            remote_id: Some(id),
            remote_slots: parents
                .iter()
                .map(|parent| (parent.clone(), name.clone()))
                .collect(),
            remote_parents: parents.clone(),
        })
    }

    fn intersects(&self, other: &Self) -> bool {
        (self.item.is_some() && self.item == other.item)
            || (self.local_identity.is_some() && self.local_identity == other.local_identity)
            || self.local_paths.iter().any(|a| {
                other
                    .local_paths
                    .iter()
                    .any(|b| a.starts_with(b) || b.starts_with(a))
            })
            || self.remote_id.as_ref().is_some_and(|id| {
                Some(id) == other.remote_id.as_ref() || other.remote_parents.contains(id)
            })
            || other
                .remote_id
                .as_ref()
                .is_some_and(|id| self.remote_parents.contains(id))
            || self
                .remote_slots
                .iter()
                .any(|slot| other.remote_slots.contains(slot))
    }
}

impl Refresh {
    pub(super) fn record(&mut self, epoch: u64, affected: Dependencies) {
        self.affected.push((epoch, affected));
    }

    pub(super) fn advance(&mut self, epoch: u64) {
        self.floor = self.floor.max(epoch);
        self.affected.retain(|(epoch, _)| *epoch > self.floor);
    }

    pub(super) fn pending(&self) -> bool {
        !self.affected.is_empty()
    }

    pub(super) fn current(&mut self, file: &File, complete_epoch: Option<u64>) -> bool {
        if let Some(epoch) = complete_epoch {
            // Once both collections have covered an effect, older queued pairs can no longer
            // supersede that evidence. This also bounds the retained mutation history.
            let covered = self
                .affected
                .iter()
                .rev()
                .find(|(effect, _)| *effect <= epoch);
            if let Some((effect, _)) = covered {
                self.advance(*effect);
            }
        }
        let epoch = complete_epoch.unwrap_or(0);
        if epoch < self.floor {
            return false;
        }
        if self.affected.is_empty() {
            return true;
        }
        let dependencies = Dependencies::for_file(file);
        !self
            .affected
            .iter()
            .any(|(effect, affected)| *effect > epoch && affected.intersects(&dependencies))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::analyzer::tests::fixture;

    #[test]
    fn awaiting_a_confirmed_effect_blocks_its_slot_and_dependencies() {
        let file = fixture();
        let mut refresh = Refresh::default();
        let effect = AppliedEffect {
            local: Some(file.local.clone()),
            remote: Some(file.remote.clone()),
            remote_id: file.remote.id.clone(),
            verified: true,
        };
        refresh.record(
            1,
            Dependencies::for_effect(file.identity.item_id.clone().unwrap(), &effect).unwrap(),
        );
        assert!(!refresh.current(&file, Some(0)));
        assert!(!refresh.current(&file, None));

        let mut other = file.clone();
        other.identity.item_id = None;
        other.identity.local_identity = Evidence::NotCollected;
        other.identity.remote_id = Some(RemoteId("other".into()));
        other.remote.id = other.identity.remote_id.clone();
        other.baseline.local = None;
        other.local.path = Some("folder/file.txt/child".into());
        other.remote.name = Evidence::Known("unrelated-name".into());
        assert!(!refresh.current(&other, Some(0)));

        other.local.path = Some("folder/sibling.txt".into());
        assert!(refresh.current(&other, Some(0)));
        other.remote.name = file.remote.name.clone();
        assert!(!refresh.current(&other, Some(0)));
        assert!(refresh.current(&other, Some(1)));
        assert!(!refresh.current(&file, Some(0)));
    }
}
