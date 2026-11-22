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
        match record.command.action {
            OperationAction::CreateLocalDirectory | OperationAction::Download => {
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
                if !matches!(remote.version, Evidence::Known(_))
                    || remote.version != record.remote.version
                    || remote.name != record.remote.name
                    || remote.parents != record.remote.parents
                    || remote.trashed != Evidence::Known(false)
                    || (record.command.action == OperationAction::Download
                        && (remote.checksum != record.remote.checksum
                            || remote.size != record.remote.size))
                    || remote.mime_type != record.remote.mime_type
                {
                    return RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
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
            OperationAction::CreateRemoteDirectory | OperationAction::Upload => {
                let Some(id) = record.reserved_remote_id.as_ref() else {
                    return RecoveryAssessment::unknown(RecoveryIssue::MissingCommand);
                };
                let local = match fs.check(&record.local) {
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
                        && file.parents.as_ref().map(|parents| {
                            parents
                                .iter()
                                .map(|id| RemoteId(id.0.clone()))
                                .collect::<Vec<_>>()
                        }) == record.command.parent_id.as_ref().map(|id| vec![id.clone()])
                        && matches!(remote.version,Evidence::Known(version) if version > 0)
                        && crate::orchestrator::evidence::equivalent(&local, &remote)
                        && self
                            .within(session, id, &config.remote_root_id)
                            .await
                            .is_ok()
                    {
                        if let Ok(after) = self.snapshot(session, id).await {
                            if normalize(&after.file).version == remote.version {
                                let local = match fs.check(&record.local) {
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
                    if let Ok(GoogleUploadProgress::Incomplete { .. }) =
                        self.client.query_upload_progress(session, &upload).await
                    {
                        return RecoveryAssessment::unknown(RecoveryIssue::TransferIncomplete);
                    }
                }
                RecoveryAssessment::unknown(RecoveryIssue::RemoteInconclusive)
            }
            _ => RecoveryAssessment::unknown(RecoveryIssue::UnsupportedAction),
        }
    }
}
