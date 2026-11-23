//! Recheck raw and projected namespaces; document links never authorize remote content writes.
use super::*;
impl GoogleOperator {
    pub(super) async fn projection_destination(
        &self,
        config: &OperatorConfig,
        session: &GoogleSession,
        remote: &RemoteState,
    ) -> Result<(), OperatorError> {
        let (Some(id), Evidence::Known(name), Evidence::Known(mime), Evidence::Known(parents)) =
            (&remote.id, &remote.name, &remote.mime_type, &remote.parents)
        else {
            return Err(filesystem::stale());
        };
        let projected = crate::projections::local_name(name, mime).ok_or_else(filesystem::stale)?;
        if parents.len() != 1 {
            return Err(filesystem::stale());
        }
        self.check_destination(session, config, &parents[0], name, Some(id), false)
            .await?;
        let quote = |s: &str| s.replace('\\', "\\\\").replace('\'', "\\'");
        let result = self
            .client
            .files_list(
                session,
                FileListOptions {
                    q: Some(format!(
                        "'{}' in parents and name = '{}' and trashed = false",
                        quote(&parents[0].0),
                        quote(&projected)
                    )),
                    fields: Some(FieldMask(
                        "files(id,name,mimeType),incompleteSearch,nextPageToken".into(),
                    )),
                    page_size: Some(100),
                    supports_all_drives: Some(true),
                    include_items_from_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(read_error)?;
        if result.incomplete_search != Some(false)
            || result.next_page_token.is_some()
            || result
                .files
                .iter()
                .any(|file| match (&file.id, &file.name, &file.mime_type) {
                    (Some(other), Some(name), Some(mime)) => {
                        other.0 != id.0
                            && crate::projections::local_name(name, &mime.0).as_ref()
                                == Some(&projected)
                    }
                    _ => true,
                })
        {
            return Err(filesystem::stale());
        }
        Ok(())
    }
}
