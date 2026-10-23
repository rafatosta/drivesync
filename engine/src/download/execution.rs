//! One manual, non-overwriting binary download cycle with durable replay evidence.

use super::filesystem::{identity, Root};
use super::{
    filesystem,
    models::{DownloadIssue, DownloadProgress, DownloadReport, Operation, Phase},
    stream,
};
use crate::{
    state::Store, Cancellation, DownloadPlan, EngineError, InventoryEntry, InventoryKind,
    PlanAction, RemoteInventory,
};
use std::{collections::HashMap, ffi::OsStr, io::Read, path::Path};

/// Provider seam: mocks exercise failure paths without Google or credentials.
pub(crate) trait Source {
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
    root: Root,
    operations: HashMap<std::path::PathBuf, Vec<Operation>>,
}

/// Executes a freshly built plan. Installed evidence is reverified, never trusted
/// by name alone. Fatal state/configuration errors stop before further effects.
pub(crate) fn run(
    context: Context<'_>,
    inventory: &RemoteInventory,
    source: &mut impl Source,
    mut progress: impl FnMut(DownloadProgress),
) -> Result<DownloadReport, EngineError> {
    stream::cancelled(context.cancel)?;
    if context.scope.account_id != inventory.account_id
        || context.scope.remote_root != inventory.root_id
        || context.scope.entries.len() != inventory.entries.len()
    {
        return Err(EngineError::InvalidPlan);
    }
    let root = Root::open(&context.scope.local_root)?;
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
    // Parents must exist even if a fixture/provider order is not breadth first.
    work.sort_by_key(|(_, entry)| {
        entry
            .relative_path
            .as_ref()
            .map(|p| p.components().count())
            .unwrap_or(usize::MAX)
    });
    for (item, decision) in work {
        stream::cancelled(session.context.cancel)?;
        source.configured()?;
        session.root.validate()?;
        if item.id != decision.remote_id {
            return Err(EngineError::InvalidPlan);
        }
        let result = match (&item.kind, &decision.relative_path, &decision.action) {
            (InventoryKind::Skipped(_), _, _) => {
                report.skipped += 1;
                continue;
            }
            (_, None, _) => Err(EngineError::DownloadConflict),
            (InventoryKind::Folder, Some(path), PlanAction::CreateDirectory | PlanAction::NoOp) => {
                validate_chain(source, item, &by_id, &inventory.root_id)
                    .and_then(|()| session.root.mkdir(path))
                    .map(|()| {
                        report.directories += 1;
                    })
            }
            (InventoryKind::Binary, Some(path), _) => session
                .file(item, path, &by_id, source, &mut progress)
                .map(|outcome| match outcome {
                    Outcome::Downloaded => report.downloaded += 1,
                    Outcome::Recovered => report.recovered += 1,
                    Outcome::Unchanged => report.unchanged += 1,
                }),
            _ => Err(EngineError::DownloadConflict),
        };
        if let Err(error) = result {
            match error {
                EngineError::StateStorage
                | EngineError::InvalidPlan
                | EngineError::Authentication(crate::AuthError::Cancelled) => return Err(error),
                _ => report.issues.push(DownloadIssue {
                    relative_path: decision.relative_path.clone(),
                    error,
                }),
            }
        }
    }
    Ok(report)
}
enum Outcome {
    Downloaded,
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
        if source.metadata(current)? != *current {
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
impl Session<'_> {
    fn put(&self, op: &Operation) -> Result<(), EngineError> {
        self.context
            .store
            .put_operation(self.context.client, self.context.scope, op)
    }
    fn file(
        &self,
        item: &InventoryEntry,
        path: &Path,
        by_id: &HashMap<&str, &InventoryEntry>,
        source: &mut impl Source,
        progress: &mut impl FnMut(DownloadProgress),
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
            filesystem::install(&parent, &op.temporary, &name)?;
            let mut confirmed = op.clone();
            confirmed.phase = Phase::Installed {
                identity: *expected,
                sha256: sha256.clone(),
            };
            self.put(&confirmed)?;
            return Ok(Outcome::Recovered);
        }
        if let Some(mut existing) = filesystem::read(&parent, &name)? {
            let Some(op) = self
                .operations
                .get(path)
                .into_iter()
                .flatten()
                .rev()
                .find(|op| {
                    op.item.id == item.id
                        && op.relative == path
                        && matches!(op.phase, Phase::Installed { .. })
                })
            else {
                return Err(EngineError::DownloadConflict);
            };
            let Phase::Installed {
                identity: expected,
                sha256,
            } = &op.phase
            else {
                unreachable!()
            };
            if op.item != *item
                || op.root != root_id
                || op.parent != parent_id
                || identity(&existing)? != *expected
                || !stream::verify(&mut existing, size, sha256, self.context.cancel)?
            {
                return Err(EngineError::DownloadConflict);
            }
            validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
            self.root.validate_parent(path, parent_id)?;
            return Ok(Outcome::Unchanged);
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
            phase: Phase::Started,
        };
        self.put(&op)?;
        let mut staged = filesystem::create(&parent, &op.temporary, size)?;
        let mut input = source.media(&item.id)?;
        progress(DownloadProgress {
            relative_path: path.into(),
            received: 0,
            total: size,
        });
        let sha256 = stream::copy(
            &mut input,
            &mut staged,
            size,
            checksum,
            self.context.cancel,
            |received| {
                progress(DownloadProgress {
                    relative_path: path.into(),
                    received,
                    total: size,
                })
            },
        )?;
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
        op.phase = Phase::Ready {
            identity: staged_id,
            sha256: sha256.clone(),
        };
        self.put(&op)?;
        // Cancellation after Ready still leaves enough evidence for replay.
        stream::cancelled(self.context.cancel)?;
        filesystem::install(&parent, &op.temporary, &name)?;
        op.phase = Phase::Installed {
            identity: staged_id,
            sha256,
        };
        self.put(&op)?;
        Ok(Outcome::Downloaded)
    }
}
