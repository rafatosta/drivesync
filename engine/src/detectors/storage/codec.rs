//! Versioned transport codec preserving complete local and remote metadata.
use super::models::*;
use crate::{detectors::record::ObservationRecord, models::*};
use serde::{Deserialize, Serialize};
const MAX_PAYLOAD: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    record: ObservationRecord,
}
pub(super) fn encode(file: &File) -> QueueResult<Vec<u8>> {
    encode_record(&ObservationRecord::Common(Box::new(file.clone())))
}
pub(super) fn encode_record(record: &ObservationRecord) -> QueueResult<Vec<u8>> {
    record
        .validate()
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
    let bytes = serde_json::to_vec(&Wire {
        version: 4,
        record: record.clone(),
    })
    .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))?;
    if bytes.len() > MAX_PAYLOAD {
        return Err(QueueFailure::new(QueueError::CapacityExceeded));
    }
    Ok(bytes)
}
pub(super) fn decode_record(bytes: &[u8]) -> QueueResult<ObservationRecord> {
    if bytes.len() > MAX_PAYLOAD {
        return Err(QueueFailure::new(QueueError::Corrupt));
    }
    let value: Wire =
        serde_json::from_slice(bytes).map_err(|_| QueueFailure::new(QueueError::Corrupt))?;
    if value.version != 4 {
        return Err(QueueFailure::new(QueueError::SchemaUnsupported));
    }
    value
        .record
        .validate()
        .map_err(|_| QueueFailure::new(QueueError::Corrupt))?;
    Ok(value.record)
}
#[cfg(test)]
pub(super) fn decode(bytes: &[u8]) -> QueueResult<File> {
    decode_record(bytes)?
        .into_file()
        .map_err(|_| QueueFailure::new(QueueError::InvalidRequest))
}
