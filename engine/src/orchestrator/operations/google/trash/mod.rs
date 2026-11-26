//! Revalidate trash scope, empty descendants and retained local evidence immediately before effects.
use super::*;
use crate::inventory::models::RecoveryIssue;
impl GoogleOperator {
    pub(super) async fn trash_guard(
        &self,
        config: &OperatorConfig,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> Result<(), OperatorError> {
        if !config.trash_propagation_authorized {
            return Err(filesystem::stale());
        }
        self.trash_root(config, session).await?;
        if r.operation.action == OperationAction::TrashRemote {
            fs.confirm_missing_incarnation(&r.local, &config.sync_root_id)?;
        } else {
            fs.check(&r.local)?;
        }
        if r.local.kind == Evidence::Known(FileKind::Directory) {
            if r.local.state == FileState::Present {
                fs.require_empty(r.local.path.as_ref().ok_or_else(filesystem::stale)?)?;
            }
            self.require_remote_empty(session, r.remote.id.as_ref().ok_or_else(filesystem::stale)?)
                .await?;
        }
        Ok(())
    }
    pub(super) async fn trash_root(
        &self,
        config: &OperatorConfig,
        session: &GoogleSession,
    ) -> Result<(), OperatorError> {
        let root = self.snapshot(session, &config.remote_root_id).await?.file;
        if root.trashed != Some(false)
            || root.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
            || !root
                .capabilities
                .is_some_and(|c| c.can_list_children == Some(true))
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    pub(super) async fn require_remote_empty(
        &self,
        session: &GoogleSession,
        id: &RemoteId,
    ) -> Result<(), OperatorError> {
        let escaped = id.0.replace('\\', "\\\\").replace('\'', "\\'");
        let children = activity::page(
            self.client.files_list(
                session,
                FileListOptions {
                    q: Some(format!("'{escaped}' in parents")),
                    fields: Some(FieldMask("files(id),nextPageToken,incompleteSearch".into())),
                    page_size: Some(1),
                    supports_all_drives: Some(true),
                    include_items_from_all_drives: Some(true),
                    ..Default::default()
                },
            ),
            |page| page.files.len(),
        )
        .await
        .map_err(read_error)?;
        if children.incomplete_search != Some(false)
            || children.next_page_token.is_some()
            || !children.files.is_empty()
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
    pub(super) async fn recover_trash(
        &self,
        config: &OperatorConfig,
        record: &RecoveryRecord,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> RecoveryAssessment {
        let unknown = || RecoveryAssessment::unknown(RecoveryIssue::SourceChanged);
        let Some(id) = &record.command.remote_id else {
            return unknown();
        };
        let Some(ancestors) = &record.remote_ancestry else {
            return RecoveryAssessment {
                outcome: ExecutionEffect::NotApplied,
                issue: Some(RecoveryIssue::NotDispatched),
                effect: None,
            };
        };
        if self.check_ancestry(session, ancestors).await.is_err()
            || self
                .within(session, id, &config.remote_root_id)
                .await
                .is_err()
        {
            return RecoveryAssessment::unknown(RecoveryIssue::ParentChanged);
        }
        if self.trash_root(config, session).await.is_err() {
            return RecoveryAssessment::unknown(RecoveryIssue::RootChanged);
        }
        let snapshot = match self.snapshot(session, id).await {
            Ok(s) => s,
            Err(_) => return unknown(),
        };
        let remote = normalize(&snapshot.file);
        if remote.trashed != Evidence::Known(true) {
            if record.command.action == OperationAction::TrashRemote
                && crate::conflicts::remote_matches(&remote, &record.remote)
            {
                return RecoveryAssessment {
                    outcome: ExecutionEffect::NotApplied,
                    issue: Some(RecoveryIssue::NotDispatched),
                    effect: None,
                };
            }
            return unknown();
        }
        let local = if record.command.action == OperationAction::TrashRemote {
            if snapshot
                .file
                .app_properties
                .as_ref()
                .and_then(|p| p.get("drivesyncExecution"))
                != Some(&record.execution_id.0)
                || remote.name != record.remote.name
                || remote.parents != record.remote.parents
                || remote.mime_type != record.remote.mime_type
                || remote.checksum != record.remote.checksum
                || remote.size != record.remote.size
                || !matches!((&remote.version,&record.remote.version),(Evidence::Known(a),Evidence::Known(b)) if a>b)
                || fs
                    .confirm_missing_incarnation(&record.local, &config.sync_root_id)
                    .is_err()
            {
                return unknown();
            }
            record.local.clone()
        } else {
            if !crate::conflicts::remote_matches(&remote, &record.remote) {
                return unknown();
            }
            if record.local_trash.is_none() {
                return if fs.check(&record.local).is_ok() {
                    RecoveryAssessment {
                        outcome: ExecutionEffect::NotApplied,
                        issue: Some(RecoveryIssue::NotDispatched),
                        effect: None,
                    }
                } else {
                    unknown()
                };
            }
            match fs.trashed(record) {
                Ok(Some(local)) => local,
                Ok(None) => {
                    return RecoveryAssessment {
                        outcome: ExecutionEffect::NotApplied,
                        issue: Some(RecoveryIssue::NotDispatched),
                        effect: None,
                    }
                }
                Err(_) => return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged),
            }
        };
        if local.kind == Evidence::Known(FileKind::Directory)
            && self.require_remote_empty(session, id).await.is_err()
        {
            return unknown();
        }
        let after = match self.snapshot(session, id).await {
            Ok(s) => normalize(&s.file),
            Err(_) => return unknown(),
        };
        if !crate::conflicts::remote_matches(&after, &remote)
            || self.check_ancestry(session, ancestors).await.is_err()
            || self.trash_root(config, session).await.is_err()
        {
            return unknown();
        }
        if record.command.action == OperationAction::RemoveLocal
            && fs.trashed(record).ok().flatten().as_ref() != Some(&local)
        {
            return RecoveryAssessment::unknown(RecoveryIssue::DestinationChanged);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::operations::{
        google::tests::{endpoint, operator},
        tests::Sandbox,
    };
    #[tokio::test]
    async fn empty_folder_proof_rejects_children_incomplete_pages_and_access_loss() {
        for (status, body, allowed) in [
            (200, r#"{"files":[],"incompleteSearch":false}"#, true),
            (
                200,
                r#"{"files":[{"id":"edited-child"}],"incompleteSearch":false}"#,
                false,
            ),
            (
                200,
                r#"{"files":[],"incompleteSearch":false,"nextPageToken":"more"}"#,
                false,
            ),
            (200, r#"{"files":[],"incompleteSearch":true}"#, false),
            (403, r#"{}"#, false),
        ] {
            let s = Sandbox::new();
            let server = endpoint(move |_| (status, body.into(), vec![])).await;
            let proof = operator(&s, server.client.clone())
                .require_remote_empty(
                    &GoogleSession::new("test", "synthetic"),
                    &RemoteId("folder".into()),
                )
                .await;
            assert_eq!(proof.is_ok(), allowed);
            assert!(server
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| r.starts_with("GET ") && !r.contains("trashed%20%3D%20false")));
        }
    }
}
