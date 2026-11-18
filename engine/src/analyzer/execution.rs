//! The Analyzer fulfills guarded plans and calls Operator with durable ownership.
//! No Provider, SQL, local traversal, name-based association or conflict resolution.
use super::*;
use crate::{inventory::*, models::*, operations::*};
use std::os::unix::ffi::OsStringExt;

fn notify(
    e: &AnalyzerExecution,
    item: &SyncItem,
    command: Option<OperationCommand>,
    op: Option<OperationId>,
    stage: CreationStage,
    error: Option<OperatorError>,
) {
    e.observer.notify(CreationEvent {
        item_id: item.id.clone(),
        command,
        operation_id: op,
        stage,
        error,
    });
}
fn source(item: &SyncItem) -> Option<SourceEvidence> {
    if item.baseline.is_some() {
        return None;
    }
    if item.local_presence == PresenceState::Present
        && item.remote_ref.is_none()
        && item.remote.is_none()
    {
        Some(SourceEvidence::Local {
            metadata: Box::new(item.local_latest.as_ref().or(item.local.as_ref())?.clone()),
            sha256: None,
        })
    } else if item.remote_presence == PresenceState::Present
        && item.local.is_none()
        && item.local_file_id.is_none()
    {
        Some(SourceEvidence::Remote {
            metadata: Box::new(
                item.remote_latest
                    .as_ref()
                    .or(item.remote.as_ref())?
                    .clone(),
            ),
            revision: None,
        })
    } else {
        None
    }
}
fn new_id() -> Option<ExecutionId> {
    use std::io::Read;
    let mut bytes = [0; 24];
    std::fs::File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    ExecutionId::new(format!(
        "create-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
    .ok()
}
/// Existing resources require a complete current plan and strict source proof.
/// Initial creation probes may discharge missing-replica identity/evidence
/// requirements; remote-to-local trash requires a confirmed absence and baseline.
pub(super) async fn verify_candidate(
    e: &AnalyzerExecution,
    item: &SyncItem,
    a: &mut NewAnalysisAssessment,
) -> Option<(ExecutionContext, OperationCommand)> {
    let plan = a.plan.as_ref()?;
    if item.baseline.is_some() {
        if !plan.manual.is_empty()
            || plan.steps.is_empty()
            || a.intents.is_empty()
            || !a.requirements.is_empty()
            || !plan.steps[0].blocked_by.is_empty()
            || !plan.steps[0].depends_on.is_empty()
        {
            return None;
        }
        let command = plan.steps[0].command.clone()?;
        let evidence = match &command {
            OperationCommand::Rename { target, .. } | OperationCommand::Move { target, .. } => {
                match target {
                    ResourceRef::Local { .. } => SourceEvidence::Local {
                        metadata: Box::new(
                            item.local_latest.as_ref().or(item.local.as_ref())?.clone(),
                        ),
                        sha256: None,
                    },
                    ResourceRef::Remote { .. } => SourceEvidence::Remote {
                        metadata: Box::new(
                            item.remote_latest
                                .as_ref()
                                .or(item.remote.as_ref())?
                                .clone(),
                        ),
                        revision: None,
                    },
                }
            }
            OperationCommand::UpdateContent { source, .. } => match source {
                ContentSource::LocalFile { .. } => SourceEvidence::Local {
                    metadata: Box::new(item.local_latest.as_ref().or(item.local.as_ref())?.clone()),
                    sha256: None,
                },
                ContentSource::RemoteFile { .. } => SourceEvidence::Remote {
                    metadata: Box::new(
                        item.remote_latest
                            .as_ref()
                            .or(item.remote.as_ref())?
                            .clone(),
                    ),
                    revision: None,
                },
                _ => return None,
            },
            OperationCommand::Trash {
                target:
                    ResourceRef::Local {
                        relative_path,
                        local_id,
                    },
                ..
            } if item.local_presence == PresenceState::Present
                && item.kind == ItemKind::File
                && item.remote_presence == PresenceState::AbsentConfirmed
                && item.baseline.as_ref().is_some_and(|baseline| {
                    baseline.local.presence == PresenceState::Present
                        && baseline.remote.presence == PresenceState::Present
                })
                && item
                    .local_latest
                    .as_ref()
                    .or(item.local.as_ref())
                    .is_some_and(|metadata| {
                        metadata.relative_path == *relative_path && metadata.local_id == *local_id
                    })
                && a.intents.first().is_some_and(|intent| {
                    intent.kind == OperationKind::Trash
                        && intent.direction == OperationDirection::Download
                }) =>
            {
                SourceEvidence::Local {
                    metadata: Box::new(item.local_latest.as_ref().or(item.local.as_ref())?.clone()),
                    sha256: None,
                }
            }
            _ => return None,
        };
        notify(
            e,
            item,
            Some(command.clone()),
            None,
            CreationStage::Checking,
            None,
        );
        if let Err(error) = e
            .operator
            .verify_source(VerifySourceRequest {
                source: evidence.clone(),
                allow_version_advance: false,
                options: e.options.clone(),
            })
            .await
        {
            notify(
                e,
                item,
                Some(command),
                None,
                CreationStage::Blocked,
                Some(error.kind),
            );
            return None;
        }
        return Some((
            ExecutionContext {
                id: new_id()?,
                source: Some(evidence),
            },
            command,
        ));
    }
    let source = source(item)?;
    if !plan.manual.is_empty()
        || plan.steps.len() != 1
        || a.intents.len() != 1
        || a.intents[0].kind != OperationKind::Create
    {
        return None;
    }
    let step = &plan.steps[0];
    let command = step.command.clone()?;
    if !matches!(
        command,
        OperationCommand::Upload { .. }
            | OperationCommand::Download { .. }
            | OperationCommand::CreateDirectory { .. }
    ) || step
        .blocked_by
        .iter()
        .any(|b| !matches!(b, PlanBlock::Identity | PlanBlock::Evidence))
        || !step.depends_on.is_empty()
    {
        return None;
    }
    let allowed = match source {
        SourceEvidence::Local { .. } => AnalysisRequirement::ObserveRemote,
        SourceEvidence::Remote { .. } => AnalysisRequirement::ObserveLocal,
    };
    if a.requirements
        .iter()
        .any(|r| *r != allowed && *r != AnalysisRequirement::VerifyIdentity)
    {
        return None;
    }
    let context = ExecutionContext {
        id: new_id()?,
        source: Some(source),
    };
    notify(
        e,
        item,
        Some(command.clone()),
        None,
        CreationStage::Checking,
        None,
    );
    match e
        .operator
        .verify_creation(VerifyCreationRequest {
            context: context.clone(),
            command: command.clone(),
            options: e.options.clone(),
        })
        .await
    {
        Ok(verified) if verified.command == command && verified.source.is_some() => {
            // A single initial replica and a verified free destination satisfy
            // the previously unknown creation identity; no absence is persisted.
            for finding in &mut a.findings {
                if finding.status == AnalysisFindingStatus::Unknown {
                    finding.status = AnalysisFindingStatus::Compatible;
                    finding.reason = AnalysisReason::NewItem;
                }
            }
            a.requirements.clear();
            for step in &mut a.plan.as_mut()?.steps {
                step.blocked_by.clear();
            }
            a.disposition = AnalysisDisposition::PendingOperations;
            Some((context, command))
        }
        Ok(_) => {
            notify(
                e,
                item,
                Some(command),
                None,
                CreationStage::Blocked,
                Some(OperatorError::InvalidRequest),
            );
            None
        }
        Err(error) => {
            notify(
                e,
                item,
                Some(command),
                None,
                CreationStage::Blocked,
                Some(error.kind),
            );
            None
        }
    }
}
pub(super) async fn start(
    inventory: &InventoryManager,
    e: &AnalyzerExecution,
    analysis: &RecordedAnalysis,
    item: &SyncItem,
    candidate: (ExecutionContext, OperationCommand),
    at: Timestamp,
    guard: &Reservation,
) -> AnalyzerResult<()> {
    let info = inventory
        .get_sync_info(GetSyncInfoRequest {})
        .map_err(inventory_error)?;
    let intent = ExecutionIntent {
        context: candidate.0,
        command: candidate.1,
        scope: ExecutionScope {
            sync_id: info.info.sync_id,
            account_id: info.info.account_id,
            remote_root_id: RemoteFileId(info.info.remote_root_id),
            local_root: info.local_root,
        },
    };
    let current = inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(item.id.clone()),
        })
        .map_err(inventory_error)?
        .item
        .ok_or_else(|| error(AnalyzerErrorKind::InvalidRequest))?;
    let binding = inventory
        .begin_creation(BeginCreationRequest {
            analysis_id: analysis.id.clone(),
            expected_item_revision: current.revision,
            intent,
            started_at: at,
        })
        .map_err(inventory_error)?;
    run(inventory, e, item, binding, at, guard).await
}
pub(super) async fn resume(
    inventory: &InventoryManager,
    e: &AnalyzerExecution,
    item: &SyncItem,
    binding: CreationBinding,
    at: Timestamp,
    guard: &Reservation,
) -> AnalyzerResult<()> {
    let completed = inventory
        .get_execution(GetExecutionRequest {
            id: binding.intent.context.id.clone(),
        })
        .map_err(inventory_error)?
        .state
        .phase
        == CheckpointPhase::Completed;
    if binding
        .operation
        .next_attempt_at
        .is_some_and(|t| t.unix_millis > at.unix_millis)
        || binding.operation.attempt_count >= 5
        || (binding.operation.state == OperationState::Blocked && !completed)
    {
        return Ok(());
    }
    let analysis = inventory
        .get_analysis(GetAnalysisRequest {
            selector: AnalysisSelector::LatestForItem(item.id.clone()),
        })
        .map_err(inventory_error)?
        .analysis
        .ok_or_else(|| error(AnalyzerErrorKind::InvalidRequest))?;
    let claimed = inventory.begin_creation(BeginCreationRequest {
        analysis_id: analysis.id,
        expected_item_revision: item.revision,
        intent: binding.intent.clone(),
        started_at: at,
    });
    let binding = match claimed {
        Ok(claimed) => claimed,
        Err(error) if error.kind == InventoryError::VersionMismatch => {
            inventory
                .fail_creation(FailCreationRequest {
                    binding: binding.clone(),
                    error: OperatorError::PreconditionFailed,
                    at,
                })
                .map_err(inventory_error)?;
            notify(
                e,
                item,
                Some(binding.intent.command),
                Some(binding.operation.id),
                CreationStage::Blocked,
                Some(OperatorError::PreconditionFailed),
            );
            return Ok(());
        }
        Err(error) => return Err(inventory_error(error)),
    };
    run(inventory, e, item, binding, at, guard).await
}
async fn run(
    inventory: &InventoryManager,
    e: &AnalyzerExecution,
    item: &SyncItem,
    binding: CreationBinding,
    at: Timestamp,
    guard: &Reservation,
) -> AnalyzerResult<()> {
    guard.phase(AnalysisPhase::Executing);
    notify(
        e,
        item,
        Some(binding.intent.command.clone()),
        Some(binding.operation.id.clone()),
        CreationStage::Executing,
        None,
    );
    let work = execute(e, &binding).await;
    let result = match work {
        Ok(result) => result,
        Err(error) => {
            inventory
                .fail_creation(FailCreationRequest {
                    binding: binding.clone(),
                    error: error.kind,
                    at,
                })
                .map_err(inventory_error)?;
            notify(
                e,
                item,
                Some(binding.intent.command),
                Some(binding.operation.id),
                CreationStage::Failed,
                Some(error.kind),
            );
            return Ok(());
        }
    };
    if matches!(
        &binding.intent.command,
        OperationCommand::Trash {
            target: ResourceRef::Local { .. },
            ..
        }
    ) {
        guard.phase(AnalysisPhase::Verifying);
        notify(
            e,
            item,
            Some(binding.intent.command.clone()),
            Some(binding.operation.id.clone()),
            CreationStage::Verifying,
            None,
        );
        let causal_order = inventory.reserve_order().map_err(inventory_error)?;
        inventory
            .record_trashed_local(RecordTrashedLocalRequest {
                binding: binding.clone(),
                result,
                trashed_at: now(),
                causal_order,
            })
            .map_err(inventory_error)?;
        notify(
            e,
            item,
            Some(binding.intent.command.clone()),
            Some(binding.operation.id),
            CreationStage::Completed,
            None,
        );
        return Ok(());
    }
    let causal_order = inventory.reserve_order().map_err(inventory_error)?;
    let current = inventory
        .record_created_replica(RecordCreatedReplicaRequest {
            binding: binding.clone(),
            result: result.clone(),
            causal_order,
        })
        .map_err(inventory_error)?;
    guard.phase(AnalysisPhase::Verifying);
    notify(
        e,
        item,
        Some(binding.intent.command.clone()),
        Some(binding.operation.id.clone()),
        CreationStage::Verifying,
        None,
    );
    let verification = verify_both(inventory, e, &binding, &current, &result).await;
    match verification {
        Ok(confirmed) => {
            let verified_at = confirmed.verified_at;
            let b = &confirmed.baseline;
            let mapped_parent = match &b.local.parent_identity {
                None => Some(binding.intent.scope.remote_root_id.clone()),
                Some(BaselineParent::Local(id)) => inventory
                    .get_item(GetItemRequest {
                        selector: ItemSelector::Id(id.clone()),
                    })
                    .map_err(inventory_error)?
                    .item
                    .and_then(|parent| parent.remote_ref.map(|r| r.file_id)),
                _ => None,
            };
            let remote_parent = match &b.remote.parent_identity {
                Some(BaselineParent::Remote(r)) => Some(r.file_id.clone()),
                _ => None,
            };
            let aligned = b.local.local_name_bytes.as_deref()
                == b.remote.name.as_ref().map(|n| n.as_bytes())
                && mapped_parent.is_some()
                && mapped_parent == remote_parent;
            if !aligned {
                let analysis = inventory
                    .get_analysis(GetAnalysisRequest {
                        selector: AnalysisSelector::LatestForItem(current.id.clone()),
                    })
                    .map_err(inventory_error)?
                    .analysis;
                if analysis
                    .and_then(|a| a.plan)
                    .is_some_and(|p| p.steps.len() > 1)
                {
                    inventory
                        .complete_execution_step(CompleteExecutionStepRequest {
                            binding: binding.clone(),
                            at: verified_at,
                        })
                        .map_err(inventory_error)?;
                    notify(
                        e,
                        item,
                        Some(binding.intent.command),
                        Some(binding.operation.id),
                        CreationStage::Completed,
                        None,
                    );
                    return Ok(());
                }
                inventory
                    .fail_creation(FailCreationRequest {
                        binding: binding.clone(),
                        error: OperatorError::PreconditionFailed,
                        at: verified_at,
                    })
                    .map_err(inventory_error)?;
                notify(
                    e,
                    item,
                    Some(binding.intent.command),
                    Some(binding.operation.id),
                    CreationStage::Blocked,
                    Some(OperatorError::PreconditionFailed),
                );
                return Ok(());
            }
            inventory
                .confirm_sync(ConfirmSyncRequest {
                    item_id: current.id.clone(),
                    expected_item_revision: current.revision,
                    confirmed,
                    completed_operation: Some(binding.operation.id.clone()),
                    expected_operation_attempt: Some(binding.operation.attempt_count),
                    activity: Some(NewActivity {
                        item_id: current.id.clone(),
                        operation_id: Some(binding.operation.id.clone()),
                        event_type: if binding.operation.kind == OperationKind::Rename {
                            ActivityType::Rename
                        } else if binding.operation.kind == OperationKind::Move {
                            ActivityType::Move
                        } else if binding.operation.direction == OperationDirection::Upload {
                            ActivityType::Upload
                        } else {
                            ActivityType::Download
                        },
                        origin: ActivityOrigin::System,
                        outcome: ActivityOutcome::Completed,
                        occurred_at: verified_at,
                        description: Some("Operation result verified by Operator".into()),
                        error_kind: None,
                        item_name: None,
                        item_remote_id: current.remote_ref.as_ref().map(|r| r.file_id.0.clone()),
                        item_local_path: current
                            .local
                            .as_ref()
                            .map(|l| l.relative_path.as_bytes().to_vec()),
                    }),
                })
                .map_err(inventory_error)?;
            notify(
                e,
                item,
                Some(binding.intent.command),
                Some(binding.operation.id),
                CreationStage::Completed,
                None,
            );
        }
        Err(error) => {
            inventory
                .fail_creation(FailCreationRequest {
                    binding: binding.clone(),
                    error: error.kind,
                    at: now(),
                })
                .map_err(inventory_error)?;
            notify(
                e,
                item,
                Some(binding.intent.command),
                Some(binding.operation.id),
                CreationStage::Failed,
                Some(error.kind),
            );
        }
    }
    Ok(())
}
async fn execute(e: &AnalyzerExecution, binding: &CreationBinding) -> OperatorResult<ItemResult> {
    let context = binding.intent.context.clone();
    let options = e.options.clone();
    match binding.intent.command.clone() {
        OperationCommand::Upload {
            source,
            destination,
            name,
        } => e
            .operator
            .upload(UploadRequest {
                execution: context,
                source,
                destination,
                name,
                options,
            })
            .await
            .map(|r| r.item),
        OperationCommand::Download {
            source,
            destination,
            name,
        } => e
            .operator
            .download(DownloadRequest {
                execution: context,
                source,
                destination,
                name,
                options,
            })
            .await
            .map(|r| r.item),
        OperationCommand::CreateDirectory { parent, name } => {
            e.operator
                .create_directory(CreateDirectoryRequest {
                    execution: context,
                    parent,
                    name,
                    options,
                })
                .await
        }
        OperationCommand::Rename {
            target,
            new_name,
            preconditions,
        } => {
            e.operator
                .rename(RenameRequest {
                    execution: context,
                    target,
                    new_name,
                    preconditions,
                    options,
                })
                .await
        }
        OperationCommand::Move {
            target,
            destination,
            preconditions,
        } => {
            e.operator
                .move_item(MoveRequest {
                    execution: context,
                    target,
                    destination,
                    preconditions,
                    options,
                })
                .await
        }
        OperationCommand::Trash {
            target,
            preconditions,
        } => {
            e.operator
                .trash(TrashRequest {
                    execution: context,
                    target,
                    preconditions,
                    options,
                })
                .await
        }
        OperationCommand::UpdateContent {
            target,
            source,
            preconditions,
        } => e
            .operator
            .update_content(UpdateContentRequest {
                execution: context,
                target,
                source,
                preconditions,
                options,
            })
            .await
            .map(|r| r.item),
        _ => Err(OperatorFailure {
            kind: OperatorError::UnsupportedOperation,
            effect: ExecutionEffect::NotApplied,
            resource: None,
        }),
    }
}
fn now() -> Timestamp {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0);
    Timestamp { unix_millis: n }
}
async fn verify_both(
    inventory: &InventoryManager,
    e: &AnalyzerExecution,
    binding: &CreationBinding,
    item: &SyncItem,
    result: &ItemResult,
) -> OperatorResult<ConfirmedState> {
    let failure = || OperatorFailure {
        kind: OperatorError::PreconditionFailed,
        effect: ExecutionEffect::Applied,
        resource: Some(result.resource.clone()),
    };
    let source = if matches!(
        binding.intent.command,
        OperationCommand::Rename { .. } | OperationCommand::Move { .. }
    ) {
        match result.resource {
            ResourceRef::Remote { .. } => SourceEvidence::Local {
                metadata: Box::new(
                    item.local_latest
                        .as_ref()
                        .or(item.local.as_ref())
                        .ok_or_else(failure)?
                        .clone(),
                ),
                sha256: None,
            },
            ResourceRef::Local { .. } => SourceEvidence::Remote {
                metadata: Box::new(
                    item.remote_latest
                        .as_ref()
                        .or(item.remote.as_ref())
                        .ok_or_else(failure)?
                        .clone(),
                ),
                revision: None,
            },
        }
    } else {
        binding.intent.context.source.clone().ok_or_else(failure)?
    };
    let source_order = inventory.reserve_order().map_err(|_| failure())?;
    let source = e
        .operator
        .verify_source(VerifySourceRequest {
            source,
            allow_version_advance: false,
            options: e.options.clone(),
        })
        .await?;
    let evidence = match result.metadata.as_ref().ok_or_else(failure)? {
        FileData::Local(local) => SourceEvidence::Local {
            metadata: Box::new(local.clone()),
            sha256: None,
        },
        FileData::Remote(remote) => SourceEvidence::Remote {
            metadata: Box::new(remote.clone()),
            revision: None,
        },
    };
    let replica_order = inventory.reserve_order().map_err(|_| failure())?;
    let replica = e
        .operator
        .verify_source(VerifySourceRequest {
            source: evidence,
            allow_version_advance: true,
            options: e.options.clone(),
        })
        .await?;
    let (local, remote, hash, local_order, remote_order) =
        match (&source.metadata, &replica.metadata) {
            (FileData::Local(l), FileData::Remote(r)) => {
                (l, r, source.sha256.as_ref(), source_order, replica_order)
            }
            (FileData::Remote(r), FileData::Local(l)) => {
                (l, r, replica.sha256.as_ref(), replica_order, source_order)
            }
            _ => return Err(failure()),
        };
    if item.kind == ItemKind::File {
        let checkpoint = inventory
            .get_execution(GetExecutionRequest {
                id: binding.intent.context.id.clone(),
            })
            .map_err(|_| failure())?;
        let expected = checkpoint.state.staging.as_ref().map(|s| &s.sha256);
        let metadata_mutation = matches!(
            binding.intent.command,
            OperationCommand::Rename { .. } | OperationCommand::Move { .. }
        );
        if (!metadata_mutation && hash != expected)
            || (metadata_mutation
                && hash.map(|h| MetadataValue::Known(h.as_str().to_owned()))
                    != Some(remote.sha256_checksum.clone()))
        {
            return Err(failure());
        }
    }
    let checksum = hash.map(|h| format!("sha256:{}", h.as_str()));
    let bytes = local.relative_path.as_bytes();
    let slash = bytes.iter().rposition(|b| *b == b'/');
    let local_parent = if let Some(slash) = slash {
        inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::LocalRelativePath(
                    LocalRelativePath::new(bytes[..slash].to_vec()).map_err(|_| failure())?,
                ),
            })
            .map_err(|_| failure())?
            .item
            .map(|p| BaselineParent::Local(p.id))
    } else {
        None
    };
    let name = match &remote.name {
        MetadataValue::Known(n) => Some(n.clone()),
        _ => None,
    };
    let remote_parent = match &remote.parent_ids {
        MetadataValue::Known(ids) if ids.len() == 1 => {
            Some(BaselineParent::Remote(RemoteFileRef {
                account_id: remote.reference.account_id.clone(),
                file_id: ids[0].clone(),
            }))
        }
        _ => None,
    };
    let size = if item.kind == ItemKind::File {
        match local.storage.size_bytes {
            MetadataValue::Known(n) => Some(n),
            _ => return Err(failure()),
        }
    } else {
        None
    };
    let verified_at = now();
    Ok(ConfirmedState {
        status: SyncStatus::Synced,
        verified_at,
        local_causal_order: local_order,
        remote_causal_order: remote_order,
        baseline: SyncBaseline {
            confirmed_at: verified_at,
            local: BaselineSide {
                presence: PresenceState::Present,
                name: std::ffi::OsString::from_vec(
                    bytes[slash.map(|n| n + 1).unwrap_or(0)..].to_vec(),
                )
                .into_string()
                .ok(),
                local_name_bytes: Some(bytes[slash.map(|n| n + 1).unwrap_or(0)..].to_vec()),
                parent_identity: local_parent,
                version: None,
                checksum: checksum.clone(),
                size,
                modified_at: None,
            },
            remote: BaselineSide {
                presence: PresenceState::Present,
                name,
                local_name_bytes: None,
                parent_identity: remote_parent,
                version: match &remote.version {
                    MetadataValue::Known(v) => Some(v.clone()),
                    _ => None,
                },
                checksum,
                size,
                modified_at: None,
            },
        },
    })
}

/// A new observation can invalidate the display state without invalidating the
/// original creation. Recheck both current replicas instead of executing again.
pub(super) async fn reconfirm(
    inventory: &InventoryManager,
    e: &AnalyzerExecution,
    item: &SyncItem,
    guard: &Reservation,
) -> AnalyzerResult<()> {
    if item.baseline.is_none() || item.status == SyncStatus::Synced {
        return Ok(());
    }
    let Some(mut binding) = inventory
        .get_creation(GetCreationRequest {
            item_id: item.id.clone(),
        })
        .map_err(inventory_error)?
    else {
        return Ok(());
    };
    if binding.operation.state != OperationState::Completed {
        return Ok(());
    }
    let local = item.local_latest.as_ref().or(item.local.as_ref());
    let remote = item.remote_latest.as_ref().or(item.remote.as_ref());
    let (Some(local), Some(remote)) = (local, remote) else {
        return Ok(());
    };
    let upload = binding.operation.direction == OperationDirection::Upload;
    binding.intent.context.source = Some(if upload {
        SourceEvidence::Local {
            metadata: Box::new(local.clone()),
            sha256: None,
        }
    } else {
        SourceEvidence::Remote {
            metadata: Box::new(remote.clone()),
            revision: None,
        }
    });
    let result = ItemResult {
        resource: if upload {
            ResourceRef::Remote {
                provider_id: remote.provider.clone(),
                reference: remote.reference.clone(),
            }
        } else {
            ResourceRef::Local {
                relative_path: local.relative_path.clone(),
                local_id: local.local_id.clone(),
            }
        },
        metadata: Some(if upload {
            FileData::Remote(remote.clone())
        } else {
            FileData::Local(local.clone())
        }),
        verification: VerificationStatus::Verified,
    };
    guard.phase(AnalysisPhase::Verifying);
    match verify_both(inventory, e, &binding, item, &result).await {
        Ok(confirmed) => {
            inventory
                .confirm_sync(ConfirmSyncRequest {
                    item_id: item.id.clone(),
                    expected_item_revision: item.revision,
                    confirmed,
                    completed_operation: None,
                    expected_operation_attempt: None,
                    activity: None,
                })
                .map_err(inventory_error)?;
        }
        Err(error) => notify(
            e,
            item,
            Some(binding.intent.command),
            Some(binding.operation.id),
            CreationStage::Blocked,
            Some(error.kind),
        ),
    }
    Ok(())
}
