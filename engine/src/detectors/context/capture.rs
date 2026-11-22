//! Scoped current evidence and destination resolution. No synchronization writes occur here.
use super::*;
use crate::{
    detectors::observation::models::{CaptureEntries, ScopeCapture},
    inventory::models::{InventoryScopeQuery, InventorySnapshot},
    models::{FileKind, RemoteId},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Captures {
    local: Option<ScopeCapture>,
    remote: Option<ScopeCapture>,
}
impl Context {
    pub(crate) fn snapshots(&self) -> Result<Vec<InventorySnapshot>, ContextError> {
        let snapshots = self
            .inventory
            .enumerate(InventoryScopeQuery {
                account_id: self.config.account_id.clone(),
                sync_root_id: self.config.sync_root_id.clone(),
            })
            .map_err(|_| ContextError::InventoryUnavailable)?;
        for snapshot in &snapshots {
            if let Some(local) = &snapshot.baseline.local {
                if let Some(path) = &local.path {
                    validate_relative_path(path)?;
                }
                if let Evidence::Known(parent) = &local.parent {
                    validate_relative_path(parent)?;
                }
            }
        }
        Ok(snapshots)
    }

    /// An empty complete round is still evidence; partial rounds replace and invalidate old facts.
    pub(super) fn build_capture(&self, capture: ScopeCapture) -> Result<Vec<File>, ContextError> {
        let _round = self
            .rounds
            .lock()
            .map_err(|_| ContextError::InvalidCapture)?;
        if capture.account_id != self.config.account_id
            || capture.provider_id != self.config.provider_id
            || capture.sync_root_id != self.config.sync_root_id
            || capture.local_root != self.config.local_root.0
            || capture.remote_root_id != self.config.remote_root_id
        {
            return Err(ContextError::ScopeMismatch);
        }
        let mut keys = BTreeSet::new();
        match &capture.entries {
            CaptureEntries::Local(entries) => {
                for entry in entries {
                    self.validate_scope(&entry.sync_root_id)?;
                    validate_local_paths(entry)?;
                    let path = entry.path.as_ref().ok_or(ContextError::InvalidCapture)?;
                    if path.as_os_str().is_empty()
                        || path
                            .components()
                            .any(|c| !matches!(c, Component::Normal(_)))
                        || (capture.coverage == ObservationCoverage::Complete
                            && entry.coverage != ObservationCoverage::Complete)
                        || entry.state != FileState::Present
                        || !keys.insert(path.clone())
                    {
                        return Err(ContextError::InvalidCapture);
                    }
                }
            }
            CaptureEntries::Remote(entries) => {
                for entry in entries {
                    self.validate_remote_scope(entry)?;
                    let id = entry
                        .remote_id
                        .as_ref()
                        .ok_or(ContextError::InvalidCapture)?;
                    if id.0.is_empty()
                        || id == &self.config.remote_root_id
                        || (capture.coverage == ObservationCoverage::Complete
                            && entry.state == FileState::Present
                            && (!matches!(&entry.name, Evidence::Known(name) if valid_name(name))
                                || !matches!(&entry.parents, Evidence::Known(parents) if parents.len() == 1)
                                || entry.trashed != Evidence::Known(false)))
                        || !keys.insert(Path::from(&id.0))
                    {
                        return Err(ContextError::InvalidCapture);
                    }
                }
            }
        }
        let snapshots = self.snapshots()?;
        let mut captures = self
            .captures
            .lock()
            .map_err(|_| ContextError::InvalidCapture)?;
        let previous = match &capture.entries {
            CaptureEntries::Local(_) => &mut captures.local,
            CaptureEntries::Remote(_) => &mut captures.remote,
        };
        if previous
            .as_ref()
            .is_some_and(|old| old.generation >= capture.generation)
        {
            return Err(ContextError::InvalidCapture);
        }
        *previous = Some(capture);
        // Per-item cache entries from a previous generation must not survive capture invalidation.
        *self
            .cache
            .lock()
            .map_err(|_| ContextError::InvalidCapture)? = cache::Cache::default();
        let observations = captures.observations();
        let view = View::new(&captures, &snapshots, &self.config.remote_root_id);
        let mut files = observations
            .into_iter()
            .map(|observation| {
                let mut file = self.build_item(observation)?;
                self.enrich_view(&mut file, &view);
                Ok(file)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut identities = BTreeSet::new();
        files.retain(|file| {
            file.identity
                .item_id
                .as_ref()
                .is_none_or(|id| identities.insert(id.clone()))
        });
        // Reopened Inventory records are revisited even when neither capture lists them.
        for snapshot in &snapshots {
            if snapshot
                .identity
                .item_id
                .as_ref()
                .is_some_and(|id| identities.contains(id))
            {
                continue;
            }
            let mut file = File {
                identity: snapshot.identity.clone(),
                local: unknown_local_state(),
                remote: unknown_remote_state(),
                baseline: snapshot.baseline.clone(),
                pending_operations: snapshot.pending_operations.clone(),
                prior_conflict: snapshot.prior_conflict.clone(),
                inventory_revision: Some(snapshot.revision),
                inventory_lookup: InventoryLookupStatus::Found,
                environment: self.config.environment(),
                initial_association_authorized: self.config.initial_association_authorized,
                association_candidates: Vec::new(),
                observations: Observations {
                    events: Vec::new(),
                    latest_origin: ObservationOrigin::Remote,
                    local_coverage: ObservationCoverage::Unavailable,
                    remote_coverage: ObservationCoverage::Unavailable,
                    local_generation: None,
                    remote_page_token: None,
                    observed_at: crate::detectors::observation::scanner::now(),
                },
            };
            self.enrich_view(&mut file, &view);
            files.push(file);
        }
        Ok(files)
    }

    pub(super) fn enrich(&self, file: &mut File) -> Result<(), ContextError> {
        let captures = self
            .captures
            .lock()
            .map_err(|_| ContextError::InvalidCapture)?;
        if captures.local.is_none() && captures.remote.is_none() {
            return Ok(());
        }
        let snapshots = self.snapshots()?;
        let view = View::new(&captures, &snapshots, &self.config.remote_root_id);
        self.enrich_view(file, &view);
        Ok(())
    }

    fn enrich_view(&self, file: &mut File, view: &View<'_>) {
        let captures = view.captures;
        let local = &view.local;
        let remote = &view.remote;
        let local_complete = view.local_complete;
        let remote_complete = view.remote_complete;
        if let Some(capture) = &captures.local {
            file.observations.observed_at = capture.observed_at;
            file.observations.local_generation = Some(capture.generation);
            file.observations.local_coverage = capture.coverage;
            file.environment.local_root_available =
                capture.coverage != ObservationCoverage::Unavailable;
        }
        if let Some(capture) = &captures.remote {
            file.observations.remote_coverage = capture.coverage;
            file.environment.remote_available =
                capture.coverage != ObservationCoverage::Unavailable;
        }
        if file.identity.item_id.is_some() {
            if let Some(capture) = &captures.remote {
                file.remote = file
                    .identity
                    .remote_id
                    .as_ref()
                    .and_then(|id| remote.get(id))
                    .map(|entry| remote_state((*entry).clone()))
                    .unwrap_or_else(unknown_remote_state);
                if capture.coverage == ObservationCoverage::Unavailable {
                    file.remote = unknown_remote_state();
                    file.remote.state = FileState::Unavailable;
                }
            }
            if let Some(capture) = &captures.local {
                let path = file
                    .local
                    .path
                    .as_ref()
                    .or_else(|| file.baseline.local.as_ref().and_then(|s| s.path.as_ref()))
                    .cloned();
                file.local = unknown_local_state();
                if let Some(path) = path {
                    if let Some(entry) = local.get(&path) {
                        // A reused path is not sufficient to attach a replacement inode to this item.
                        if matches!((&entry.identity, &file.identity.local_identity), (Evidence::Known(a), Evidence::Known(b)) if a == b)
                        {
                            file.local = local_state((*entry).clone());
                        }
                    } else if local_complete
                        && !identity_key(&file.identity.local_identity)
                            .is_some_and(|key| view.local_identities.contains(&key))
                    {
                        file.local.state = FileState::Absent;
                        file.local.path = Some(path);
                    }
                }
                if capture.coverage == ObservationCoverage::Unavailable {
                    file.local.state = FileState::Unavailable;
                }
            }
        }
        let associated = |id: &RemoteId| view.associated.get(id).cloned();
        // Destination evidence is rebuilt from this pair of rounds, never carried across failure.
        file.environment.destination_local_parent = Evidence::NotCollected;
        file.environment.destination_remote_parent_id = Evidence::NotCollected;
        file.environment.destination_remote_matches = Evidence::NotCollected;
        file.environment.destination_local_occupied = Evidence::NotCollected;
        if let (Evidence::Known(parents), Evidence::Known(name)) =
            (&file.remote.parents, &file.remote.name)
        {
            if parents.len() == 1 && valid_name(name) {
                let collisions = view
                    .matches
                    .get(&(parents[0].clone(), name.clone()))
                    .cloned()
                    .unwrap_or_default();
                if remote_complete {
                    file.environment.destination_remote_matches =
                        Evidence::Known(collisions.clone());
                }
                if let Some(parent) =
                    associated(&parents[0]).filter(|_| remote_complete && collisions.len() == 1)
                {
                    let destination = parent.join(name);
                    file.environment.destination_local_parent = Evidence::Known(parent);
                    if local_complete {
                        file.environment.destination_local_occupied =
                            Evidence::Known(local.contains_key(&destination));
                        if file.identity.item_id.is_none() && !local.contains_key(&destination) {
                            file.local.state = FileState::Absent;
                            file.local.path = Some(destination);
                        }
                    }
                }
            }
        }
        if let (Evidence::Known(parent), Evidence::Known(name)) =
            (&file.local.parent, &file.local.name)
        {
            let destination = if parent.as_os_str().is_empty() {
                Some(self.config.remote_root_id.clone())
            } else {
                let ids = view.local_parents.get(parent).cloned().unwrap_or_default();
                if ids.len() == 1 {
                    ids.into_iter().next()
                } else {
                    None
                }
            };
            if let Some(parent_id) = destination {
                file.environment.destination_remote_parent_id = Evidence::Known(parent_id.clone());
                if remote_complete {
                    let matches = view
                        .matches
                        .get(&(parent_id.clone(), name.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if matches.is_empty() && file.identity.item_id.is_none() {
                        file.remote.state = FileState::Absent;
                    }
                    file.environment.destination_remote_matches = Evidence::Known(matches);
                }
            }
        }
        // Project names through the captured tree only to gather candidates. Duplicate projections
        // remain multiple candidates, without merging sides or granting adoption permission.
        file.association_candidates.clear();
        let projected = file
            .remote
            .id
            .as_ref()
            .and_then(|id| view.projected.get(id));
        let paths: BTreeSet<_> = file.local.path.iter().chain(projected).collect();
        for path in paths {
            if let (Some(local_entry), Some(entries)) = (local.get(path), view.candidates.get(path))
            {
                file.association_candidates
                    .extend(entries.iter().map(|entry| models::AssociationCandidate {
                        local: local_state((*local_entry).clone()),
                        remote: remote_state((*entry).clone()),
                    }));
            }
        }
    }
}
type IdentityKey = (u64, u64, Option<u64>);
fn identity_key(identity: &Evidence<crate::models::LocalIdentity>) -> Option<IdentityKey> {
    match identity {
        Evidence::Known(id) => Some((id.device_id, id.inode, id.mount_id)),
        _ => None,
    }
}
/// Derived once per round. These indexes accelerate reads without introducing identity claims.
struct View<'a> {
    captures: &'a Captures,
    local: BTreeMap<Path, &'a LocalObservation>,
    remote: BTreeMap<RemoteId, &'a RemoteObservation>,
    associated: BTreeMap<RemoteId, Path>,
    local_parents: BTreeMap<Path, Vec<RemoteId>>,
    matches: BTreeMap<(RemoteId, String), Vec<RemoteId>>,
    projected: BTreeMap<RemoteId, Path>,
    candidates: BTreeMap<Path, Vec<&'a RemoteObservation>>,
    local_identities: BTreeSet<IdentityKey>,
    local_complete: bool,
    remote_complete: bool,
}
impl<'a> View<'a> {
    fn new(captures: &'a Captures, snapshots: &[InventorySnapshot], root: &RemoteId) -> Self {
        let local = captures.locals();
        let remote = captures.remotes();
        let local_complete = captures
            .local
            .as_ref()
            .is_some_and(|c| c.coverage == ObservationCoverage::Complete);
        let remote_complete = captures
            .remote
            .as_ref()
            .is_some_and(|c| c.coverage == ObservationCoverage::Complete)
            && remote
                .values()
                .all(|entry| !matches!(entry.state, FileState::Unknown | FileState::Unavailable))
            && snapshots
                .iter()
                .filter_map(|snapshot| snapshot.identity.remote_id.as_ref())
                .all(|id| remote.contains_key(id));
        let local_identities = local
            .values()
            .filter_map(|entry| identity_key(&entry.identity))
            .collect();
        let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for snapshot in snapshots {
            if let Some(id) = &snapshot.identity.remote_id {
                grouped.entry(id).or_default().push(snapshot);
            }
        }
        let mut associated = BTreeMap::from([(root.clone(), Path::new())]);
        let mut local_parents: BTreeMap<Path, Vec<RemoteId>> = BTreeMap::new();
        for (id, records) in grouped {
            if records.len() != 1 {
                continue;
            }
            let snapshot = records[0];
            let Some(path) = snapshot
                .baseline
                .local
                .as_ref()
                .and_then(|s| s.path.as_ref())
            else {
                continue;
            };
            let (Some(l), Some(r)) = (local.get(path), remote.get(id)) else {
                continue;
            };
            if l.kind == Evidence::Known(FileKind::Directory)
                && matches!((&l.identity,&snapshot.identity.local_identity),(Evidence::Known(a),Evidence::Known(b)) if a == b)
                && r.state == FileState::Present
                && r.trashed == Evidence::Known(false)
                && r.mime_type == Evidence::Known("application/vnd.google-apps.folder".into())
            {
                associated.insert(id.clone(), path.clone());
                local_parents
                    .entry(path.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
        let mut matches: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut projected = BTreeMap::new();
        let mut candidates: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (id, entry) in &remote {
            if entry.state != FileState::Present || entry.trashed != Evidence::Known(false) {
                continue;
            }
            if let (Evidence::Known(parents), Evidence::Known(name)) = (&entry.parents, &entry.name)
            {
                if parents.len() == 1 {
                    matches
                        .entry((parents[0].clone(), name.clone()))
                        .or_default()
                        .push(id.clone());
                }
            }
            if let Some(path) = projected_path(entry, &remote, root, &mut BTreeSet::new()) {
                projected.insert(id.clone(), path.clone());
                candidates.entry(path).or_default().push(*entry);
            }
        }
        Self {
            captures,
            local,
            remote,
            associated,
            local_parents,
            matches,
            projected,
            candidates,
            local_identities,
            local_complete,
            remote_complete,
        }
    }
}
impl Captures {
    fn locals(&self) -> BTreeMap<Path, &LocalObservation> {
        match self.local.as_ref().map(|c| &c.entries) {
            Some(CaptureEntries::Local(entries)) => entries
                .iter()
                .filter_map(|e| e.path.clone().map(|p| (p, e)))
                .collect(),
            _ => BTreeMap::new(),
        }
    }
    fn remotes(&self) -> BTreeMap<RemoteId, &RemoteObservation> {
        match self.remote.as_ref().map(|c| &c.entries) {
            Some(CaptureEntries::Remote(entries)) => entries
                .iter()
                .filter_map(|e| e.remote_id.clone().map(|id| (id, e)))
                .collect(),
            _ => BTreeMap::new(),
        }
    }
    fn observations(&self) -> Vec<Observation> {
        let mut result = Vec::new();
        if let Some(ScopeCapture {
            entries: CaptureEntries::Local(entries),
            ..
        }) = &self.local
        {
            result.extend(entries.iter().cloned().map(|data| {
                Observation::Local {
                    source: if data
                        .event
                        .as_ref()
                        .is_some_and(|e| e.origin == ObservationOrigin::Local)
                    {
                        LocalObservationSource::Watcher
                    } else {
                        LocalObservationSource::Scanner
                    },
                    data,
                }
            }));
        }
        if let Some(ScopeCapture {
            entries: CaptureEntries::Remote(entries),
            ..
        }) = &self.remote
        {
            result.extend(entries.iter().cloned().map(Observation::Remote));
        }
        result
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\0')
        && name != ".drivesync"
        && !name.starts_with(".drivesync-transfer-")
}
fn projected_path(
    entry: &RemoteObservation,
    entries: &BTreeMap<RemoteId, &RemoteObservation>,
    root: &RemoteId,
    seen: &mut BTreeSet<RemoteId>,
) -> Option<Path> {
    let id = entry.remote_id.as_ref()?;
    if seen.len() >= 128 || !seen.insert(id.clone()) {
        return None;
    }
    let Evidence::Known(name) = &entry.name else {
        return None;
    };
    let Evidence::Known(parents) = &entry.parents else {
        return None;
    };
    if !valid_name(name) || parents.len() != 1 {
        return None;
    }
    if &parents[0] == root {
        return Some(Path::from(name));
    }
    let parent = entries.get(&parents[0])?;
    if parent.state != FileState::Present
        || parent.mime_type != Evidence::Known("application/vnd.google-apps.folder".into())
    {
        return None;
    }
    Some(projected_path(parent, entries, root, seen)?.join(name))
}
