//! Manual downloads and managed updates with durable replay evidence.
mod updates;

use super::filesystem::{identity, Root};
use super::{
    filesystem,
    models::{
        DownloadIssue, DownloadItemProgress, DownloadProgress, DownloadReport, DownloadStage,
        Operation, Phase, Previous,
    },
    stream,
};
use crate::{
    state::Store, Cancellation, DownloadPlan, EngineError, InventoryEntry, InventoryKind,
    PlanAction, RemoteInventory,
};
use std::{collections::HashMap, ffi::OsStr, io::Read, path::Path};

/// Provider seam: mocks exercise failure paths without Google or credentials.
pub(crate) trait Source {
    /// A complete feed checkpoint proves only observation equality, not local bytes.
    /// Returning true permits a no-effect unchanged result after journal/hash checks.
    fn cached_unchanged(&self, _item: &InventoryEntry) -> bool {
        false
    }
    /// Checks account/root configuration between items without mandatory network I/O.
    fn check_configuration(&mut self) -> Result<(), EngineError> {
        self.configured()
    }
    fn metadata(&mut self, item: &InventoryEntry) -> Result<InventoryEntry, EngineError>;
    fn media(&mut self, id: &str) -> Result<Box<dyn Read>, EngineError>;
    /// Rechecks the saved configuration and authorized remote root.
    fn configured(&mut self) -> Result<(), EngineError>;
}
pub(crate) struct Context<'a> {
    pub client: &'a str,
    pub scope: &'a DownloadPlan,
    pub store: &'a Store,
    pub cancel: &'a Cancellation,
}
struct Session<'a> {
    context: Context<'a>,
    root: &'a Root,
    operations: HashMap<std::path::PathBuf, Vec<Operation>>,
    preserved: std::cell::RefCell<Option<std::path::PathBuf>>,
}

/// Executes a freshly built plan. Installed evidence is reverified, never trusted
/// by name alone. Fatal state/configuration errors stop before further effects.
#[cfg(test)]
pub(crate) fn run(
    context: Context<'_>,
    inventory: &RemoteInventory,
    source: &mut impl Source,
    progress: impl FnMut(DownloadProgress),
) -> Result<DownloadReport, EngineError> {
    let root = Root::open(&context.scope.local_root)?;
    run_with_root(context, inventory, source, progress, &root)
}

/// Executes with the lock and descriptor retained from preparation.
pub(crate) fn run_with_root<'a>(
    context: Context<'a>,
    inventory: &RemoteInventory,
    source: &mut impl Source,
    mut progress: impl FnMut(DownloadProgress),
    root: &'a Root,
) -> Result<DownloadReport, EngineError> {
    root.validate()?;
    source.configured()?;
    stream::cancelled(context.cancel)?;
    if context.scope.account_id != inventory.account_id
        || context.scope.remote_root != inventory.root_id
        || context.scope.entries.len() != inventory.entries.len()
    {
        return Err(EngineError::InvalidPlan);
    }
    let mut operations: HashMap<std::path::PathBuf, Vec<Operation>> = HashMap::new();
    for operation in context.store.operations(context.client, context.scope)? {
        operations
            .entry(operation.relative.clone())
            .or_default()
            .push(operation);
    }
    let session = Session {
        context,
        root,
        operations,
        preserved: Default::default(),
    };
    let mut report = DownloadReport::default();
    let by_id: HashMap<_, _> = inventory
        .entries
        .iter()
        .map(|e| (e.id.as_str(), e))
        .collect();
    let mut work: Vec<_> = inventory
        .entries
        .iter()
        .zip(&session.context.scope.entries)
        .collect();
    let represented: std::collections::HashSet<_> = work
        .iter()
        .filter_map(|(item, entry)| {
            (item.kind == InventoryKind::Binary)
                .then(|| {
                    entry
                        .relative_path
                        .as_deref()
                        .map(|path| (item.id.as_str(), path))
                })
                .flatten()
        })
        .collect();
    for op in session
        .operations
        .values()
        .flatten()
        .filter(|op| matches!(op.phase, Phase::UpdateReady { .. }))
    {
        let present = represented.contains(&(op.item.id.as_str(), op.relative.as_path()));
        if !present {
            stream::cancelled(session.context.cancel)?;
            let result = session.retire_update(op);
            if let Some(copy) = session.preserved.borrow_mut().take() {
                report.preserved_copies.push(copy);
            }
            match result {
                Err(EngineError::StateStorage) => return Err(EngineError::StateStorage),
                Err(EngineError::Authentication(crate::AuthError::Cancelled)) => {
                    return Err(EngineError::Authentication(crate::AuthError::Cancelled))
                }
                _ => report.issues.push(DownloadIssue {
                    relative_path: Some(op.relative.clone()),
                    error: result.err().unwrap_or(EngineError::RemoteChanged),
                }),
            }
        }
    }
    // Parents must exist even if a fixture/provider order is not breadth first.
    work.sort_by_key(|(_, entry)| {
        entry
            .relative_path
            .as_ref()
            .map(|p| p.components().count())
            .unwrap_or(usize::MAX)
    });
    let total_items = work.len();
    progress(DownloadProgress {
        preparation: None,
        completed_items: 0,
        total_items,
        current: None,
    });
    for (completed_items, (item, decision)) in work.into_iter().enumerate() {
        progress(DownloadProgress {
            preparation: None,
            completed_items,
            total_items,
            current: Some(DownloadItemProgress {
                relative_path: decision
                    .relative_path
                    .clone()
                    .unwrap_or_else(|| item.name.clone().into()),
                stage: DownloadStage::Checking,
                received: 0,
                total: item.size,
            }),
        });
        stream::cancelled(session.context.cancel)?;
        source.configured()?;
        session.root.validate()?;
        if item.id != decision.remote_id {
            return Err(EngineError::InvalidPlan);
        }
        let result = match (&item.kind, &decision.relative_path, &decision.action) {
            (InventoryKind::Skipped(_), _, _) => {
                report.skipped += 1;
                Ok(())
            }
            (_, None, _) => Err(EngineError::DownloadConflict),
            (InventoryKind::Folder, Some(path), PlanAction::NoOp)
                if source.cached_unchanged(item) =>
            {
                session
                    .root
                    .check_directory(path)
                    .map(|()| report.directories += 1)
            }
            (InventoryKind::Folder, Some(path), PlanAction::CreateDirectory | PlanAction::NoOp) => {
                validate_chain(source, item, &by_id, &inventory.root_id)
                    .and_then(|()| session.root.mkdir(path))
                    .map(|()| {
                        report.directories += 1;
                    })
            }
            (InventoryKind::GoogleWorkspaceLink, Some(path), PlanAction::CreateLink) => {
                validate_chain(source, item, &by_id, &inventory.root_id).and_then(|()| {
                    let url = item
                        .web_view_link
                        .as_deref()
                        .ok_or(EngineError::InvalidPlan)?;
                    progress(DownloadProgress {
                        preparation: None,
                        completed_items,
                        total_items,
                        current: Some(DownloadItemProgress {
                            relative_path: path.clone(),
                            stage: DownloadStage::Linking,
                            received: 0,
                            total: None,
                        }),
                    });
                    super::links::ensure(
                        session.root,
                        path,
                        &item.name,
                        &item.mime_type,
                        url,
                        session.context.cancel,
                    )
                    .map(|()| report.linked += 1)
                })
            }
            (InventoryKind::Binary, Some(path), PlanAction::Equivalent) => session
                .equivalent(item, path, decision, &by_id, source)
                .map(|()| report.unchanged += 1),
            (InventoryKind::Binary, Some(_), PlanAction::DeferredUnknown) => {
                let version = item
                    .version
                    .filter(|v| *v > 0)
                    .ok_or(EngineError::InvalidPlan)?;
                let bases = session.context.store.confirmed_bases(
                    session.context.client,
                    &session.context.scope.account_id,
                    &session.context.scope.remote_root,
                    &session.context.scope.local_root,
                )?;
                Err(
                    if bases
                        .get(&item.id)
                        .is_some_and(|b| version < b.remote_version)
                    {
                        EngineError::RemoteChanged
                    } else if decision.expected_local == crate::LocalObservation::Unknown {
                        EngineError::ObservationUnknown
                    } else {
                        EngineError::DownloadConflict
                    },
                )
            }
            (InventoryKind::Binary, Some(path), _) => session
                .file(item, path, &by_id, source, &mut |current| {
                    progress(DownloadProgress {
                        preparation: None,
                        completed_items,
                        total_items,
                        current: Some(current),
                    })
                })
                .map(|outcome| match outcome {
                    Outcome::Downloaded => report.downloaded += 1,
                    Outcome::Updated => report.updated += 1,
                    Outcome::Recovered => report.recovered += 1,
                    Outcome::Unchanged => report.unchanged += 1,
                }),
            _ => Err(EngineError::DownloadConflict),
        };
        if let Some(copy) = session.preserved.borrow_mut().take() {
            report.preserved_copies.push(copy);
        }
        if let Err(error) = result {
            match error {
                EngineError::StateStorage
                | EngineError::RootSuspended(_)
                | EngineError::InvalidPlan
                | EngineError::Authentication(crate::AuthError::Cancelled) => return Err(error),
                _ => report.issues.push(DownloadIssue {
                    relative_path: decision.relative_path.clone(),
                    error,
                }),
            }
        }
        progress(DownloadProgress {
            preparation: None,
            completed_items: completed_items + 1,
            total_items,
            current: None,
        });
    }
    Ok(report)
}
enum Outcome {
    Downloaded,
    Updated,
    Recovered,
    Unchanged,
}
/// Revalidates all known ancestors as well as the leaf, detecting observed moves.
fn validate_chain(
    source: &mut impl Source,
    item: &InventoryEntry,
    by_id: &HashMap<&str, &InventoryEntry>,
    root: &str,
) -> Result<(), EngineError> {
    let mut current = item;
    for _ in 0..=by_id.len() {
        if !matches_metadata(current, &source.metadata(current)?) {
            return Err(EngineError::RemoteChanged);
        }
        if current.parent_id == root {
            return Ok(());
        }
        current = by_id
            .get(current.parent_id.as_str())
            .copied()
            .filter(|p| p.kind == InventoryKind::Folder)
            .ok_or(EngineError::InvalidPlan)?;
    }
    Err(EngineError::InvalidPlan)
}
/// Folder versions also reflect unrelated metadata changes. Location checks use
/// identity, parent, name and type; binary bytes still require the exact version.
fn matches_metadata(expected: &InventoryEntry, current: &InventoryEntry) -> bool {
    if expected.kind == InventoryKind::Folder {
        current.kind == InventoryKind::Folder
            && current.id == expected.id
            && current.parent_id == expected.parent_id
            && current.name == expected.name
            && current.mime_type == expected.mime_type
    } else {
        current == expected
    }
}
impl Session<'_> {
    fn put(&self, op: &Operation) -> Result<(), EngineError> {
        self.context
            .store
            .put_operation(self.context.client, self.context.scope, op)
    }
    /// Equal bytes never bypass identity, latest-operation or scope checks.
    fn equivalent(
        &self,
        item: &InventoryEntry,
        path: &Path,
        decision: &crate::PlanEntry,
        by_id: &HashMap<&str, &InventoryEntry>,
        source: &mut impl Source,
    ) -> Result<(), EngineError> {
        let base = self
            .context
            .store
            .confirmed_bases(
                self.context.client,
                &self.context.scope.account_id,
                &self.context.scope.remote_root,
                &self.context.scope.local_root,
            )?
            .remove(&item.id)
            .ok_or(EngineError::DownloadConflict)?;
        let root_id = identity(&self.root.file)?;
        let parent_id = identity(&self.root.parent(path)?.0)?;
        let valid_binding = self.operations.get(path).into_iter().flatten().any(|op| {
            op.item.id == item.id
                && op.item.parent_id == item.parent_id
                && op.item.name == item.name
                && op.item.kind == item.kind
                && op.root == root_id
                && op.parent == parent_id
                && op.item.mime_type == item.mime_type
                && matches!(op.phase, Phase::Installed { .. } | Phase::Updated { .. })
        });
        if !valid_binding
            || crate::planner::decide(
                Some(&base),
                &decision.expected_local,
                item.version.ok_or(EngineError::InvalidPlan)?,
            ) != PlanAction::Blocked(crate::ConflictReason::ConcurrentChanges)
        {
            return Err(EngineError::DownloadConflict);
        }
        let expected = match &decision.expected_local {
            crate::LocalObservation::Verified(value) => value,
            _ => return Err(EngineError::ObservationUnknown),
        };
        if filesystem::fingerprint_root(self.root, path, self.context.cancel)?.as_ref()
            != Some(expected)
        {
            return Err(EngineError::LocalWriting);
        }
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        let remote = super::content_fingerprint(item, source, self.context.cancel)?;
        if expected.split(':').next() != Some(remote.as_str()) {
            return Err(EngineError::DownloadConflict);
        }
        if filesystem::fingerprint_root(self.root, path, self.context.cancel)?.as_ref()
            != Some(expected)
        {
            return Err(EngineError::LocalWriting);
        }
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        self.root.validate_parent(path, parent_id)
    }
    fn file(
        &self,
        item: &InventoryEntry,
        path: &Path,
        by_id: &HashMap<&str, &InventoryEntry>,
        source: &mut impl Source,
        progress: &mut impl FnMut(DownloadItemProgress),
    ) -> Result<Outcome, EngineError> {
        let size = item.size.ok_or(EngineError::InvalidPlan)?;
        let checksum = item
            .md5_checksum
            .as_deref()
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or(EngineError::InvalidPlan)?;
        if item.version.is_none_or(|v| v == 0) {
            return Err(EngineError::InvalidPlan);
        }
        let (parent, name) = self.root.parent(path)?;
        let root_id = identity(&self.root.file)?;
        let parent_id = identity(&parent)?;
        if let Some(op) = self
            .operations
            .get(path)
            .into_iter()
            .flatten()
            .rev()
            .find(|op| matches!(op.phase, Phase::UpdateReady { .. }))
        {
            return self.resume_update(op, item, by_id, source, progress);
        }
        // Replay Ready first: rename may already have happened without DB confirmation.
        if let Some(op) = self
            .operations
            .get(path)
            .into_iter()
            .flatten()
            .rev()
            .find(|op| op.relative == path && matches!(op.phase, Phase::Ready { .. }))
        {
            if op.root != root_id || op.parent != parent_id {
                return Err(EngineError::DownloadConflict);
            }
            let Phase::Ready {
                identity: expected,
                sha256,
            } = &op.phase
            else {
                unreachable!()
            };
            if let Some(mut final_file) = filesystem::read(&parent, &name)? {
                if identity(&final_file)? != *expected
                    || !stream::verify(
                        &mut final_file,
                        op.item.size.ok_or(EngineError::StateStorage)?,
                        sha256,
                        self.context.cancel,
                    )?
                {
                    return Err(EngineError::DownloadConflict);
                }
                self.root.validate_parent(path, parent_id)?;
                parent.sync_all().map_err(filesystem::io)?;
                let mut confirmed = op.clone();
                confirmed.phase = Phase::Installed {
                    identity: *expected,
                    sha256: sha256.clone(),
                };
                self.put(&confirmed)?;
                if op.item != *item {
                    return Err(EngineError::RemoteChanged);
                }
                return Ok(Outcome::Recovered);
            }
            if op.item != *item {
                return Err(EngineError::RemoteChanged);
            }
            let mut staged = filesystem::read(&parent, OsStr::new(&op.temporary))?
                .ok_or(EngineError::DownloadConflict)?;
            if identity(&staged)? != *expected
                || !stream::verify(&mut staged, size, sha256, self.context.cancel)?
            {
                return Err(EngineError::DownloadIntegrity);
            }
            source.configured()?;
            validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
            self.root.validate_parent(path, parent_id)?;
            stream::cancelled(self.context.cancel)?;
            progress(DownloadItemProgress {
                relative_path: path.into(),
                stage: DownloadStage::Installing,
                received: size,
                total: Some(size),
            });
            filesystem::install(&parent, &op.temporary, &name)?;
            let mut confirmed = op.clone();
            confirmed.phase = Phase::Installed {
                identity: *expected,
                sha256: sha256.clone(),
            };
            self.put(&confirmed)?;
            return Ok(Outcome::Recovered);
        }
        let mut previous = None;
        if let Some(mut existing) = filesystem::read(&parent, &name)? {
            let Some(op) = self
                .operations
                .get(path)
                .into_iter()
                .flatten()
                .rev()
                .find(|op| {
                    op.item.id == item.id
                        && matches!(op.phase, Phase::Installed { .. } | Phase::Updated { .. })
                })
            else {
                return Err(EngineError::DownloadConflict);
            };
            let (expected, sha256) = match &op.phase {
                Phase::Installed { identity, sha256 }
                | Phase::Updated {
                    identity, sha256, ..
                } => (*identity, sha256),
                _ => unreachable!(),
            };
            let old_size = op.item.size.ok_or(EngineError::StateStorage)?;
            let old_version = op.item.version.ok_or(EngineError::StateStorage)?;
            if op.root != root_id
                || op.parent != parent_id
                || op.item.parent_id != item.parent_id
                || op.item.name != item.name
                || op.item.mime_type != item.mime_type
                || op.item.kind != item.kind
                || identity(&existing)? != expected
                || !stream::verify(&mut existing, old_size, sha256, self.context.cancel)?
            {
                return Err(EngineError::DownloadConflict);
            }
            if op.item == *item && source.cached_unchanged(item) {
                self.root.validate_parent(path, parent_id)?;
                return Ok(Outcome::Unchanged);
            }
            validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
            self.root.validate_parent(path, parent_id)?;
            if op.item == *item {
                return Ok(Outcome::Unchanged);
            }
            if item.version.is_none_or(|version| version <= old_version) {
                return Err(EngineError::RemoteChanged);
            }
            if op.item.size == item.size
                && op
                    .item
                    .md5_checksum
                    .as_ref()
                    .is_some_and(|old| old.eq_ignore_ascii_case(checksum))
            {
                // A newer metadata version does not necessarily change file bytes.
                // Recheck the path after remote I/O before advancing durable evidence.
                source.configured()?;
                let mut current =
                    filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
                if identity(&current)? != expected
                    || !stream::verify(&mut current, old_size, sha256, self.context.cancel)?
                {
                    return Err(EngineError::DownloadConflict);
                }
                self.root.validate_parent(path, parent_id)?;
                stream::cancelled(self.context.cancel)?;
                let mut confirmed = op.clone();
                confirmed.item = item.clone();
                self.put(&confirmed)?;
                return Ok(Outcome::Unchanged);
            }
            previous = Some(Previous {
                identity: expected,
                sha256: sha256.clone(),
                size: old_size,
                version: old_version,
            });
        }
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        let id = oauth2::CsrfToken::new_random().secret().clone();
        let mut op = Operation {
            id: id.clone(),
            item: item.clone(),
            relative: path.into(),
            temporary: format!(".drivesync-{id}.part"),
            root: root_id,
            parent: parent_id,
            phase: previous
                .as_ref()
                .map(|previous| Phase::Updating {
                    previous: previous.clone(),
                })
                .unwrap_or(Phase::Started),
        };
        self.root.validate_parent(path, parent_id)?;
        self.put(&op)?;
        let mut staged = filesystem::create(&parent, &op.temporary, size)?;
        progress(DownloadItemProgress {
            relative_path: path.into(),
            stage: DownloadStage::Downloading,
            received: 0,
            total: Some(size),
        });
        let mut input = source.media(&item.id)?;
        let sha256 = stream::copy(
            &mut input,
            &mut staged,
            size,
            checksum,
            self.context.cancel,
            |received| {
                progress(DownloadItemProgress {
                    relative_path: path.into(),
                    stage: DownloadStage::Downloading,
                    received,
                    total: Some(size),
                })
            },
            || {
                self.root.validate()?;
                source.check_configuration()
            },
        )?;
        progress(DownloadItemProgress {
            relative_path: path.into(),
            stage: DownloadStage::Verifying,
            received: size,
            total: Some(size),
        });
        staged.sync_all().map_err(filesystem::io)?;
        parent.sync_all().map_err(filesystem::io)?;
        let staged_id = identity(&staged)?;
        // A second local read detects staging mutations during/after streaming.
        use std::io::{Seek, SeekFrom};
        staged.seek(SeekFrom::Start(0)).map_err(filesystem::io)?;
        if !stream::verify(&mut staged, size, &sha256, self.context.cancel)? {
            return Err(EngineError::DownloadIntegrity);
        }
        source.configured()?;
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        self.root.validate_parent(path, parent_id)?;
        let current = filesystem::read(&parent, OsStr::new(&op.temporary))?
            .ok_or(EngineError::DownloadConflict)?;
        if identity(&current)? != staged_id {
            return Err(EngineError::DownloadConflict);
        }
        stream::cancelled(self.context.cancel)?;
        if let Some(previous) = previous {
            op.phase = Phase::UpdateReady {
                identity: staged_id,
                sha256,
                previous,
                backup: format!("DriveSync backup {}.bak", op.id),
            };
            self.put(&op)?;
            stream::cancelled(self.context.cancel)?;
            progress(DownloadItemProgress {
                relative_path: path.into(),
                stage: DownloadStage::Installing,
                received: size,
                total: Some(size),
            });
            return self.commit_update(&op);
        }
        op.phase = Phase::Ready {
            identity: staged_id,
            sha256: sha256.clone(),
        };
        self.put(&op)?;
        // Cancellation after Ready still leaves enough evidence for replay.
        stream::cancelled(self.context.cancel)?;
        progress(DownloadItemProgress {
            relative_path: path.into(),
            stage: DownloadStage::Installing,
            received: size,
            total: Some(size),
        });
        filesystem::install(&parent, &op.temporary, &name)?;
        op.phase = Phase::Installed {
            identity: staged_id,
            sha256,
        };
        self.put(&op)?;
        Ok(Outcome::Downloaded)
    }
}
