//! Non-destructive remote conflict snapshots with independent durable replay.
use super::*;
use crate::download::{ConflictCopy, Preservation};
use std::io::{Seek, SeekFrom};

impl Session<'_> {
    /// Finishes a verified historical snapshot without applying it to the local
    /// origin or requiring the old remote revision to remain accessible.
    pub(super) fn recover_preservation(
        &self,
        mut record: Preservation,
    ) -> Result<ConflictCopy, EngineError> {
        let op = &record.operation;
        let Phase::Ready {
            identity: file_id,
            sha256,
        } = &op.phase
        else {
            return Err(EngineError::StateStorage);
        };
        let file_id = *file_id;
        let sha256 = sha256.clone();
        let size = op.item.size.ok_or(EngineError::StateStorage)?;
        let (parent, name) = self.root.parent(&op.relative)?;
        if op.root != identity(&self.root.file)? || op.parent != identity(&parent)? {
            return Err(EngineError::DownloadConflict);
        }
        self.root.validate_parent(&op.relative, op.parent)?;
        if !filesystem::exists(&parent, &name)? {
            if !self.intact(
                &parent,
                OsStr::new(&op.temporary),
                file_id,
                size,
                &sha256,
                self.context.cancel,
            )? {
                return Err(EngineError::DownloadIntegrity);
            }
            self.root.validate_parent(&op.relative, op.parent)?;
            stream::cancelled(self.context.cancel)?;
            filesystem::install(&parent, &op.temporary, &name)?;
        }
        if !self.intact(&parent, &name, file_id, size, &sha256, self.context.cancel)? {
            return Err(EngineError::DownloadConflict);
        }
        self.root.validate_parent(&op.relative, op.parent)?;
        parent.sync_all().map_err(filesystem::io)?;
        let copy = ConflictCopy {
            original_path: record.original.clone(),
            copy_path: op.relative.clone(),
            remote_id: op.item.id.clone(),
            remote_version: op.item.version.ok_or(EngineError::StateStorage)?,
        };
        record.operation.phase = Phase::Installed {
            identity: file_id,
            sha256,
        };
        self.context
            .store
            .put_preservation(self.context.client, self.context.scope, &record)?;
        Ok(copy)
    }

    /// Preserves only a revalidated same-path content divergence. The original is
    /// never renamed, written or used as a synchronization baseline here.
    pub(super) fn preserve_conflict(
        &self,
        item: &InventoryEntry,
        path: &Path,
        decision: &crate::PlanEntry,
        by_id: &HashMap<&str, &InventoryEntry>,
        source: &mut impl Source,
    ) -> Result<ConflictCopy, EngineError> {
        let expected = match &decision.expected_local {
            crate::LocalObservation::Verified(value) => value,
            _ => return Err(EngineError::ObservationUnknown),
        };
        let base = self
            .bases
            .get(&item.id)
            .ok_or(EngineError::DownloadConflict)?;
        if base
            .location
            .as_ref()
            .is_none_or(|l| l.relative_path != path)
            || item.version.is_none_or(|v| v <= base.remote_version)
            || crate::planner::fingerprint_parts(expected).is_none()
        {
            return Err(EngineError::DownloadConflict);
        }
        let (local_hash, local_dev, local_ino) =
            crate::planner::fingerprint_parts(expected).ok_or(EngineError::ObservationUnknown)?;
        let (base_hash, base_dev, base_ino) =
            crate::planner::fingerprint_parts(&base.local_fingerprint)
                .ok_or(EngineError::StateStorage)?;
        if (local_dev, local_ino) != (base_dev, base_ino)
            || local_hash == base_hash
            || base
                .location
                .as_ref()
                .is_none_or(|l| l.remote_name != item.name || l.remote_parent != item.parent_id)
        {
            return Err(EngineError::DownloadConflict);
        }
        let id = crate::state::preservation_key(item, path)?;
        let copy_path = path.with_file_name(format!("DriveSync backup conflict-{id}.bak"));
        let (parent, name) = self.root.parent(&copy_path)?;
        let root_id = identity(&self.root.file)?;
        let parent_id = identity(&parent)?;
        let size = item.size.ok_or(EngineError::InvalidPlan)?;
        let checksum = item
            .md5_checksum
            .as_deref()
            .filter(|h| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or(EngineError::InvalidPlan)?;
        if filesystem::fingerprint_root(self.root, path, self.context.cancel)?.as_ref()
            != Some(expected)
        {
            return Err(EngineError::LocalWriting);
        }
        let mut saved =
            self.context
                .store
                .preservation(self.context.client, self.context.scope, &id)?;
        if let Some(record) = &saved {
            let op = &record.operation;
            if op.item != *item
                || record.original != path
                || op.root != root_id
                || op.parent != parent_id
            {
                return Err(EngineError::DownloadConflict);
            }
            if let Phase::Ready {
                identity: file_id,
                sha256,
            }
            | Phase::Installed {
                identity: file_id,
                sha256,
            } = &op.phase
            {
                if filesystem::exists(&parent, &name)? {
                    if !self.intact(&parent, &name, *file_id, size, sha256, self.context.cancel)? {
                        return Err(EngineError::DownloadConflict);
                    }
                    self.root.validate_parent(path, parent_id)?;
                    parent.sync_all().map_err(filesystem::io)?;
                    let mut confirmed = record.clone();
                    confirmed.operation.phase = Phase::Installed {
                        identity: *file_id,
                        sha256: sha256.clone(),
                    };
                    self.context.store.put_preservation(
                        self.context.client,
                        self.context.scope,
                        &confirmed,
                    )?;
                    return Ok(ConflictCopy {
                        original_path: path.into(),
                        copy_path,
                        remote_id: item.id.clone(),
                        remote_version: item.version.unwrap(),
                    });
                }
                // A previously confirmed copy removed by another actor is never
                // silently recreated or substituted with an unrelated inode.
                if matches!(op.phase, Phase::Installed { .. }) {
                    return Err(EngineError::DownloadConflict);
                }
            }
        }
        if filesystem::exists(&parent, &name)? {
            return Err(EngineError::DownloadConflict);
        }
        source.configured()?;
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        if saved
            .as_ref()
            .is_none_or(|r| matches!(r.operation.phase, Phase::Started))
        {
            let token = oauth2::CsrfToken::new_random().secret().clone();
            let mut record = Preservation {
                original: path.into(),
                operation: Operation {
                    id,
                    item: item.clone(),
                    relative: copy_path.clone(),
                    temporary: format!(".drivesync-{token}.part"),
                    root: root_id,
                    parent: parent_id,
                    phase: Phase::Started,
                },
            };
            self.context.store.put_preservation(
                self.context.client,
                self.context.scope,
                &record,
            )?;
            let op = &mut record.operation;
            let mut staged = filesystem::create(&parent, &op.temporary, size)?;
            let mut input = source.media(&item.id)?;
            let hash = stream::copy(
                &mut input,
                &mut staged,
                size,
                checksum,
                self.context.cancel,
                |_| {},
                || {
                    self.root.validate()?;
                    source.check_configuration()
                },
            )?;
            if hash == local_hash || hash == base_hash {
                return Err(EngineError::DownloadConflict);
            }
            staged.sync_all().map_err(filesystem::io)?;
            parent.sync_all().map_err(filesystem::io)?;
            staged.seek(SeekFrom::Start(0)).map_err(filesystem::io)?;
            if !stream::verify(&mut staged, size, &hash, self.context.cancel)? {
                return Err(EngineError::DownloadIntegrity);
            }
            source.configured()?;
            validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
            if filesystem::fingerprint_root(self.root, path, self.context.cancel)?.as_ref()
                != Some(expected)
            {
                return Err(EngineError::LocalWriting);
            }
            self.root.validate_parent(path, parent_id)?;
            op.phase = Phase::Ready {
                identity: identity(&staged)?,
                sha256: hash,
            };
            // Persist verified bytes before publication; the next invocation can
            // recover this record even if rename succeeds but SQLite fails.
            self.context.store.put_preservation(
                self.context.client,
                self.context.scope,
                &record,
            )?;
            saved = Some(record);
        }
        let mut record = saved.ok_or(EngineError::StateStorage)?;
        let Phase::Ready {
            identity: file_id,
            sha256,
        } = &record.operation.phase
        else {
            return Err(EngineError::StateStorage);
        };
        let file_id = *file_id;
        let sha256 = sha256.clone();
        let op = &record.operation;
        if !self.intact(
            &parent,
            OsStr::new(&op.temporary),
            file_id,
            size,
            &sha256,
            self.context.cancel,
        )? {
            return Err(EngineError::DownloadIntegrity);
        }
        source.configured()?;
        validate_chain(source, item, by_id, &self.context.scope.remote_root)?;
        if filesystem::fingerprint_root(self.root, path, self.context.cancel)?.as_ref()
            != Some(expected)
        {
            return Err(EngineError::LocalWriting);
        }
        self.root.validate_parent(path, parent_id)?;
        stream::cancelled(self.context.cancel)?;
        filesystem::install(&parent, &op.temporary, &name)?;
        if !self.intact(&parent, &name, file_id, size, &sha256, self.context.cancel)? {
            return Err(EngineError::DownloadIntegrity);
        }
        self.root.validate_parent(path, parent_id)?;
        record.operation.phase = Phase::Installed {
            identity: file_id,
            sha256,
        };
        self.context
            .store
            .put_preservation(self.context.client, self.context.scope, &record)?;
        Ok(ConflictCopy {
            original_path: path.into(),
            copy_path,
            remote_id: item.id.clone(),
            remote_version: item.version.unwrap(),
        })
    }
}
