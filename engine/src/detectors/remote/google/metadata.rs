//! Filtered Google metadata adaptation without historical comparison or action classification.
use super::{incomplete, valid, RemoteResult};
use crate::{models::*, providers::google::GoogleFile};

/// A single allowlist drives the observed metadata and the provider payload.
pub(super) fn filtered(raw: &GoogleFile) -> GoogleFile {
    GoogleFile {
        id: raw.id.clone(),
        name: raw.name.clone(),
        mime_type: raw.mime_type.clone(),
        parents: raw.parents.clone(),
        trashed: raw.trashed,
        version: raw.version.clone(),
        created_time: raw.created_time.clone(),
        modified_time: raw.modified_time.clone(),
        size: raw.size.clone(),
        md5_checksum: raw.md5_checksum.clone(),
        sha256_checksum: raw.sha256_checksum.clone(),
        ..Default::default()
    }
}
pub(super) fn location(raw: &GoogleFile, account: &AccountId) -> Option<FileLocation> {
    Some(FileLocation::Remote {
        reference: RemoteFileRef {
            account_id: account.clone(),
            file_id: RemoteFileId(raw.id.as_ref()?.0.clone()),
        },
        parent_id: raw.parents.as_ref().and_then(|p| {
            if p.len() == 1 {
                Some(RemoteFileId(p[0].0.clone()))
            } else {
                None
            }
        }),
        name: raw.name.clone(),
    })
}
fn value<T>(value: Option<T>) -> MetadataValue<T> {
    value
        .map(MetadataValue::Known)
        .unwrap_or(MetadataValue::NotCollected)
}
pub(super) fn adapt(raw: &GoogleFile, account: &AccountId) -> RemoteResult<RemoteFile> {
    let id = raw
        .id
        .as_ref()
        .filter(|id| valid(&id.0))
        .ok_or_else(incomplete)?;
    let raw = filtered(raw);
    let mut payload = serde_json::to_value(&raw).map_err(|_| incomplete())?;
    let allowed = [
        "id",
        "name",
        "mimeType",
        "parents",
        "trashed",
        "version",
        "createdTime",
        "modifiedTime",
        "size",
        "md5Checksum",
        "sha256Checksum",
    ];
    payload
        .as_object_mut()
        .ok_or_else(incomplete)?
        .retain(|key, value| allowed.contains(&key.as_str()) && !value.is_null());
    let payload = serde_json::to_vec(&payload).map_err(|_| incomplete())?;
    let provider = ProviderId("google".into());
    let file = RemoteFile {
        reference: RemoteFileRef {
            account_id: account.clone(),
            file_id: RemoteFileId(id.0.clone()),
        },
        provider: provider.clone(),
        name: value(raw.name.clone()),
        kind: value(raw.mime_type.as_ref().map(|mime| match mime.0.as_str() {
            "application/vnd.google-apps.folder" => RemoteEntryKind::Directory,
            "application/vnd.google-apps.shortcut" => RemoteEntryKind::Shortcut,
            _ => RemoteEntryKind::File,
        })),
        mime_type: value(raw.mime_type.as_ref().map(|mime| mime.0.clone())),
        parent_ids: value(
            raw.parents
                .as_ref()
                .map(|parents| parents.iter().map(|p| RemoteFileId(p.0.clone())).collect()),
        ),
        trashed: value(raw.trashed),
        version: value(raw.version.clone()),
        created_time: value(raw.created_time.clone()),
        modified_time: value(raw.modified_time.clone()),
        size: match raw.size {
            Some(ref size) => value(Some(size.parse::<u64>().map_err(|_| incomplete())?)),
            None => MetadataValue::NotCollected,
        },
        md5_checksum: value(raw.md5_checksum.clone()),
        sha256_checksum: value(raw.sha256_checksum.clone()),
        provider_data: MetadataValue::Known(ProviderFileData {
            provider,
            format: "google.drive.detector-file".into(),
            format_version: 1,
            payload,
        }),
    };
    file.validate().map_err(|_| incomplete())?;
    Ok(file)
}

/// Extracts a common File from the immutable original record at delivery time.
pub(crate) fn extract(raw: &crate::detectors::record::RawRemoteObservation) -> RemoteResult<File> {
    raw.validate().map_err(|_| incomplete())?;
    let (file, evidence) = match raw.source {
        RemoteObservationSource::Capture => (
            Some(
                serde_json::from_value::<GoogleFile>(raw.original.clone())
                    .map_err(|_| incomplete())?,
            ),
            RemoteObservation {
                source: raw.source,
                removed: None,
                time: None,
                change_type: None,
                drive_id: None,
            },
        ),
        RemoteObservationSource::Changes => {
            let change: crate::providers::google::GoogleChange =
                serde_json::from_value(raw.original.clone()).map_err(|_| incomplete())?;
            (
                change.file,
                RemoteObservation {
                    source: raw.source,
                    removed: change.removed,
                    time: change.time,
                    change_type: change.change_type,
                    drive_id: change.drive_id.map(|id| id.0),
                },
            )
        }
    };
    let event = File {
        id: raw.id.clone(),
        sync_id: raw.sync_id.clone(),
        observed_at: raw.observed_at,
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Remote(RemoteFileRef {
            account_id: raw.account_id.clone(),
            file_id: raw.resource_id.clone(),
        }),
        event: FileEvent {
            local: None,
            remote: Some(evidence),
            kind: FileEventKind::Observed,
            uncertainty: None,
            previous: None,
            current: file
                .as_ref()
                .and_then(|file| location(file, &raw.account_id)),
        },
        data: file
            .as_ref()
            .map(|file| adapt(file, &raw.account_id))
            .transpose()?
            .map(FileData::Remote),
    };
    event.validate().map_err(|_| incomplete())?;
    Ok(event)
}

/// Checks the resource identity in the original Google item without adapting metadata.
pub(crate) fn validate_original(
    raw: &crate::detectors::record::RawRemoteObservation,
) -> ModelResult<()> {
    let key = match raw.source {
        RemoteObservationSource::Capture => "id",
        RemoteObservationSource::Changes => "fileId",
    };
    if raw.original.get(key).and_then(serde_json::Value::as_str) != Some(raw.resource_id.0.as_str())
    {
        return Err(ModelError::InconsistentEvent);
    }
    if raw.source == RemoteObservationSource::Changes {
        if let Some(file) = raw.original.get("file").filter(|file| !file.is_null()) {
            if !file.is_object()
                || file.get("id").and_then(serde_json::Value::as_str)
                    != Some(raw.resource_id.0.as_str())
            {
                return Err(ModelError::InconsistentEvent);
            }
        }
    }
    Ok(())
}
