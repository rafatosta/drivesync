//! Recovery continues the same upload only from fresh server progress and a committed checkpoint.
use super::*;
use crate::inventory::models::RecoveryIssue;
impl GoogleOperator {
    pub(super) async fn continue_upload(
        &self,
        config: &OperatorConfig,
        record: &RecoveryRecord,
        fs: &Filesystem,
        journal: &dyn ExecutionJournal,
        session: &GoogleSession,
    ) -> RecoveryAssessment {
        let unknown = RecoveryAssessment::unknown;
        if record.command.action != OperationAction::Upload || record.reserved_remote_id.is_none() {
            return unknown(RecoveryIssue::MissingCommand);
        }
        if fs.check(&record.local).is_err() {
            return unknown(RecoveryIssue::SourceChanged);
        }
        let (Some(transfer), Some(parent), Some(name)) = (
            &record.transfer,
            &record.command.parent_id,
            &record.command.name,
        ) else {
            return unknown(RecoveryIssue::MissingCommand);
        };
        if record.local.size != Evidence::Known(transfer.total_bytes) {
            return unknown(RecoveryIssue::SourceChanged);
        }
        if self
            .destination(session, config, parent, name, None)
            .await
            .is_err()
        {
            return unknown(RecoveryIssue::DestinationChanged);
        }
        let upload = GoogleUploadSession {
            session_uri: transfer.session_uri.clone(),
            total_bytes: Some(transfer.total_bytes),
        };
        let progress = match activity::request(
            Phase::Resume,
            self.client.query_upload_progress(session, &upload),
        )
        .await
        {
            Ok(progress) => progress,
            Err(error) => {
                return unknown(if error.kind == GoogleApiError::ExpiredUploadSession {
                    RecoveryIssue::TransferExpired
                } else {
                    RecoveryIssue::RemoteInconclusive
                })
            }
        };
        let offset = match progress {
            GoogleUploadProgress::Complete { .. } => {
                report_resume(transfer.total_bytes, transfer.total_bytes);
                return self.probe_recovery(config, record, fs, session).await;
            }
            GoogleUploadProgress::Incomplete {
                received_range: None,
            } => 0,
            GoogleUploadProgress::Incomplete {
                received_range: Some(range),
            } if range.start == 0 => {
                let Some(end) = range.end_inclusive.and_then(|end| end.checked_add(1)) else {
                    return unknown(RecoveryIssue::RemoteInconclusive);
                };
                end
            }
            _ => return unknown(RecoveryIssue::RemoteInconclusive),
        };
        if offset < transfer.confirmed_offset
            || offset > transfer.total_bytes
            || (offset == transfer.total_bytes && offset != 0)
            || offset % (256 * 1024) != 0
        {
            return unknown(RecoveryIssue::TransferIncomplete);
        }
        report_resume(offset, transfer.total_bytes);
        if fs.validate_root().is_err()
            || fs.root_identity().ok().as_ref() != Some(&record.root_identity)
            || record
                .command
                .local_path
                .as_ref()
                .is_none_or(|p| fs.parent_identity(p).ok() != record.parent_identity)
        {
            return unknown(RecoveryIssue::ParentChanged);
        }
        // The account OS lease excludes competing execution; this CAS rejects stale Inventory facts.
        if journal
            .checkpoint(RecoveryCheckpoint::Transfer(TransferReference {
                session_uri: transfer.session_uri.clone(),
                total_bytes: transfer.total_bytes,
                confirmed_offset: offset,
            }))
            .is_err()
        {
            return unknown(RecoveryIssue::InventoryConflict);
        }
        let request = ExecuteOperationRequest {
            operation: record.command.clone(),
            execution_id: record.execution_id.clone(),
            expected_revision: Some(record.expected_revision.unwrap_or(0)),
            identity: record.identity.clone(),
            local: record.local.clone(),
            remote: record.remote.clone(),
            completed_steps: record.completed_steps.clone(),
        };
        if self
            .transfer(
                session,
                &request,
                fs,
                UploadCursor {
                    session: upload,
                    offset,
                },
                None,
                Some(journal),
            )
            .await
            .is_err()
        {
            return unknown(if fs.check(&record.local).is_err() {
                RecoveryIssue::SourceChanged
            } else {
                RecoveryIssue::TransferIncomplete
            });
        }
        // Completion can precede stable readback metadata. Retry observations only, never content.
        for attempt in 0..3 {
            let assessment = self.probe_recovery(config, record, fs, session).await;
            if attempt == 2
                || assessment.outcome == ExecutionEffect::Applied
                || !matches!(
                    assessment.issue,
                    Some(
                        RecoveryIssue::RemoteInconclusive
                            | RecoveryIssue::TransferExpired
                            | RecoveryIssue::TransferIncomplete
                    )
                )
            {
                return assessment;
            }
            let mut retry = Span::start(Phase::VerifyEffect, State::Waiting(Wait::Retry));
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            retry.finish(Outcome::Complete);
        }
        unreachable!()
    }
}

fn report_resume(offset: u64, total: u64) {
    let mut span = Span::start(Phase::Resume, State::Running);
    span.progress(
        Progress::Bytes(Bytes {
            confirmed: Some(offset),
            total: Some(total),
            ..Bytes::default()
        }),
        true,
    );
    span.finish(Outcome::Complete);
}
