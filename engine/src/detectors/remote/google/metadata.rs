//! Filtered Google metadata adaptation and conservative classification, independent of polling.
use super::{incomplete, now, valid, RemoteEventSummary, RemoteResult};
use crate::{models::*, providers::google::GoogleFile};

pub(in crate::detectors::remote) fn summarize(file: File) -> RemoteEventSummary {
    RemoteEventSummary {
        id: file.id,
        observed_at: file.observed_at,
        event: file.event,
        scope: file.scope,
        entry_kind: match file.data {
            Some(FileData::Remote(remote)) => match remote.kind {
                MetadataValue::Known(kind) => Some(kind),
                _ => None,
            },
            _ => None,
        },
    }
}
pub(super) fn is_folder(file: &GoogleFile) -> bool {
    file.mime_type.as_ref().map(|m| m.0.as_str()) == Some("application/vnd.google-apps.folder")
}
/// A single allowlist drives both retained progress and the provider payload.
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
pub(super) fn classify(
    previous: Option<&GoogleFile>,
    raw: &GoogleFile,
    scanning: bool,
    started_at: Option<i64>,
) -> (FileEventKind, Option<ObservationUncertainty>) {
    let Some(previous) = previous else {
        if raw.trashed == Some(false)
            && started_at
                .zip(
                    raw.created_time
                        .as_deref()
                        .and_then(crate::detectors::remote::timestamp::utc_millis),
                )
                .is_some_and(|(started, created)| {
                    created >= started && created <= now().unix_millis
                })
        {
            return (FileEventKind::Created, None);
        }
        return (
            FileEventKind::RecheckRequired,
            Some(if scanning {
                ObservationUncertainty::CaptureStarted
            } else {
                ObservationUncertainty::ObservationUnstable
            }),
        );
    };
    if raw.trashed != Some(false)
        || raw.name.is_none()
        || raw.mime_type.is_none()
        || raw.parents.is_none()
        || (previous.md5_checksum.is_some() && raw.md5_checksum.is_none())
        || (previous.sha256_checksum.is_some() && raw.sha256_checksum.is_none())
    {
        return (
            FileEventKind::RecheckRequired,
            Some(ObservationUncertainty::ObservationUnstable),
        );
    }
    let moved =
        previous.parents.is_some() && raw.parents.is_some() && previous.parents != raw.parents;
    let renamed = previous.name.is_some() && raw.name.is_some() && previous.name != raw.name;
    let content = (previous.md5_checksum.is_some()
        && raw.md5_checksum.is_some()
        && previous.md5_checksum != raw.md5_checksum)
        || (previous.sha256_checksum.is_some()
            && raw.sha256_checksum.is_some()
            && previous.sha256_checksum != raw.sha256_checksum);
    if (moved && renamed) || ((moved || renamed) && content) {
        return (
            FileEventKind::RecheckRequired,
            Some(ObservationUncertainty::ConflictingSequence),
        );
    }
    if moved {
        return (FileEventKind::Moved, None);
    }
    if renamed {
        return (FileEventKind::Renamed, None);
    }
    if content {
        return (FileEventKind::Modified, None);
    }
    if previous.modified_time.is_some()
        && raw.modified_time.is_some()
        && previous.modified_time != raw.modified_time
    {
        return (
            FileEventKind::RecheckRequired,
            Some(ObservationUncertainty::ObservationUnstable),
        );
    }
    (FileEventKind::MetadataChanged, None)
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
