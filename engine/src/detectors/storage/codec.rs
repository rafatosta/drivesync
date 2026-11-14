//! Versioned transport codec preserving complete local and remote metadata.
use super::models::*;
use crate::models::*;
use serde::{Deserialize, Serialize};
const MAX_PAYLOAD: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    id: FileId,
    sync_id: SyncId,
    event: FileEvent,
    observed_at: Timestamp,
    coverage: ObservationCoverage,
    scope: FileScope,
    data: Option<Data>,
}
#[derive(Serialize, Deserialize)]
enum Data {
    Local(LocalFile),
    Remote(RemoteFile),
}
pub(super) fn encode(file: &File) -> QueueResult<Vec<u8>> {
    file.validate()
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
    let value = Wire {
        version: 2,
        id: file.id.clone(),
        sync_id: file.sync_id.clone(),
        event: file.event.clone(),
        observed_at: file.observed_at,
        coverage: file.coverage,
        scope: file.scope.clone(),
        data: match &file.data {
            Some(FileData::Local(data)) => Some(Data::Local(data.clone())),
            None => None,
            Some(FileData::Remote(value)) => Some(Data::Remote(value.clone())),
        },
    };
    let bytes =
        serde_json::to_vec(&value).map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
    if bytes.len() > MAX_PAYLOAD {
        return Err(QueueFailure::new(QueueError::CapacityExceeded));
    }
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8]) -> QueueResult<File> {
    if bytes.len() > MAX_PAYLOAD {
        return Err(QueueFailure::new(QueueError::Corrupt));
    }
    let value: Wire =
        serde_json::from_slice(bytes).map_err(|_| QueueFailure::new(QueueError::Corrupt))?;
    if value.version != 2 {
        return Err(QueueFailure::new(QueueError::SchemaUnsupported));
    }
    let file = File {
        id: value.id,
        sync_id: value.sync_id,
        event: value.event,
        observed_at: value.observed_at,
        coverage: value.coverage,
        scope: value.scope,
        data: value.data.map(|data| match data {
            Data::Local(local) => FileData::Local(local),
            Data::Remote(remote) => FileData::Remote(remote),
        }),
    };
    file.validate()
        .map_err(|_| QueueFailure::new(QueueError::Corrupt))?;
    Ok(file)
}
