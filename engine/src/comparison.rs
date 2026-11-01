//! Bounded, read-only version inspection. A preview never grants mutation consent.
use crate::{
    download::{
        filesystem::{self, Root},
        stream,
    },
    resolution::execution::Remote,
    Cancellation, ConflictRecord, EngineError, LocalState, RemoteState, ResolutionObservation,
};
use sha2::{Digest, Sha256};
use std::io::Read;

/// Maximum bytes per in-memory version; larger files retain metadata only.
pub(crate) const LIMIT: u64 = 1024 * 1024;

/// Verified bytes or an explicit fallback; no truncated content is presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonContent {
    Available { local: Vec<u8>, remote: Vec<u8> },
    MetadataOnly,
}

/// A point-in-time inspection bound to the saved conflict, not a live file handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionComparison {
    pub record: ConflictRecord,
    pub mime_type: String,
    pub local_size: u64,
    pub remote_size: u64,
    pub content: ComparisonContent,
}

/// Collects bytes through no-follow descriptors and brackets remote reads with
/// exact metadata. Neither originals, preservation copies nor decisions change.
/// Oversized versions are not streamed; unknown or stale versions return errors.
pub(crate) fn inspect(
    root: &Root,
    remote: &mut impl Remote,
    record: &ConflictRecord,
    cancel: &Cancellation,
) -> Result<VersionComparison, EngineError> {
    crate::ResolutionProposal::from_record(record).map_err(|_| EngineError::InvalidPlan)?;
    let path = record
        .relative_path
        .as_ref()
        .ok_or(EngineError::InvalidPlan)?;
    let id = record
        .remote_id
        .as_deref()
        .ok_or(EngineError::InvalidPlan)?;
    remote.configured()?;
    let version = remote.observe(id)?;
    if !version.valid() {
        return Err(EngineError::ObservationUnknown);
    }
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    let mut file = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    let stamp = crate::local::Stamp::read(&file)?;
    let observation = ResolutionObservation {
        relative_path: path.clone(),
        local: LocalState::Present {
            fingerprint: filesystem::fingerprint_root(root, path, cancel)?
                .ok_or(EngineError::DownloadConflict)?,
        },
        remote: RemoteState::Present {
            id: version.item.id.clone(),
            parent_id: version.item.parent_id.clone(),
            name: version.item.name.clone(),
            version: version
                .item
                .version
                .ok_or(EngineError::ObservationUnknown)?,
        },
    };
    crate::ResolutionProposal::from_record(record)
        .map_err(|_| EngineError::InvalidPlan)?
        .revalidate(&observation)
        .map_err(|_| EngineError::RemoteChanged)?;
    let size = version.item.size.ok_or(EngineError::ObservationUnknown)?;
    let content = if stamp.size <= LIMIT && size <= LIMIT {
        let mut local = Vec::new();
        (&mut file)
            .take(LIMIT + 1)
            .read_to_end(&mut local)
            .map_err(filesystem::io)?;
        if local.len() as u64 != stamp.size
            || crate::local::Stamp::read(&file)? != stamp
            || filesystem::identity(&file)?.fingerprint(&format!("{:x}", Sha256::digest(&local)))
                != record
                    .local_fingerprint
                    .clone()
                    .ok_or(EngineError::ObservationUnknown)?
        {
            return Err(EngineError::DownloadConflict);
        }
        let mut bytes = Vec::new();
        stream::copy(
            &mut *remote.media(&version)?,
            &mut bytes,
            size,
            version
                .item
                .md5_checksum
                .as_deref()
                .ok_or(EngineError::ObservationUnknown)?,
            cancel,
            |_| {},
            || root.validate_parent(path, parent_id),
        )?;
        ComparisonContent::Available {
            local,
            remote: bytes,
        }
    } else {
        ComparisonContent::MetadataOnly
    };
    crate::local::check(cancel)?;
    if remote.observe(id)? != version {
        return Err(EngineError::RemoteChanged);
    }
    root.validate_parent(path, parent_id)?;
    if filesystem::read(&parent, &name)?
        .as_ref()
        .map(crate::local::Stamp::read)
        .transpose()?
        != Some(stamp.clone())
        || crate::local::Stamp::read(&file)? != stamp
        || filesystem::fingerprint_root(root, path, cancel)?.as_ref()
            != record.local_fingerprint.as_ref()
    {
        return Err(EngineError::DownloadConflict);
    }
    remote.configured()?;
    Ok(VersionComparison {
        record: record.clone(),
        mime_type: version.item.mime_type,
        local_size: stamp.size,
        remote_size: size,
        content,
    })
}
