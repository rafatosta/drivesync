//! Read-only remote content proof, bound to exact before/after metadata.
use super::{stream, Source};
use crate::{Cancellation, EngineError, InventoryEntry, InventoryKind};

/// Hashes bounded provider bytes into a sink, with size/MD5 and exact revision
/// verification. No staging file, installation or baseline is created.
pub(crate) fn content_fingerprint(
    item: &InventoryEntry,
    source: &mut impl Source,
    cancel: &Cancellation,
) -> Result<String, EngineError> {
    if item.kind != InventoryKind::Binary || item.version.is_none_or(|v| v == 0) {
        return Err(EngineError::InvalidPlan);
    }
    stream::cancelled(cancel)?;
    source.check_configuration()?;
    if source.metadata(item)? != *item {
        return Err(EngineError::RemoteChanged);
    }
    let mut input = source.media(&item.id)?;
    let hash = stream::copy(
        &mut input,
        &mut std::io::sink(),
        item.size.ok_or(EngineError::InvalidPlan)?,
        item.md5_checksum
            .as_deref()
            .ok_or(EngineError::InvalidPlan)?,
        cancel,
        |_| {},
    )?;
    source.check_configuration()?;
    if source.metadata(item)? != *item {
        return Err(EngineError::RemoteChanged);
    }
    stream::cancelled(cancel)?;
    Ok(hash)
}
