//! Scoped current evidence and destination resolution. No synchronization writes occur here.
use super::*;
use crate::SynchronizationProjectionIssue as ProjectionIssue;
use crate::{
    detectors::observation::models::{CaptureEntries, ScopeCapture},
    inventory::models::{InventoryScopeQuery, InventorySnapshot},
    models::{FileKind, RemoteId},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Captures {
    pub(super) local: Option<ScopeCapture>,
    pub(super) remote: Option<ScopeCapture>,
    consolidated: Option<(Vec<InventorySnapshot>, Vec<File>)>,
}
impl Context {
    #[cfg(test)]
    pub(crate) fn snapshots(&self) -> Result<Vec<InventorySnapshot>, ContextError> {
        self.snapshots_with(Trace::default())
    }

    pub(super) fn snapshots_with(
        &self,
        trace: Trace<'_>,
    ) -> Result<Vec<InventorySnapshot>, ContextError> {
        let inventory_context = trace.inventory_context();
        let mut snapshots = trace
            .wait(Resource::Inventory, || {
                self.inventory.enumerate_with_context(
                    InventoryScopeQuery {
                        account_id: self.config.account_id.clone(),
                        sync_root_id: self.config.sync_root_id.clone(),
                    },
                    inventory_context.as_ref(),
                )
            })
            .map_err(|_| ContextError::InventoryUnavailable)?;
        let mut filtering = trace.batch(Work::InventoryRecords, snapshots.len());
        snapshots.retain(|snapshot| {
            let keep = !snapshot
                .local_path
                .as_deref()
                .is_some_and(crate::projections::libreoffice_lock_path);
            filtering.advance();
            keep
        });
        filtering.finish();
        let mut validating = trace.batch(Work::InventoryRecords, snapshots.len());
        for snapshot in &snapshots {
            if let Some(local) = &snapshot.baseline.local {
                if let Some(path) = &local.path {
                    validate_relative_path(path)?;
                }
                if let Evidence::Known(parent) = &local.parent {
                    validate_relative_path(parent)?;
                }
            }
            validating.advance();
        }
        validating.finish();
        Ok(snapshots)
    }

    pub(in crate::detectors) fn known_remote_ids(
        &self,
        activity: &ActivityGuard,
    ) -> Result<Vec<RemoteId>, ContextError> {
        let trace = Trace::new(activity);
        let snapshots = self.snapshots_with(trace)?;
        let mut selecting = trace.batch(Work::InventoryRecords, snapshots.len());
        let ids = snapshots
            .into_iter()
            .inspect(|_| selecting.advance())
            .filter_map(|snapshot| snapshot.identity.remote_id)
            .collect();
        selecting.finish();
        Ok(ids)
    }

    #[cfg(test)]
    pub(super) fn build_capture(&self, capture: ScopeCapture) -> Result<Vec<File>, ContextError> {
        self.build_capture_with(capture, Trace::default())
    }

    /// An empty complete round is still evidence; partial rounds replace and invalidate old facts.
    pub(super) fn build_capture_with(
        &self,
        capture: ScopeCapture,
        trace: Trace<'_>,
    ) -> Result<Vec<File>, ContextError> {
        let _round = trace
            .wait(Resource::Context, || self.rounds.lock())
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
        let mut validating = trace.batch(
            Work::Validate,
            match &capture.entries {
                CaptureEntries::Local(entries) => entries.len(),
                CaptureEntries::Remote(entries) => entries.len(),
            },
        );
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
                    validating.advance();
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
                            && (!matches!(&entry.parents, Evidence::Known(parents) if parents.len() == 1)
                                || entry.trashed != Evidence::Known(false)))
                        || !keys.insert(Path::from(&id.0))
                    {
                        return Err(ContextError::InvalidCapture);
                    }
                    validating.advance();
                }
            }
        }
        validating.finish();
        let snapshots = self.snapshots_with(trace)?;
        let mut captures = trace
            .wait(Resource::Context, || self.captures.lock())
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
        let unchanged = previous
            .as_ref()
            .is_some_and(|previous| same_evidence(previous, &capture, trace));
        *previous = Some(capture);
        if unchanged
            && captures
                .local
                .as_ref()
                .is_some_and(|c| c.coverage == ObservationCoverage::Complete)
            && captures
                .remote
                .as_ref()
                .is_some_and(|c| c.coverage == ObservationCoverage::Complete)
        {
            if let Some((previous, files)) = &captures.consolidated {
                if *previous == snapshots {
                    let _sample = crate::performance::Sample::begin("context_capture_reuse");
                    return Ok(captures.republish(files.clone(), trace));
                }
            }
        }
        // Per-item cache entries from a previous generation must not survive capture invalidation.
        *trace
            .wait(Resource::Context, || self.cache.lock())
            .map_err(|_| ContextError::InvalidCapture)? = cache::Cache::default();
        let observations = captures.observations(trace);
        let view = View::new(&captures, &snapshots, &self.config.remote_root_id, trace);
        let queries = std::cell::Cell::new(0);
        let building = trace.count_lookups(&queries);
        let mut batch = building.batch(Work::Build, observations.len());
        let mut files = observations
            .into_iter()
            .map(|observation| {
                let mut file = self.build_item(observation, building)?;
                self.enrich_view(&mut file, &view);
                batch.advance();
                Ok(file)
            })
            .collect::<Result<Vec<_>, _>>()?;
        batch.finish();
        let mut identities = BTreeSet::new();
        let mut deduplicating = trace.batch(Work::Index, files.len());
        files.retain(|file| {
            let unique = file
                .identity
                .item_id
                .as_ref()
                .is_none_or(|id| identities.insert(id.clone()));
            deduplicating.advance();
            unique
        });
        deduplicating.finish();
        // Reopened Inventory records are revisited even when neither capture lists them.
        let mut revisiting = trace.batch(Work::InventoryRecords, snapshots.len());
        for snapshot in &snapshots {
            revisiting.advance();
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
                registration_id: None,
                local: {
                    let mut local = unknown_local_state();
                    local.path = snapshot.local_path.clone();
                    local
                },
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
                    local_epoch: None,
                    remote_epoch: None,
                    remote_generation: None,
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
        revisiting.finish();
        // A newly relocated parent's durable path must be confirmed before its descendants.
        // Otherwise a child's unmet parent precondition can repeatedly invalidate the whole batch.
        files.sort_by_key(|file| {
            (
                file.local
                    .path
                    .as_ref()
                    .map_or(usize::MAX, |p| p.components().count()),
                file.identity.item_id.is_none(),
            )
        });
        captures.consolidated = (captures.local.is_some() && captures.remote.is_some())
            .then(|| (snapshots, files.clone()));
        Ok(files)
    }

    pub(super) fn enrich(&self, file: &mut File, trace: Trace<'_>) -> Result<(), ContextError> {
        let captures = trace
            .wait(Resource::Context, || self.captures.lock())
            .map_err(|_| ContextError::InvalidCapture)?;
        if captures.local.is_none() && captures.remote.is_none() {
            return Ok(());
        }
        let snapshots = self.snapshots_with(trace)?;
        let view = View::new(&captures, &snapshots, &self.config.remote_root_id, trace);
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
            file.observations.local_epoch = Some(capture.collect_epoch);
            file.observations.local_coverage = capture.coverage;
            file.environment.local_root_available =
                capture.coverage != ObservationCoverage::Unavailable;
        }
        if let Some(capture) = &captures.remote {
            file.observations.remote_coverage = capture.coverage;
            file.observations.remote_generation = Some(capture.generation);
            file.observations.remote_epoch = Some(capture.collect_epoch);
            file.environment.remote_available =
                capture.coverage != ObservationCoverage::Unavailable;
        }
        file.environment.local_replacement = Evidence::NotCollected;
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
                let registered = file.baseline.local.as_ref().and_then(|s| s.path.as_ref());
                let path = file.local.path.as_ref().or(registered).cloned();
                // An incarnation may move only once in a complete scope, with no other owner.
                let moved = if local_complete && remote_complete {
                    view.locate(&file.identity.local_identity).filter(|entry| {
                        entry.path != path
                            && !path.as_ref().is_some_and(|p| local.contains_key(p))
                            && entry
                                .path
                                .as_ref()
                                .is_some_and(|p| !view.owners.contains_key(p))
                    })
                } else {
                    None
                };
                file.local = unknown_local_state();
                if let Some(entry) = moved {
                    file.local = local_state(entry.clone());
                } else if let Some(path) = path {
                    if let Some(entry) = local.get(&path) {
                        // A reused path is not sufficient to attach a replacement inode to this item.
                        if matches!((&entry.identity, &file.identity.local_identity), (Evidence::Known(a), Evidence::Known(b)) if a == b)
                        {
                            file.local = local_state((*entry).clone());
                        } else if local_complete
                            && remote_complete
                            && file.baseline.state
                                == crate::inventory::models::BaselineState::Synced
                            && entry.kind == Evidence::Known(FileKind::Regular)
                            && file.baseline.local.as_ref().is_some_and(|old| {
                                old.kind == entry.kind && old.path.as_ref() == Some(&path)
                            })
                            && file.baseline.remote.as_ref().is_some_and(|old| {
                                old.id == file.identity.remote_id
                                    && (file.remote.unchanged_binary(old)
                                        || (file.remote.state==FileState::Present
                                            && file.baseline.local.as_ref().is_some_and(|old_local|
                                                matches!((&old_local.checksum,&entry.checksum,&old.checksum,&file.remote.checksum),
                                                    (Evidence::Known(a),Evidence::Known(b),Evidence::Known(c),Evidence::Known(d)) if a!=b && c!=d))))
                            })
                            && !identity_key(&file.identity.local_identity)
                                .is_some_and(|key| view.local_identities.contains(&key))
                            && local
                                .values()
                                .filter(|e| e.identity == entry.identity)
                                .count()
                                == 1
                        {
                            // The persisted synchronized slot binds this edit; inode reuse alone never does.
                            file.local = local_state((*entry).clone());
                            file.environment.local_replacement = Evidence::Known(true);
                        }
                    } else if local_complete
                        && !identity_key(&file.identity.local_identity)
                            .is_some_and(|key| view.local_identities.contains(&key))
                    {
                        if let Some(old) = &file.baseline.local {
                            file.local = old.clone();
                        }
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
                if file.remote.state == FileState::Absent
                    && file.remote.trashed == Evidence::Known(true)
                {
                    // An explicit trash probe establishes this item's parent independently of
                    // unrelated inaccessible IDs. It never establishes a free destination.
                    if let Some(parent) = associated(&parents[0]).filter(|_| {
                        captures
                            .remote
                            .as_ref()
                            .is_some_and(|c| c.coverage == ObservationCoverage::Complete)
                    }) {
                        file.environment.destination_local_parent = Evidence::Known(parent);
                    }
                }
                let projected_name = match &file.remote.mime_type {
                    Evidence::Known(mime) => crate::projections::local_name(name, mime),
                    _ => None,
                };
                let Some(projected_name) = projected_name else {
                    file.environment.projection_issue = Some(ProjectionIssue::InvalidRemoteName);
                    return;
                };
                let collisions = view
                    .matches
                    .get(&(parents[0].clone(), name.clone()))
                    .cloned()
                    .unwrap_or_default();
                if remote_complete {
                    file.environment.destination_remote_matches =
                        Evidence::Known(collisions.clone());
                }
                if let Some(parent) = associated(&parents[0]).filter(|_| {
                    remote_complete
                        && collisions.len() == 1
                        && view
                            .projected_matches
                            .get(&(parents[0].clone(), projected_name.clone()))
                            .is_some_and(|ids| ids.len() == 1)
                }) {
                    let destination = parent.join(&projected_name);
                    file.environment.destination_local_parent = Evidence::Known(parent);
                    if local_complete {
                        file.environment.destination_local_occupied =
                            Evidence::Known(local.contains_key(&destination));
                        if !local.contains_key(&destination)
                            && (file.identity.item_id.is_none()
                                || (file.baseline.state
                                    == crate::inventory::models::BaselineState::NeverSynced
                                    && !matches!(file.identity.local_identity, Evidence::Known(_))))
                        {
                            file.local.state = FileState::Absent;
                            file.local.path = Some(destination.clone());
                            file.local.name = Evidence::Known(projected_name.clone());
                            file.local.parent =
                                Evidence::Known(destination.parent().unwrap().to_path_buf());
                            file.environment.destination_remote_parent_id =
                                Evidence::Known(parents[0].clone());
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
                    // A derived local filename is not the provider's document name.
                    let destination_name = if crate::projections::workspace(&file.remote) {
                        match &file.remote.name {
                            Evidence::Known(remote_name) => remote_name,
                            _ => name,
                        }
                    } else {
                        name
                    };
                    let matches = view
                        .matches
                        .get(&(parent_id.clone(), destination_name.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if matches.is_empty()
                        && file.identity.remote_id.is_none()
                        && file.baseline.state
                            == crate::inventory::models::BaselineState::NeverSynced
                    {
                        file.remote.state = FileState::Absent;
                        file.remote.name = Evidence::Known(name.clone());
                        file.remote.parents = Evidence::Known(vec![parent_id.clone()]);
                        file.environment.destination_local_parent = Evidence::Known(parent.clone());
                    }
                    file.environment.destination_remote_matches = Evidence::Known(matches);
                }
            }
        }
        file.environment.trash_tree_safe = if file.local.kind
            != Evidence::Known(FileKind::Directory)
        {
            Evidence::NotApplicable
        } else if local_complete && remote_complete {
            let path = file.local.path.as_ref();
            let id = file.identity.remote_id.as_ref();
            Evidence::Known(!view.local.keys().any(|p| path.is_some_and(|root| p != root && p.starts_with(root)))
                && !view.remote.values().any(|r| matches!(&r.parents, Evidence::Known(parents) if id.is_some_and(|id| parents.contains(id))))
                && !view.snapshots.iter().any(|s| s.identity.item_id != file.identity.item_id && (
                    s.local_path.as_ref().is_some_and(|p| path.is_some_and(|root| p != root && p.starts_with(root)))
                    || s.baseline.remote.as_ref().is_some_and(|r| matches!(&r.parents, Evidence::Known(parents) if id.is_some_and(|id| parents.contains(id)))))))
        } else {
            Evidence::NotCollected
        };
        self.projection_policy(file, view);
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
                    .extend(entries.iter().map(|entry| AssociationCandidate {
                        local: local_state((*local_entry).clone()),
                        remote: remote_state((*entry).clone()),
                        parents_correspond: match (&local_entry.parent, &entry.parents) {
                            (Evidence::Known(local_parent), Evidence::Known(remote_parents))
                                if remote_parents.len() == 1
                                    && view.associated.get(&remote_parents[0])
                                        == Some(local_parent) =>
                            {
                                Evidence::Known(true)
                            }
                            _ => Evidence::NotCollected,
                        },
                    }));
            }
        }
    }
    fn projection_policy(&self, file: &mut File, view: &View<'_>) {
        use crate::projections::{self, RemoteKind};
        file.environment.projection_issue = None;
        let reason = if file.local.state == FileState::Present
            && !matches!(
                file.local.kind,
                Evidence::Known(FileKind::Regular | FileKind::Directory)
            ) {
            Some(ProjectionIssue::SpecialLocalFile)
        } else if file.local.state == FileState::Present
            && !matches!(&file.local.name,Evidence::Known(n) if projections::valid_name(n))
        {
            Some(ProjectionIssue::InvalidLocalName)
        } else if let (Evidence::Known(name), Evidence::Known(mime)) =
            (&file.remote.name, &file.remote.mime_type)
        {
            let projected = projections::local_name(name, mime);
            if projected.is_none() {
                Some(ProjectionIssue::InvalidRemoteName)
            } else if projections::kind(mime) == RemoteKind::RemoteShortcut {
                Some(ProjectionIssue::DriveShortcut)
            } else if projections::kind(mime) == RemoteKind::Unsupported {
                Some(ProjectionIssue::UnsupportedDriveType)
            } else if let Evidence::Known(parents) = &file.remote.parents {
                if parents.len() == 1
                    && (view
                        .matches
                        .get(&(parents[0].clone(), name.clone()))
                        .is_some_and(|ids| ids.len() > 1)
                        || projected.as_ref().is_some_and(|projected| {
                            view.projected_matches
                                .get(&(parents[0].clone(), projected.clone()))
                                .is_some_and(|ids| ids.len() > 1)
                        }))
                {
                    Some(ProjectionIssue::NameCollision)
                } else if projections::kind(mime) == RemoteKind::Workspace
                    && file.baseline.state != crate::inventory::models::BaselineState::Synced
                    && (file.local.state == FileState::Present
                        || file.environment.destination_local_occupied == Evidence::Known(true))
                {
                    Some(ProjectionIssue::OccupiedWorkspacePath)
                } else if projections::kind(mime) == RemoteKind::Binary
                    && file.remote.state == FileState::Present
                    && (!matches!(&file.remote.checksum,Evidence::Known(h) if h.algorithm=="sha256" && h.value.len()==64 && h.value.bytes().all(|b|b.is_ascii_hexdigit()))
                        || !matches!(file.remote.size, Evidence::Known(_)))
                {
                    Some(ProjectionIssue::MissingRemoteContentEvidence)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        file.environment.projection_issue = reason;
        if file.environment.projection_issue.is_none()
            && file.identity.remote_id.is_none()
            && file.local.state == FileState::Present
        {
            if let (Evidence::Known(parent), Evidence::Known(name)) = (
                &file.environment.destination_remote_parent_id,
                &file.local.name,
            ) {
                if view.projected_matches.get(&(parent.clone(),name.clone())).is_some_and(|ids|ids.iter().any(|id|view.remote.get(id).is_some_and(|r|matches!(&r.mime_type,Evidence::Known(m) if projections::kind(m)==RemoteKind::Workspace))))
                    || view.matches.get(&(parent.clone(),name.clone())).is_some_and(|ids|ids.iter().any(|id|view.remote.get(id).is_some_and(|r|matches!(&r.mime_type,Evidence::Known(m) if projections::kind(m)==RemoteKind::Workspace)))) {
                    file.environment.projection_issue=Some(ProjectionIssue::NameCollision);
                }
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
    snapshots: &'a [InventorySnapshot],
    captures: &'a Captures,
    local: BTreeMap<Path, &'a LocalObservation>,
    remote: BTreeMap<RemoteId, &'a RemoteObservation>,
    associated: BTreeMap<RemoteId, Path>,
    local_parents: BTreeMap<Path, Vec<RemoteId>>,
    matches: BTreeMap<(RemoteId, String), Vec<RemoteId>>,
    projected: BTreeMap<RemoteId, Path>,
    candidates: BTreeMap<Path, Vec<&'a RemoteObservation>>,
    projected_matches: BTreeMap<(RemoteId, String), Vec<RemoteId>>,
    local_identities: BTreeSet<IdentityKey>,
    incarnations: BTreeMap<IdentityKey, Vec<&'a LocalObservation>>,
    owners: BTreeMap<Path, usize>,
    persisted_identities: BTreeMap<IdentityKey, usize>,
    local_complete: bool,
    remote_complete: bool,
}
impl<'a> View<'a> {
    fn locate(
        &self,
        identity: &Evidence<crate::models::LocalIdentity>,
    ) -> Option<&'a LocalObservation> {
        let Evidence::Known(id) = identity else {
            return None;
        };
        if id.birth_ns.is_none()
            || self.persisted_identities.get(&identity_key(identity)?) != Some(&1)
        {
            return None;
        }
        let mut entries = self
            .incarnations
            .get(&identity_key(identity)?)?
            .iter()
            .filter(|entry| &entry.identity == identity);
        let entry = *entries.next()?;
        entries.next().is_none().then_some(entry)
    }

    fn new(
        captures: &'a Captures,
        snapshots: &'a [InventorySnapshot],
        root: &RemoteId,
        trace: Trace<'_>,
    ) -> Self {
        let local = captures.locals(trace);
        let remote = captures.remotes(trace);
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
        let mut local_indexes = trace.batch(Work::Index, local.len() * 2);
        let local_identities = local
            .values()
            .inspect(|_| local_indexes.advance())
            .filter_map(|entry| identity_key(&entry.identity))
            .collect();
        let mut incarnations: BTreeMap<IdentityKey, Vec<&LocalObservation>> = BTreeMap::new();
        for entry in local.values() {
            local_indexes.advance();
            if let Some(key) = identity_key(&entry.identity) {
                incarnations.entry(key).or_default().push(*entry);
            }
        }
        local_indexes.finish();
        let mut inventory_indexes = trace.batch(Work::Index, snapshots.len() * 2);
        let mut owners = BTreeMap::new();
        let mut persisted_identities = BTreeMap::new();
        for snapshot in snapshots {
            inventory_indexes.advance();
            if let Some(path) = &snapshot.local_path {
                *owners.entry(path.clone()).or_insert(0) += 1;
            }
            if let Some(key) = identity_key(&snapshot.identity.local_identity) {
                *persisted_identities.entry(key).or_insert(0) += 1;
            }
        }
        let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for snapshot in snapshots {
            inventory_indexes.advance();
            if let Some(id) = &snapshot.identity.remote_id {
                grouped.entry(id).or_default().push(snapshot);
            }
        }
        inventory_indexes.finish();
        let mut associations = trace.batch(Work::Index, grouped.len());
        let mut associated = BTreeMap::from([(root.clone(), Path::new())]);
        let mut local_parents: BTreeMap<Path, Vec<RemoteId>> = BTreeMap::new();
        for (id, records) in grouped {
            associations.advance();
            if records.len() != 1 {
                continue;
            }
            let snapshot = records[0];
            let current = identity_key(&snapshot.identity.local_identity)
                .and_then(|key| incarnations.get(&key))
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|entry| entry.identity == snapshot.identity.local_identity)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let Some(l) = current.first().filter(|_| current.len() == 1) else {
                continue;
            };
            let Some(path) = l.path.as_ref() else {
                continue;
            };
            let Some(r) = remote.get(id) else {
                continue;
            };
            if snapshot.baseline.state == crate::inventory::models::BaselineState::Synced
                && !snapshot.pending_operations.iter().any(|op| {
                    matches!(
                        op.state,
                        crate::inventory::models::OperationState::Pending
                            | crate::inventory::models::OperationState::Running
                            | crate::inventory::models::OperationState::AppliedUnconfirmed
                    )
                })
                && (snapshot.local_path.as_ref() == Some(path)
                    || matches!(&l.identity, Evidence::Known(i) if i.birth_ns.is_some()))
                && l.kind == Evidence::Known(FileKind::Directory)
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
        associations.finish();
        let mut remote_indexes = trace.batch(Work::Index, remote.len());
        let mut matches: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut projected = BTreeMap::new();
        let mut projected_matches: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut candidates: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (id, entry) in &remote {
            remote_indexes.advance();
            if entry.state != FileState::Present || entry.trashed != Evidence::Known(false) {
                continue;
            }
            if let (Evidence::Known(parents), Evidence::Known(name)) = (&entry.parents, &entry.name)
            {
                if parents.len() == 1 {
                    if let Evidence::Known(mime) = &entry.mime_type {
                        if let Some(name) = crate::projections::local_name(name, mime) {
                            projected_matches
                                .entry((parents[0].clone(), name))
                                .or_default()
                                .push(id.clone());
                        }
                    }
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
        remote_indexes.finish();
        Self {
            snapshots,
            captures,
            local,
            remote,
            associated,
            local_parents,
            matches,
            projected,
            candidates,
            projected_matches,
            local_identities,
            incarnations,
            owners,
            persisted_identities,
            local_complete,
            remote_complete,
        }
    }
}
fn same_evidence(previous: &ScopeCapture, incoming: &ScopeCapture, trace: Trace<'_>) -> bool {
    if previous.coverage != ObservationCoverage::Complete
        || incoming.coverage != ObservationCoverage::Complete
    {
        return false;
    }
    let equal_event = |a: &Option<crate::models::ObservationEvent>,
                       b: &Option<crate::models::ObservationEvent>| {
        match (a, b) {
            (Some(a), Some(b)) => {
                let mut a = a.clone();
                a.observed_at = b.observed_at;
                a == *b
            }
            (None, None) => true,
            _ => false,
        }
    };
    let total = match &incoming.entries {
        CaptureEntries::Local(entries) => entries.len(),
        CaptureEntries::Remote(entries) => entries.len(),
    };
    let mut comparison = trace.batch(Work::Compare, total);
    let equal = match (&previous.entries, &incoming.entries) {
        (CaptureEntries::Local(a), CaptureEntries::Local(b)) => {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    comparison.advance();
                    a.previous_path == b.previous_path
                        && a.coverage == b.coverage
                        && local_state(a.clone()) == local_state(b.clone())
                        && equal_event(&a.event, &b.event)
                })
        }
        (CaptureEntries::Remote(a), CaptureEntries::Remote(b)) => {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    comparison.advance();
                    a.coverage == b.coverage
                        && remote_state(a.clone()) == remote_state(b.clone())
                        && equal_event(&a.event, &b.event)
                })
        }
        _ => false,
    };
    comparison.finish();
    equal
}

impl Captures {
    fn republish(&self, mut files: Vec<File>, trace: Trace<'_>) -> Vec<File> {
        let locals = self.locals(trace);
        let remotes = self.remotes(trace);
        let mut reuse = trace.batch(Work::Reuse, files.len());
        let local = self.local.as_ref().unwrap();
        let remote = self.remote.as_ref().unwrap();
        for file in &mut files {
            let observations = &mut file.observations;
            observations.local_generation = Some(local.generation);
            observations.remote_generation = Some(remote.generation);
            observations.local_epoch = Some(local.collect_epoch);
            observations.remote_epoch = Some(remote.collect_epoch);
            observations.observed_at = local.observed_at;
            for event in &mut observations.events {
                let current = if let Some(id) = &event.remote_id {
                    remotes.get(id).and_then(|entry| entry.event.as_ref())
                } else {
                    event
                        .new_path
                        .as_ref()
                        .or(event.old_path.as_ref())
                        .and_then(|path| locals.get(path))
                        .and_then(|entry| entry.event.as_ref())
                };
                if let Some(current) = current {
                    *event = current.clone();
                }
            }
            if observations.latest_origin == ObservationOrigin::Remote {
                if let Some(entry) = file.remote.id.as_ref().and_then(|id| remotes.get(id)) {
                    observations.remote_page_token = entry.page_token.clone();
                }
            }
            reuse.advance();
        }
        reuse.finish();
        files
    }

    pub(super) fn locals(&self, trace: Trace<'_>) -> BTreeMap<Path, &LocalObservation> {
        match self.local.as_ref().map(|c| &c.entries) {
            Some(CaptureEntries::Local(entries)) => {
                let mut batch = trace.batch(Work::Index, entries.len());
                let index = entries
                    .iter()
                    .inspect(|_| batch.advance())
                    .filter_map(|e| e.path.clone().map(|p| (p, e)))
                    .collect();
                batch.finish();
                index
            }
            _ => BTreeMap::new(),
        }
    }
    pub(super) fn remotes(&self, trace: Trace<'_>) -> BTreeMap<RemoteId, &RemoteObservation> {
        match self.remote.as_ref().map(|c| &c.entries) {
            Some(CaptureEntries::Remote(entries)) => {
                let mut batch = trace.batch(Work::Index, entries.len());
                let index = entries
                    .iter()
                    .inspect(|_| batch.advance())
                    .filter_map(|e| e.remote_id.clone().map(|id| (id, e)))
                    .collect();
                batch.finish();
                index
            }
            _ => BTreeMap::new(),
        }
    }
    fn observations(&self, trace: Trace<'_>) -> Vec<Observation> {
        let mut result = Vec::new();
        if let Some(ScopeCapture {
            entries: CaptureEntries::Local(entries),
            ..
        }) = &self.local
        {
            let mut batch = trace.batch(Work::Build, entries.len());
            result.extend(
                entries
                    .iter()
                    .cloned()
                    .inspect(|_| batch.advance())
                    .map(|data| Observation::Local {
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
                    }),
            );
            batch.finish();
        }
        if let Some(ScopeCapture {
            entries: CaptureEntries::Remote(entries),
            ..
        }) = &self.remote
        {
            let mut batch = trace.batch(Work::Build, entries.len());
            result.extend(
                entries
                    .iter()
                    .cloned()
                    .inspect(|_| batch.advance())
                    .map(Observation::Remote),
            );
            batch.finish();
        }
        result
    }
}
fn valid_name(name: &str) -> bool {
    crate::projections::valid_name(name)
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
        return Some(Path::from(crate::projections::local_name(
            name,
            match &entry.mime_type {
                Evidence::Known(m) => m,
                _ => return None,
            },
        )?));
    }
    let parent = entries.get(&parents[0])?;
    if parent.state != FileState::Present
        || parent.mime_type != Evidence::Known("application/vnd.google-apps.folder".into())
    {
        return None;
    }
    Some(
        projected_path(parent, entries, root, seen)?.join(crate::projections::local_name(
            name,
            match &entry.mime_type {
                Evidence::Known(m) => m,
                _ => return None,
            },
        )?),
    )
}
