//! Managed updates preserve the prior entry before no-clobber installation.
use super::*;
use std::fs::File;

impl Session<'_> {
    fn intact(
        &self,
        parent: &File,
        name: &OsStr,
        expected: super::super::models::Identity,
        size: u64,
        sha: &str,
        cancel: &Cancellation,
    ) -> Result<bool, EngineError> {
        let Some(mut file) = filesystem::read(parent, name)? else {
            return Ok(false);
        };
        if identity(&file)? != expected || !stream::verify(&mut file, size, sha, cancel)? {
            return Ok(false);
        }
        file.sync_all().map_err(filesystem::io)?;
        Ok(true)
    }
    fn remember_backup(&self, op: &Operation, backup: &str) {
        *self.preserved.borrow_mut() = Some(op.relative.with_file_name(backup));
    }
    fn remember_if_prior(
        &self,
        parent: &File,
        op: &Operation,
        backup: &str,
        expected: super::super::models::Identity,
    ) {
        if let Ok(Some(file)) = filesystem::read(parent, OsStr::new(backup)) {
            if identity(&file).ok() == Some(expected) {
                self.remember_backup(op, backup);
            }
        }
    }
    /// Restores without replacing a newly created user entry. Failure retains backup.
    fn restore(&self, parent: &File, op: &Operation, backup: &str) -> Result<bool, EngineError> {
        let name = op.relative.file_name().ok_or(EngineError::InvalidPlan)?;
        if !filesystem::exists(parent, OsStr::new(backup))? {
            return Ok(false);
        }
        match filesystem::rename_entry(parent, OsStr::new(backup), name) {
            Ok(()) => {
                *self.preserved.borrow_mut() = None;
                parent.sync_all().map_err(filesystem::io)?;
                Ok(true)
            }
            Err(EngineError::DownloadConflict) => Ok(false),
            Err(e) => Err(e),
        }
    }
    /// A local commit finishes or restores once cutover starts; cancellation is
    /// checked before entering, not between moving the prior file and verifying it.
    pub(super) fn commit_update(&self, op: &Operation) -> Result<Outcome, EngineError> {
        let Phase::UpdateReady {
            identity: new_id,
            sha256,
            previous,
            backup,
        } = &op.phase
        else {
            return Err(EngineError::InvalidPlan);
        };
        let (parent, name) = self.root.parent(&op.relative)?;
        if op.root != identity(&self.root.file)? || op.parent != identity(&parent)? {
            return Err(EngineError::DownloadConflict);
        }
        self.root.validate_parent(&op.relative, op.parent)?;
        if !self.intact(
            &parent,
            OsStr::new(&op.temporary),
            *new_id,
            op.item.size.ok_or(EngineError::StateStorage)?,
            sha256,
            self.context.cancel,
        )? {
            return Err(EngineError::DownloadIntegrity);
        }
        let backup_exists = filesystem::exists(&parent, OsStr::new(backup))?;
        if backup_exists {
            if !self.intact(
                &parent,
                OsStr::new(backup),
                previous.identity,
                previous.size,
                &previous.sha256,
                self.context.cancel,
            )? {
                return Err(EngineError::DownloadConflict);
            }
            self.remember_backup(op, backup);
            if filesystem::exists(&parent, &name)? {
                return Err(EngineError::DownloadConflict);
            }
        } else {
            if !self.intact(
                &parent,
                &name,
                previous.identity,
                previous.size,
                &previous.sha256,
                self.context.cancel,
            )? {
                return Err(EngineError::DownloadConflict);
            }
            stream::cancelled(self.context.cancel)?;
            filesystem::rename_entry(&parent, &name, OsStr::new(backup))?;
            self.remember_backup(op, backup);
            if let Err(error) = parent.sync_all().map_err(filesystem::io) {
                self.restore(&parent, op, backup)?;
                return Err(error);
            }
        }
        // Verify the entry actually moved, detecting a target swap after preflight.
        let commit_cancel = Cancellation::new();
        match self.intact(
            &parent,
            OsStr::new(backup),
            previous.identity,
            previous.size,
            &previous.sha256,
            &commit_cancel,
        ) {
            Ok(true) => {}
            result => {
                self.restore(&parent, op, backup)?;
                return Err(result.err().unwrap_or(EngineError::DownloadConflict));
            }
        }
        if let Err(error) = self
            .root
            .validate_parent(&op.relative, op.parent)
            .and_then(|()| filesystem::install(&parent, &op.temporary, &name))
        {
            self.restore(&parent, op, backup)?;
            return Err(error);
        }
        self.confirm_update(op)?;
        Ok(Outcome::Updated)
    }
    fn confirm_update(&self, op: &Operation) -> Result<(), EngineError> {
        let Phase::UpdateReady {
            identity,
            sha256,
            previous,
            backup,
        } = &op.phase
        else {
            return Err(EngineError::InvalidPlan);
        };
        let mut confirmed = op.clone();
        confirmed.phase = Phase::Updated {
            identity: *identity,
            sha256: sha256.clone(),
            previous: previous.clone(),
            backup: backup.clone(),
        };
        self.put(&confirmed)
    }
    /// A removed/renamed/unsupported remote item cannot strand a displaced local
    /// version. Reconcile its verified effect or restore without overwriting.
    pub(super) fn retire_update(&self, op: &Operation) -> Result<(), EngineError> {
        let Phase::UpdateReady {
            identity: new_id,
            sha256,
            previous,
            backup,
        } = &op.phase
        else {
            return Err(EngineError::InvalidPlan);
        };
        let (parent, name) = self.root.parent(&op.relative)?;
        if op.root != identity(&self.root.file)? || op.parent != identity(&parent)? {
            return Err(EngineError::DownloadConflict);
        }
        self.root.validate_parent(&op.relative, op.parent)?;
        let saved = filesystem::exists(&parent, OsStr::new(backup))?;
        if saved {
            self.remember_if_prior(&parent, op, backup, previous.identity);
        }
        if self.intact(
            &parent,
            &name,
            *new_id,
            op.item.size.ok_or(EngineError::StateStorage)?,
            sha256,
            self.context.cancel,
        )? {
            if !saved
                || !self.intact(
                    &parent,
                    OsStr::new(backup),
                    previous.identity,
                    previous.size,
                    &previous.sha256,
                    self.context.cancel,
                )?
            {
                return Err(EngineError::DownloadConflict);
            }
            parent.sync_all().map_err(filesystem::io)?;
            return self.confirm_update(op);
        }
        if saved {
            if filesystem::exists(&parent, &name)?
                || !self.intact(
                    &parent,
                    OsStr::new(backup),
                    previous.identity,
                    previous.size,
                    &previous.sha256,
                    self.context.cancel,
                )?
            {
                return Err(EngineError::DownloadConflict);
            }
            self.restore(&parent, op, backup)?;
        }
        if !self.intact(
            &parent,
            &name,
            previous.identity,
            previous.size,
            &previous.sha256,
            self.context.cancel,
        )? {
            return Err(EngineError::DownloadConflict);
        }
        let mut abandoned = op.clone();
        abandoned.phase = Phase::UpdateAbandoned {
            backup: backup.clone(),
        };
        self.put(&abandoned)
    }
    /// Reconciles both sides of an interrupted cutover before allowing new work.
    pub(super) fn resume_update(
        &self,
        op: &Operation,
        item: &InventoryEntry,
        by_id: &HashMap<&str, &InventoryEntry>,
        source: &mut impl Source,
        progress: &mut impl FnMut(DownloadItemProgress),
    ) -> Result<Outcome, EngineError> {
        let Phase::UpdateReady {
            identity: new_id,
            sha256,
            previous,
            backup,
        } = &op.phase
        else {
            return Err(EngineError::InvalidPlan);
        };
        let (parent, name) = self.root.parent(&op.relative)?;
        if op.root != identity(&self.root.file)? || op.parent != identity(&parent)? {
            return Err(EngineError::DownloadConflict);
        }
        self.root.validate_parent(&op.relative, op.parent)?;
        let backup_exists = filesystem::exists(&parent, OsStr::new(backup))?;
        if backup_exists {
            self.remember_if_prior(&parent, op, backup, previous.identity);
        }
        if self.intact(
            &parent,
            &name,
            *new_id,
            op.item.size.ok_or(EngineError::StateStorage)?,
            sha256,
            self.context.cancel,
        )? {
            if !backup_exists
                || !self.intact(
                    &parent,
                    OsStr::new(backup),
                    previous.identity,
                    previous.size,
                    &previous.sha256,
                    self.context.cancel,
                )?
            {
                return Err(EngineError::DownloadConflict);
            }
            parent.sync_all().map_err(filesystem::io)?;
            self.confirm_update(op)?;
            if op.item != *item {
                return Err(EngineError::RemoteChanged);
            }
            return Ok(Outcome::Recovered);
        }
        let validation = if op.item != *item {
            Err(EngineError::RemoteChanged)
        } else {
            source
                .configured()
                .and_then(|()| validate_chain(source, item, by_id, &self.context.scope.remote_root))
        };
        if let Err(error) = validation {
            if backup_exists && !filesystem::exists(&parent, &name)? {
                if !self.intact(
                    &parent,
                    OsStr::new(backup),
                    previous.identity,
                    previous.size,
                    &previous.sha256,
                    self.context.cancel,
                )? {
                    return Err(EngineError::DownloadConflict);
                }
                self.restore(&parent, op, backup)?;
            }
            if error == EngineError::RemoteChanged
                && self.intact(
                    &parent,
                    &name,
                    previous.identity,
                    previous.size,
                    &previous.sha256,
                    self.context.cancel,
                )?
                && !filesystem::exists(&parent, OsStr::new(backup))?
            {
                let mut abandoned = op.clone();
                abandoned.phase = Phase::UpdateAbandoned {
                    backup: backup.clone(),
                };
                self.put(&abandoned)?;
            }
            return Err(error);
        }
        progress(DownloadItemProgress {
            relative_path: op.relative.clone(),
            stage: DownloadStage::Installing,
            received: op.item.size.unwrap_or(0),
            total: op.item.size,
        });
        self.commit_update(op)?;
        Ok(Outcome::Recovered)
    }
}
