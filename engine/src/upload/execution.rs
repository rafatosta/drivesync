//! Serial create-only execution. Intent precedes effects; identity confirms replay.
use super::{local, transport::Position, Intent};
use crate::{
    download::{
        filesystem::{self, Root},
        Identity, Operation, Phase,
    },
    state::{Store, Uploads},
    Cancellation, DownloadIssue, DownloadItemProgress, DownloadProgress, DownloadReport,
    DownloadStage, EngineError, InventoryEntry, InventoryKind, RemoteInventory,
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Provider seam includes configuration/parent revalidation before every effect.
pub(crate) trait Remote {
    fn configured(&mut self) -> Result<(), EngineError>;
    fn generate(&mut self) -> Result<String, EngineError>;
    fn children(&mut self, parent: &str, name: &str) -> Result<Vec<InventoryEntry>, EngineError>;
    fn stat(&mut self, op: &Intent) -> Result<Option<InventoryEntry>, EngineError>;
    fn folder(&mut self, op: &Intent) -> Result<(), EngineError>;
    fn begin(&mut self, op: &Intent) -> Result<String, EngineError>;
    fn send(
        &mut self,
        op: &Intent,
        offset: Option<u64>,
        bytes: Vec<u8>,
    ) -> Result<Position, EngineError>;
    fn validate_parent(&mut self, parent: &str) -> Result<(), EngineError>;
    fn remember(&mut self, item: &InventoryEntry);
}
fn matches(op: &Intent, item: &InventoryEntry) -> bool {
    item.id == op.id
        && item.parent_id == op.parent
        && item.name == op.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
        && if op.directory {
            item.kind == InventoryKind::Folder
        } else {
            item.kind == InventoryKind::Binary
                && item.size == Some(op.size)
                && item.md5_checksum.as_deref() == Some(op.md5.as_str())
        }
}
fn completed(remote: &mut impl Remote, op: &Intent) -> Result<Option<InventoryEntry>, EngineError> {
    match remote.stat(op)? {
        Some(item) if matches(op, &item) => Ok(Some(item)),
        Some(_) => Err(EngineError::DownloadConflict),
        None => Ok(None),
    }
}
pub(crate) struct Context<'a> {
    pub client: &'a str,
    pub root_path: &'a Path,
    pub state_path: &'a Path,
    pub inventory: &'a RemoteInventory,
    pub uploads: &'a Uploads,
    pub cancel: &'a Cancellation,
    pub excluded: HashSet<PathBuf>,
}
#[cfg(test)]
pub(crate) fn run(
    context: Context<'_>,
    remote: &mut impl Remote,
    report: &mut DownloadReport,
    progress: impl FnMut(DownloadProgress),
) -> Result<(), EngineError> {
    let root = Root::open(context.root_path)?;
    run_with_root(context, remote, report, progress, &root)
}

/// Keeps the preparation lock for the entire upload pass.
pub(crate) fn run_with_root(
    context: Context<'_>,
    remote: &mut impl Remote,
    report: &mut DownloadReport,
    mut progress: impl FnMut(DownloadProgress),
    root: &Root,
) -> Result<(), EngineError> {
    let Context {
        client,
        root_path,
        state_path,
        inventory,
        uploads,
        cancel,
        excluded,
    } = context;
    progress(DownloadProgress::preparing(
        crate::PreparationProgress::ScanningUploads,
    ));
    root.validate()?;
    remote.configured()?;
    uploads.validate_root(root)?;
    let observed = crate::local::scan(root, cancel)?;
    let mut paths: Vec<_> = observed.entries.iter().collect();
    paths.sort_by_key(|(path, _)| (path.components().count(), (*path).clone()));
    let plan = crate::planner::plan(inventory, root_path, |_| crate::LocalObservation::Unknown)?;
    let mut parents = HashMap::from([(PathBuf::new(), inventory.root_id.clone())]);
    let mut blocked = HashSet::<PathBuf>::new();
    let mut known = HashMap::<PathBuf, Vec<&InventoryEntry>>::new();
    for (entry, decision) in inventory.entries.iter().zip(&plan.entries) {
        if let Some(path) = &decision.relative_path {
            known.entry(path.clone()).or_default().push(entry);
        }
    }
    let journal = Store::open(state_path)?;
    // A relocated managed identity is pending reconciliation, not a new upload.
    let managed: HashSet<_> = journal
        .confirmed_bases(client, &inventory.account_id, &inventory.root_id, root_path)?
        .values()
        .filter_map(|base| {
            crate::planner::fingerprint_parts(&base.local_fingerprint).map(|(_, d, i)| (d, i))
        })
        .collect();
    let total = paths.len();
    for (index, (path, evidence)) in paths.into_iter().enumerate() {
        let path = path.clone();
        let directory = *evidence == crate::local::Evidence::Directory;
        if let crate::local::Evidence::Stable { stamp, .. } = evidence {
            if !known.contains_key(&path) && managed.contains(&(stamp.device, stamp.inode)) {
                continue;
            }
        }
        local::check(cancel)?;
        remote.configured()?;
        root.validate()?;
        if excluded.iter().any(|p| path.starts_with(p))
            || blocked.iter().any(|p| path.starts_with(p))
        {
            continue;
        }
        let result = (|| {
            let parent_path = path.parent().ok_or(EngineError::InvalidPlan)?;
            let parent = parents
                .get(parent_path)
                .ok_or(EngineError::DownloadConflict)?
                .clone();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|s| !s.chars().any(char::is_control) && s.len() <= 255)
                .ok_or(EngineError::DownloadConflict)?;
            let existing = uploads.load(&path)?;
            if let Some(items) = known.get(&path) {
                if directory
                    && items.len() == 1
                    && items[0].kind == InventoryKind::Folder
                    && existing.is_none()
                {
                    root.check_directory(&path)?;
                    parents.insert(path.clone(), items[0].id.clone());
                    return Ok(());
                }
                if existing.is_none() {
                    return Ok(());
                }
            }
            let mut op = match existing {
                Some(op) => op,
                None => {
                    match evidence {
                        crate::local::Evidence::Stable { stamp, .. } => {
                            if crate::local::stamp(root, &path)? != *stamp {
                                return Err(EngineError::LocalWriting);
                            }
                        }
                        crate::local::Evidence::Directory => {}
                        crate::local::Evidence::Temporary => {
                            return Err(EngineError::LocalTemporary)
                        }
                        crate::local::Evidence::Writing => return Err(EngineError::LocalWriting),
                        crate::local::Evidence::Unknown => {
                            return Err(EngineError::ObservationUnknown)
                        }
                        crate::local::Evidence::Unsupported => {
                            return Err(EngineError::DownloadConflict)
                        }
                    }
                    remote.validate_parent(&parent)?;
                    let siblings = remote.children(&parent, name)?;
                    if !siblings.is_empty() {
                        if directory
                            && siblings.len() == 1
                            && siblings[0].kind == InventoryKind::Folder
                        {
                            root.check_directory(&path)?;
                            remote.remember(&siblings[0]);
                            parents.insert(path.clone(), siblings[0].id.clone());
                            return Ok(());
                        }
                        return Err(EngineError::DownloadConflict);
                    }
                    let id = remote.generate()?;
                    if !crate::configuration::valid_id(&id) || id.len() > 190 {
                        return Err(EngineError::InvalidPlan);
                    }
                    let snapshot = format!("upload-{id}.snapshot");
                    progress(DownloadProgress::preparing(
                        crate::PreparationProgress::PreparingUpload {
                            current_path: path.clone(),
                        },
                    ));
                    let (size, md5, sha, source) = if directory {
                        root.check_directory(&path)?;
                        (0, String::new(), String::new(), None)
                    } else {
                        if let crate::local::Evidence::Stable { stamp, .. } = evidence {
                            if crate::local::stamp(root, &path)? != *stamp {
                                return Err(EngineError::LocalWriting);
                            }
                        }
                        let (size, md5, sha, identity) = local::snapshot(
                            root,
                            &path,
                            &state_path.with_file_name(&snapshot),
                            cancel,
                        )?;
                        if let crate::local::Evidence::Stable { stamp, fingerprint } = evidence {
                            let current = crate::local::stamp(root, &path);
                            if current.as_ref() != Ok(stamp)
                                || identity.fingerprint(&sha) != *fingerprint
                            {
                                let _ = std::fs::remove_file(state_path.with_file_name(&snapshot));
                                return Err(EngineError::LocalWriting);
                            }
                        }
                        (size, md5, sha, Some(identity))
                    };
                    let op = Intent {
                        path: path.clone(),
                        root: filesystem::identity(&root.file)?,
                        source,
                        id,
                        parent: parent.clone(),
                        directory,
                        size,
                        md5,
                        sha,
                        snapshot,
                        session: None,
                        confirmed: false,
                    };
                    uploads.save(&op)?;
                    op
                }
            };
            if op.root != filesystem::identity(&root.file)?
                || op.parent != parent
                || op.directory != directory
            {
                return Err(EngineError::DownloadConflict);
            }
            if op.confirmed {
                // A removed/moved confirmed upload must never be recreated.
                if directory {
                    let item = completed(remote, &op)?.ok_or(EngineError::DownloadConflict)?;
                    remote.remember(&item);
                    parents.insert(path.clone(), item.id);
                }
                return Ok(());
            }
            let source_stamp = if !directory {
                Some(local::verify_source(root, &path, &op, cancel)?)
            } else {
                None
            };
            remote.validate_parent(&parent)?;
            let siblings = remote.children(&parent, name)?;
            if siblings.iter().any(|s| s.id != op.id) {
                return Err(EngineError::DownloadConflict);
            }
            let mut item = completed(remote, &op)?;
            if item.is_none() {
                if directory {
                    remote.folder(&op)?;
                } else {
                    let stage = state_path.with_file_name(&op.snapshot);
                    let mut file = local::verify_stage(&stage, &op, cancel)?;
                    if op.session.is_none() {
                        op.session = Some(remote.begin(&op)?);
                        uploads.save(&op)?;
                    }
                    let mut position = remote.send(&op, None, vec![])?;
                    if position == Position::Expired {
                        if let Some(found) = completed(remote, &op)? {
                            item = Some(found);
                        } else {
                            op.session = Some(remote.begin(&op)?);
                            uploads.save(&op)?;
                            position = Position::Offset(0);
                        }
                    }
                    let mut turns = 0;
                    while item.is_none() && position != Position::Complete {
                        local::check(cancel)?;
                        remote.configured()?;
                        root.validate()?;
                        remote.validate_parent(&parent)?;
                        let Position::Offset(offset) = position else {
                            return Err(EngineError::RemoteChanged);
                        };
                        if offset > op.size || (offset == op.size && op.size > 0) {
                            return Err(EngineError::RemoteChanged);
                        }
                        file.seek(SeekFrom::Start(offset)).map_err(filesystem::io)?;
                        let count = (op.size - offset).min(1024 * 1024) as usize;
                        let mut bytes = vec![0; count];
                        file.read_exact(&mut bytes).map_err(filesystem::io)?;
                        if source_stamp.as_ref() != Some(&crate::local::stamp(root, &path)?) {
                            return Err(EngineError::LocalWriting);
                        }
                        if offset + count as u64 == op.size {
                            local::verify_source(root, &path, &op, cancel)?;
                            if source_stamp.as_ref() != Some(&crate::local::stamp(root, &path)?) {
                                return Err(EngineError::LocalWriting);
                            }
                        }
                        let next = remote.send(&op, Some(offset), bytes)?;
                        if matches!(next,Position::Offset(n) if n<=offset || n>offset+count as u64)
                        {
                            return Err(EngineError::RemoteChanged);
                        }
                        progress(DownloadProgress {
                            preparation: None,
                            completed_items: index,
                            total_items: total,
                            current: Some(DownloadItemProgress {
                                relative_path: path.clone(),
                                stage: DownloadStage::Uploading,
                                received: match next {
                                    Position::Offset(n) => n,
                                    Position::Complete => op.size,
                                    Position::Expired => offset,
                                },
                                total: Some(op.size),
                            }),
                        });
                        position = next;
                        turns += 1;
                        if turns > 1_000_000 {
                            return Err(EngineError::InventoryLimit);
                        }
                    }
                }
                if item.is_none() {
                    item = completed(remote, &op)?;
                }
            }
            let item = item.ok_or(EngineError::RemoteChanged)?;
            remote.remember(&item);
            if directory {
                parents.insert(path.clone(), item.id.clone());
                report.created_remote_folders += 1;
            } else {
                // Confirm local installed evidence only if the original local bytes
                // still match the snapshot; divergent data remains untouched.
                let (p, n) = root.parent(&path)?;
                let mut file = filesystem::read(&p, &n)?.ok_or(EngineError::DownloadConflict)?;
                let identity = filesystem::identity(&file)?;
                if Some(identity) != op.source {
                    return Err(EngineError::DownloadConflict);
                }
                local::verify_file(&mut file, &op, cancel).map_err(|error| {
                    if matches!(error, EngineError::StateStorage) {
                        EngineError::DownloadConflict
                    } else {
                        error
                    }
                })?;
                let current = filesystem::read(&p, &n)?.ok_or(EngineError::DownloadConflict)?;
                if filesystem::identity(&current)? != identity {
                    return Err(EngineError::DownloadConflict);
                }
                root.validate_parent(&path, filesystem::identity(&p)?)?;
                journal.put_operation(
                    client,
                    &plan,
                    &Operation {
                        id: op.id.clone(),
                        item: item.clone(),
                        relative: path.clone(),
                        temporary: format!(".drivesync-{}.part", op.id),
                        root: filesystem::identity(&root.file)?,
                        parent: filesystem::identity(&p)?,
                        phase: Phase::Installed {
                            identity: Identity {
                                device: identity.device,
                                inode: identity.inode,
                            },
                            sha256: op.sha.clone(),
                        },
                    },
                )?;
                // A lost upload confirmation can look like an unowned local
                // destination to the preceding download pass. Exact ID, original
                // source identity and hashes now prove this specific conflict resolved.
                report.issues.retain(|issue| {
                    !(issue.relative_path.as_ref() == Some(&path)
                        && issue.error == EngineError::DownloadConflict)
                });
                report.uploaded += 1;
            }
            op.confirmed = true;
            uploads.save(&op)?;
            report
                .reconciliation
                .retain(|entry| entry.relative_path.as_ref() != Some(&path));
            if !directory {
                let _ = std::fs::remove_file(state_path.with_file_name(&op.snapshot));
            }
            Ok(())
        })();
        if let Err(error) = result {
            if matches!(
                error,
                EngineError::Authentication(crate::AuthError::Cancelled)
            ) {
                return Err(error);
            }
            if directory {
                blocked.insert(path.clone());
            }
            if matches!(
                error,
                EngineError::StateStorage
                    | EngineError::InvalidPlan
                    | EngineError::DownloadIo
                    | EngineError::RootSuspended(_)
            ) {
                return Err(error);
            }
            let stop = !matches!(
                error,
                EngineError::DownloadConflict
                    | EngineError::LocalWriting
                    | EngineError::LocalTemporary
                    | EngineError::ObservationUnknown
            );
            if !report
                .issues
                .iter()
                .any(|issue| issue.relative_path.as_ref() == Some(&path) && issue.error == error)
            {
                report.issues.push(DownloadIssue {
                    relative_path: Some(path),
                    error,
                });
            }
            if stop {
                break;
            }
        }
    }
    Ok(())
}
