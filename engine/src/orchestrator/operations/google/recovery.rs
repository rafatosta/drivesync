//! Read-only provider reconciliation by reserved ID, execution marker and current content evidence.
use super::*;
use crate::inventory::models::RecoveryIssue;
impl GoogleOperator {
    pub(super) async fn probe_recovery(
        &self,
        config: &OperatorConfig,
        record: &RecoveryRecord,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> RecoveryAssessment {
        if let Some(id) = &record.command.conflict_id {
            if record.conflict_archive.as_ref() != Some(id) {
                return RecoveryAssessment {
                    outcome: ExecutionEffect::NotApplied,
                    issue: Some(RecoveryIssue::NotDispatched),
                    effect: None,
                };
            }
            if !fs.conflict_archived(id).unwrap_or(false) {
                return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged);
            }
        }
        if matches!(
            record.command.action,
            OperationAction::TrashRemote | OperationAction::RemoveLocal
        ) {
            return self.recover_trash(config, record, fs, session).await;
        }
        match record.command.action {
            OperationAction::RenameLocal
            | OperationAction::MoveLocal
            | OperationAction::RenameRemote
            | OperationAction::MoveRemote => {
                let Some(ancestors) = &record.remote_ancestry else {
                    return RecoveryAssessment {
                        outcome: ExecutionEffect::NotApplied,
                        issue: Some(RecoveryIssue::NotDispatched),
                        effect: None,
                    };
                };
                if self.check_ancestry(session, ancestors).await.is_err() {
                    return RecoveryAssessment::unknown(RecoveryIssue::ParentChanged);
                }
                let Some(id) = record.command.remote_id.as_ref() else {
                    return RecoveryAssessment::unknown(RecoveryIssue::MissingCommand);
                };
                if self
                    .within(session, id, &config.remote_root_id)
                    .await
                    .is_err()
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive);
                }
                let snapshot = match self.snapshot(session, id).await {
                    Ok(v) => v,
                    Err(_) => {
                        return RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive)
                    }
                };
                let remote = normalize(&snapshot.file);
                let local_action = matches!(
                    record.command.action,
                    OperationAction::RenameLocal | OperationAction::MoveLocal
                );
                let local = if local_action {
                    if !(crate::projections::same_representation(&remote, &record.remote)
                        || remote == record.remote)
                    {
                        return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                    }
                    match fs.moved(record) {
                        Ok(Some(local)) => local,
                        Ok(None) => {
                            return RecoveryAssessment {
                                outcome: ExecutionEffect::NotApplied,
                                issue: Some(RecoveryIssue::NotDispatched),
                                effect: None,
                            }
                        }
                        Err(_) => {
                            return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged)
                        }
                    }
                } else {
                    let local = match fs.check(&record.local) {
                        Ok(v) => v,
                        Err(_) => return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged),
                    };
                    let mut expected = record.remote.clone();
                    if record.command.action == OperationAction::RenameRemote {
                        expected.name =
                            Evidence::Known(record.command.name.clone().unwrap_or_default());
                    } else {
                        expected.parents =
                            Evidence::Known(record.command.parent_id.iter().cloned().collect());
                    }
                    if remote.version == record.remote.version
                        && remote.name == record.remote.name
                        && remote.parents == record.remote.parents
                        && remote.checksum == record.remote.checksum
                    {
                        return RecoveryAssessment {
                            outcome: ExecutionEffect::NotApplied,
                            issue: Some(RecoveryIssue::NotDispatched),
                            effect: None,
                        };
                    }
                    if snapshot
                        .file
                        .app_properties
                        .as_ref()
                        .and_then(|p| p.get("drivesyncExecution"))
                        != Some(&record.execution_id.0)
                        || !matches!((&remote.version, &record.remote.version), (Evidence::Known(a), Evidence::Known(b)) if a > b)
                        || remote.name != expected.name
                        || remote.parents != expected.parents
                        || remote.checksum != expected.checksum
                        || remote.size != expected.size
                        || remote.mime_type != expected.mime_type
                        || remote.trashed != expected.trashed
                    {
                        return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged);
                    }
                    local
                };
                if self
                    .snapshot(session, id)
                    .await
                    .map(|v| normalize(&v.file).version)
                    .ok()
                    != Some(remote.version.clone())
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                }
                if self.check_ancestry(session, ancestors).await.is_err() {
                    return RecoveryAssessment::unknown(RecoveryIssue::ParentChanged);
                }
                let checked = if local_action {
                    fs.moved(record)
                        .and_then(|v| v.ok_or_else(filesystem::stale))
                } else {
                    fs.check(&record.local)
                };
                if !checked.is_ok_and(|current| current == local) {
                    return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                }
                RecoveryAssessment {
                    outcome: ExecutionEffect::Applied,
                    issue: None,
                    effect: Some(AppliedEffect {
                        local: Some(local),
                        remote: Some(remote),
                        remote_id: Some(id.clone()),
                        verified: true,
                    }),
                }
            }
            OperationAction::CreateLocalDirectory
            | OperationAction::CreateLocalShortcut
            | OperationAction::Download
            | OperationAction::UpdateLocalContent => {
                let Some(id) = record.command.remote_id.as_ref() else {
                    return RecoveryAssessment::unknown(RecoveryIssue::MissingCommand);
                };
                if self
                    .within(session, id, &config.remote_root_id)
                    .await
                    .is_err()
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive);
                }
                let remote = match self.snapshot(session, id).await {
                    Ok(snapshot) => normalize(&snapshot.file),
                    Err(_) => {
                        return RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive)
                    }
                };
                if (record.command.action == OperationAction::CreateLocalShortcut
                    && !crate::projections::same_representation(&remote, &record.remote))
                    || !matches!(remote.version, Evidence::Known(_))
                    || (record.command.action != OperationAction::CreateLocalShortcut
                        && record.command.action != OperationAction::UpdateLocalContent
                        && remote.version != record.remote.version)
                    || (record.command.action == OperationAction::UpdateLocalContent
                        && !remote.unchanged_binary(&record.remote))
                    || remote.name != record.remote.name
                    || remote.parents != record.remote.parents
                    || remote.trashed != Evidence::Known(false)
                    || (matches!(
                        record.command.action,
                        OperationAction::Download | OperationAction::UpdateLocalContent
                    ) && (remote.checksum != record.remote.checksum
                        || remote.size != record.remote.size))
                    || (record.command.action != OperationAction::CreateLocalShortcut
                        && remote.mime_type != record.remote.mime_type)
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                }
                if record.command.action == OperationAction::UpdateLocalContent
                    && record.stage_identity.is_none()
                    && fs.check(&record.local).is_ok()
                {
                    return RecoveryAssessment {
                        outcome: ExecutionEffect::NotApplied,
                        issue: Some(RecoveryIssue::NotDispatched),
                        effect: None,
                    };
                }
                if crate::projections::workspace(&remote)
                    && self
                        .projection_destination(config, session, &remote)
                        .await
                        .is_err()
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged);
                }
                match fs.published(record) {
                    Ok(None) => RecoveryAssessment {
                        outcome: ExecutionEffect::NotApplied,
                        issue: Some(RecoveryIssue::NotDispatched),
                        effect: None,
                    },
                    Ok(Some(local))
                        if crate::orchestrator::evidence::equivalent(&local, &remote) =>
                    {
                        let after = match self.snapshot(session, id).await {
                            Ok(after) => after,
                            Err(_) => {
                                return RecoveryAssessment::unknown(
                                    RecoveryIssue::RemoteInconclusive,
                                )
                            }
                        };
                        if normalize(&after.file).version != remote.version {
                            return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                        }
                        let local = match fs.published(record) {
                            Ok(Some(local))
                                if crate::orchestrator::evidence::equivalent(&local, &remote) =>
                            {
                                local
                            }
                            _ => {
                                return RecoveryAssessment::unknown(
                                    RecoveryIssue::DestinationChanged,
                                )
                            }
                        };
                        RecoveryAssessment {
                            outcome: ExecutionEffect::Applied,
                            issue: None,
                            effect: Some(AppliedEffect {
                                local: Some(local),
                                remote: Some(remote),
                                remote_id: Some(id.clone()),
                                verified: true,
                            }),
                        }
                    }
                    _ => RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged),
                }
            }
            OperationAction::CreateRemoteDirectory
            | OperationAction::Upload
            | OperationAction::UpdateRemoteContent => {
                let Some(id) = record
                    .reserved_remote_id
                    .as_ref()
                    .or(record.command.remote_id.as_ref())
                else {
                    return RecoveryAssessment::unknown(RecoveryIssue::MissingCommand);
                };
                let source = || {
                    if record.command.action == OperationAction::UpdateRemoteContent {
                        fs.reconcile_source(&record.local)
                    } else {
                        fs.check(&record.local)
                    }
                };
                let local = match source() {
                    Ok(local) => local,
                    Err(_) => return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged),
                };
                if let Ok(snapshot) = self.snapshot(session, id).await {
                    let file = &snapshot.file;
                    let remote = normalize(file);
                    let marker = file
                        .app_properties
                        .as_ref()
                        .and_then(|values| values.get("drivesyncExecution"));
                    if marker == Some(&record.execution_id.0)
                        && file.name == record.command.name
                        && remote.parents
                            == if record.command.action == OperationAction::UpdateRemoteContent {
                                record.remote.parents.clone()
                            } else {
                                Evidence::Known(record.command.parent_id.iter().cloned().collect())
                            }
                        && (record.command.action != OperationAction::UpdateRemoteContent
                            || matches!((&remote.version,&record.remote.version),(Evidence::Known(after),Evidence::Known(before)) if after > before))
                        && matches!(remote.version,Evidence::Known(version) if version > 0)
                        && crate::orchestrator::evidence::equivalent(&local, &remote)
                        && self
                            .within(session, id, &config.remote_root_id)
                            .await
                            .is_ok()
                    {
                        if let Ok(after) = self.snapshot(session, id).await {
                            if normalize(&after.file).version == remote.version {
                                let local = match source() {
                                    Ok(local) => local,
                                    Err(_) => {
                                        return RecoveryAssessment::unknown(
                                            RecoveryIssue::SourceChanged,
                                        )
                                    }
                                };
                                return RecoveryAssessment {
                                    outcome: ExecutionEffect::Applied,
                                    issue: None,
                                    effect: Some(AppliedEffect {
                                        local: Some(local),
                                        remote: Some(remote),
                                        remote_id: Some(id.clone()),
                                        verified: true,
                                    }),
                                };
                            }
                        }
                    }
                }
                if record.command.action == OperationAction::UpdateRemoteContent
                    && record.transfer.is_some()
                {
                    // Keep the conditional replacement session private; uncertain updates never resume without a fresh write policy.
                    return RecoveryAssessment::unknown(RecoveryIssue::TransferIncomplete);
                }
                if let Some(transfer) = &record.transfer {
                    if transfer.total_bytes
                        != match record.local.size {
                            Evidence::Known(size) => size,
                            _ => return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged),
                        }
                    {
                        return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
                    }
                    let upload = GoogleUploadSession {
                        session_uri: transfer.session_uri.clone(),
                        total_bytes: Some(transfer.total_bytes),
                    };
                    match activity::request(
                        Phase::Resume,
                        self.client.query_upload_progress(session, &upload),
                    )
                    .await
                    {
                        Ok(GoogleUploadProgress::Incomplete { .. }) => {
                            return RecoveryAssessment::unknown(RecoveryIssue::TransferIncomplete)
                        }
                        Err(error) if error.kind == GoogleApiError::ExpiredUploadSession => {
                            return RecoveryAssessment::unknown(RecoveryIssue::TransferExpired)
                        }
                        _ => {}
                    }
                }
                RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive)
            }
            _ => RecoveryAssessment::unknown(RecoveryIssue::UnsupportedAction),
        }
    }
}
