//! Reconcile durable dispatch intent through current facts. Recovery never replays a command.
use super::*;
use crate::synchronization::SynchronizationItem;

pub(super) fn record(
    config: &OperatorConfig,
    r: &ExecuteOperationRequest,
    fs: &Filesystem,
    reserved_remote_id: Option<RemoteId>,
) -> Result<RecoveryRecord, OperatorError> {
    Ok(RecoveryRecord {
        account_id: config.account_id.clone(),
        provider_id: config.provider_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
        local_root: config.local_root.clone(),
        remote_root_id: config.remote_root_id.clone(),
        root_identity: fs.root_identity()?,
        parent_identity: r
            .operation
            .local_path
            .as_ref()
            .map(|path| fs.parent_identity(path))
            .transpose()?,
        destination_parent_identity: if matches!(
            r.operation.action,
            OperationAction::RenameLocal | OperationAction::MoveLocal
        ) {
            Some(fs.parent_identity(&Filesystem::movement_target(&r.operation)?)?)
        } else {
            None
        },
        command: r.operation.clone(),
        execution_id: r.execution_id.clone(),
        identity: r.identity.clone(),
        expected_revision: r.expected_revision,
        local: r.local.clone(),
        remote: r.remote.clone(),
        completed_steps: r.completed_steps.clone(),
        reserved_remote_id,
        stage_path: if matches!(
            r.operation.action,
            OperationAction::CreateLocalDirectory
                | OperationAction::Download
                | OperationAction::UpdateLocalContent
                | OperationAction::CreateLocalShortcut
        ) {
            Some(Filesystem::stage_path(r)?)
        } else {
            None
        },
        stage_identity: None,
        transfer: None,
        remote_ancestry: None,
        conflict_archive: None,
        local_trash: None,
    })
}
impl PersistingOperator {
    pub(super) async fn prepare(
        &self,
        r: &ExecuteOperationRequest,
    ) -> Result<RecoveryRecord, OperatorError> {
        if matches!(
            r.operation.action,
            OperationAction::CreateLocalDirectory
                | OperationAction::Download
                | OperationAction::CreateRemoteDirectory
                | OperationAction::CreateLocalShortcut
                | OperationAction::Upload
        ) {
            let remote_parents = if matches!(
                r.operation.action,
                OperationAction::Upload | OperationAction::CreateRemoteDirectory
            ) {
                r.operation.parent_id.iter().cloned().collect::<Vec<_>>()
            } else {
                match &r.remote.parents {
                    Evidence::Known(parents) => parents.clone(),
                    _ => return Err(filesystem::stale()),
                }
            };
            let Evidence::Known(local_parent) = &r.local.parent else {
                return Err(filesystem::stale());
            };
            self.confirmed_parent(local_parent, &remote_parents)
                .ok_or_else(filesystem::stale)?;
        }
        if matches!(
            r.operation.action,
            OperationAction::MoveLocal | OperationAction::MoveRemote
        ) {
            let (local, remote) = if r.operation.action == OperationAction::MoveLocal {
                (
                    r.operation.parent.clone().ok_or_else(filesystem::stale)?,
                    match &r.remote.parents {
                        Evidence::Known(ids) => ids.clone(),
                        _ => return Err(filesystem::stale()),
                    },
                )
            } else {
                (
                    match &r.local.parent {
                        Evidence::Known(p) => p.clone(),
                        _ => return Err(filesystem::stale()),
                    },
                    r.operation.parent_id.iter().cloned().collect(),
                )
            };
            self.confirmed_parent(&local, &remote)
                .ok_or_else(filesystem::stale)?;
        }
        let reserved = self.remote.reserve(&self.config, r).await?;
        record(&self.config, r, &self.filesystem, reserved)
    }
    pub(super) fn validate_recovery(&self, record: &RecoveryRecord) -> Result<(), RecoveryIssue> {
        if record.account_id != self.config.account_id
            || record.provider_id != self.config.provider_id
            || record.sync_root_id != self.config.sync_root_id
            || record.local_root != self.config.local_root
            || record.remote_root_id != self.config.remote_root_id
            || record.identity.account_id != record.account_id
            || record.identity.sync_root_id != record.sync_root_id
            || record.identity.item_id.as_ref() != Some(&record.command.item_ref)
            || record.command.remote_id != record.identity.remote_id
        {
            return Err(RecoveryIssue::ScopeMismatch);
        }
        if self.filesystem.validate_root().is_err()
            || self.filesystem.root_identity().ok().as_ref() != Some(&record.root_identity)
        {
            return Err(RecoveryIssue::RootChanged);
        }
        if let Some(path) = &record.command.local_path {
            if self.filesystem.parent_identity(path).ok() != record.parent_identity {
                return Err(RecoveryIssue::ParentChanged);
            }
        }
        if matches!(
            record.command.action,
            OperationAction::RenameLocal | OperationAction::MoveLocal
        ) && Filesystem::movement_target(&record.command)
            .ok()
            .and_then(|p| self.filesystem.parent_identity(&p).ok())
            != record.destination_parent_identity
        {
            return Err(RecoveryIssue::ParentChanged);
        }
        if !matches!(
            record.command.action,
            OperationAction::TrashRemote
                | OperationAction::RemoveLocal
                | OperationAction::CreateLocalDirectory
                | OperationAction::Download
                | OperationAction::CreateLocalShortcut
                | OperationAction::CreateRemoteDirectory
                | OperationAction::Upload
                | OperationAction::UpdateRemoteContent
                | OperationAction::UpdateLocalContent
                | OperationAction::RenameLocal
                | OperationAction::MoveLocal
                | OperationAction::RenameRemote
                | OperationAction::MoveRemote
        ) {
            return Err(RecoveryIssue::UnsupportedAction);
        }
        Ok(())
    }
    async fn recover_all(
        &self,
        checking: interfaces::RecoveryObserver<'_>,
    ) -> Result<Vec<RecoveryResult>, OperatorError> {
        let _guard = self.gate.lock().await;
        let _lease = self
            .inventory
            .execution_lease()
            .map_err(|_| OperatorError::new(OperatorErrorKind::Unavailable))?;
        let snapshots = self
            .inventory
            .enumerate(InventoryScopeQuery {
                account_id: self.config.account_id.clone(),
                sync_root_id: self.config.sync_root_id.clone(),
            })
            .map_err(|_| OperatorError::new(OperatorErrorKind::Unavailable))?;
        let mut results = Vec::new();
        for mut snapshot in snapshots {
            for pending in snapshot.pending_operations.clone().into_iter().filter(|p| {
                matches!(
                    p.state,
                    OperationState::Pending
                        | OperationState::Running
                        | OperationState::AppliedUnconfirmed
                )
            }) {
                let item = SynchronizationItem {
                    item_id: snapshot.identity.item_id.clone(),
                    local_path: snapshot.local_path.clone().or_else(|| {
                        pending
                            .recovery
                            .as_ref()
                            .and_then(|r| r.command.local_path.clone())
                    }),
                    remote_id: snapshot
                        .identity
                        .remote_id
                        .clone()
                        .or(pending.remote_id.clone()),
                };
                // Excluded editor artifacts retain their journal without replaying mutations.
                if item
                    .local_path
                    .as_deref()
                    .is_some_and(crate::projections::libreoffice_lock_path)
                {
                    continue;
                }
                checking(item.clone());
                let mut revision = Some(snapshot.revision);
                let mut assessment = match pending.recovery.as_ref() {
                    None => RecoveryAssessment::unknown(RecoveryIssue::MissingCommand),
                    Some(record)
                        if record.execution_id != pending.id
                            || record.command.action != pending.action
                            || record.command.item_ref != pending.item_id
                            || snapshot.identity.item_id.as_ref() != Some(&pending.item_id) =>
                    {
                        RecoveryAssessment::unknown(RecoveryIssue::ScopeMismatch)
                    }
                    Some(record) => match self.validate_recovery(record) {
                        Err(issue) => RecoveryAssessment::unknown(issue),
                        Ok(()) if pending.state == OperationState::Pending => RecoveryAssessment {
                            outcome: ExecutionEffect::NotApplied,
                            issue: Some(RecoveryIssue::NotDispatched),
                            effect: None,
                        },
                        Ok(()) => {
                            self.remote
                                .reconcile(&self.config, record, &self.filesystem)
                                .await
                        }
                    },
                };
                if assessment.issue == Some(RecoveryIssue::TransferIncomplete)
                    && (pending.state == OperationState::Running
                        || (pending.state == OperationState::AppliedUnconfirmed
                            && !pending
                                .applied_effect
                                .as_ref()
                                .is_some_and(|effect| effect.verified)))
                {
                    if let Some(record) = pending
                        .recovery
                        .as_ref()
                        .filter(|r| r.command.action == OperationAction::Upload)
                    {
                        let parent = match &record.local.parent {
                            Evidence::Known(parent) => self.confirmed_parent(
                                parent,
                                &record.command.parent_id.iter().cloned().collect::<Vec<_>>(),
                            ),
                            _ => None,
                        };
                        if parent.is_some() {
                            let request = ExecuteOperationRequest {
                                operation: record.command.clone(),
                                execution_id: record.execution_id.clone(),
                                expected_revision: Some(snapshot.revision),
                                identity: record.identity.clone(),
                                local: record.local.clone(),
                                remote: record.remote.clone(),
                                completed_steps: record.completed_steps.clone(),
                            };
                            let journal = Journal::new(
                                self.inventory.clone(),
                                request,
                                snapshot.revision,
                                (**record).clone(),
                            );
                            assessment = self
                                .remote
                                .resume_upload(&self.config, record, &self.filesystem, &journal)
                                .await;
                            revision = journal.revision();
                        } else {
                            assessment = RecoveryAssessment::unknown(RecoveryIssue::ParentChanged);
                        }
                    }
                }
                if assessment.outcome != ExecutionEffect::Unknown {
                    if let Some(record) = &pending.recovery {
                        if let Err(issue) = self.validate_recovery(record) {
                            assessment = RecoveryAssessment::unknown(issue);
                        }
                    }
                }
                let mut identity = snapshot.identity.clone();
                if let Some(effect) = &assessment.effect {
                    if let Some(local) = &effect.local {
                        identity.local_identity = local.identity.clone();
                    }
                    if effect.remote_id.is_some() {
                        identity.remote_id = effect.remote_id.clone();
                    }
                }
                let mut receipt = self.inventory.write(InventoryWrite {
                    operation_id: pending.id.clone(),
                    item_id: pending.item_id.clone(),
                    identity: identity.clone(),
                    expected_revision: revision,
                    effect: PersistedEffect::RecoveryObserved {
                        outcome: assessment.outcome,
                        issue: assessment.issue,
                        effect: assessment.effect.clone().map(Box::new),
                    },
                    recorded_at: now(),
                });
                if receipt.status == WriteStatus::Written
                    && assessment.outcome == ExecutionEffect::Applied
                {
                    let effect = assessment.effect.as_ref().ok_or_else(filesystem::stale)?;
                    let confirmed = match self.convergence(effect) {
                        Some((baseline, parent)) => {
                            PersistedEffect::BaselineConfirmed { baseline, parent }
                        }
                        None => PersistedEffect::OperationConfirmed,
                    };
                    receipt = self.inventory.write(InventoryWrite {
                        operation_id: pending.id.clone(),
                        item_id: pending.item_id.clone(),
                        identity,
                        expected_revision: receipt.revision,
                        effect: confirmed,
                        recorded_at: now(),
                    });
                }
                let changed = receipt.status == WriteStatus::Written;
                if let Some(revision) = receipt.revision {
                    snapshot.revision = revision;
                }
                results.push(RecoveryResult {
                    action: pending.action,
                    operation_id: pending.id,
                    item,
                    assessment: if changed {
                        assessment
                    } else {
                        RecoveryAssessment::unknown(RecoveryIssue::InventoryConflict)
                    },
                    inventory_write: Some(receipt),
                });
                // A concurrent writer invalidates this captured revision for any remaining records.
                if !changed {
                    break;
                }
            }
        }
        Ok(results)
    }
}
impl PersistingOperator {
    pub(super) fn recover_future<'a>(
        &'a self,
        checking: interfaces::RecoveryObserver<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<RecoveryResult>, OperatorError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(self.recover_all(checking))
    }
}
