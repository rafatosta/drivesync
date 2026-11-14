//! Incremental traversal DAO. The Store owns the enclosing checkpoint transaction.
use super::{models::*, store::sql};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

/// One directory page at the head of the durable traversal queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteScan {
    pub folder: String,
    pub token: Option<String>,
}
/// Bounded page delta; no directory catalog or queue snapshot is transported.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteScanUpdate {
    pub reset: bool,
    pub finished: Option<RemoteScan>,
    pub continuation: Option<String>,
    pub discovered: Vec<String>,
}
impl RemoteScanUpdate {
    /// Validates bounded traversal instructions without accessing SQLite.
    pub(crate) fn validate(&self) -> QueueResult<()> {
        if self.finished.as_ref().is_some_and(|scan| {
            !valid(&scan.folder) || scan.token.as_deref().is_some_and(|token| !valid(token))
        }) || self.discovered.len() > 200
            || self.discovered.iter().any(|id| !valid(id))
            || self
                .continuation
                .as_deref()
                .is_some_and(|token| !valid(token))
            || (self.continuation.is_some() && self.finished.is_none())
            || (self.reset && self.finished.is_some())
        {
            return Err(QueueFailure::new(QueueError::InvalidRequest));
        }
        Ok(())
    }
}
fn valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}
pub(super) fn next(connection: &Connection) -> QueueResult<Option<RemoteScan>> {
    let scan = connection
        .query_row(
            "SELECT folder,token FROM remote_scan WHERE done=0 ORDER BY seq LIMIT 1",
            [],
            |row| {
                Ok(RemoteScan {
                    folder: row.get(0)?,
                    token: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(sql)?;
    if scan.as_ref().is_some_and(|scan| {
        !valid(&scan.folder) || scan.token.as_deref().is_some_and(|token| !valid(token))
    }) {
        return Err(QueueFailure::new(QueueError::Corrupt));
    }
    Ok(scan)
}
/// Checks counters during open; normal page writes never count or scan the whole queue.
pub(super) fn validate(connection: &Connection) -> QueueResult<()> {
    let valid: bool = connection.query_row(
        "SELECT total=(SELECT COUNT(*) FROM remote_scan) AND pending=(SELECT COUNT(*) FROM remote_scan WHERE done=0) FROM remote_scan_control WHERE id=1", [], |row| row.get(0),
    ).map_err(sql)?;
    if !valid {
        return Err(QueueFailure::new(QueueError::Corrupt));
    }
    Ok(())
}
pub(super) fn apply(tx: &Transaction<'_>, update: &RemoteScanUpdate) -> QueueResult<()> {
    update.validate()?;
    if update.reset {
        tx.execute("DELETE FROM remote_scan", []).map_err(sql)?;
        tx.execute(
            "UPDATE remote_scan_control SET total=0,pending=0 WHERE id=1",
            [],
        )
        .map_err(sql)?;
    }
    if let Some(finished) = &update.finished {
        if next(tx)?.as_ref() != Some(finished) {
            return Err(QueueFailure::new(QueueError::Conflict));
        }
        if let Some(token) = &update.continuation {
            if finished.token.as_ref() == Some(token) {
                return Err(QueueFailure::new(QueueError::InvalidRequest));
            }
            tx.execute(
                "UPDATE remote_scan SET token=?2 WHERE folder=?1",
                params![finished.folder, token],
            )
            .map_err(sql)?;
        } else {
            tx.execute(
                "UPDATE remote_scan SET done=1,token=NULL WHERE folder=?1",
                [&finished.folder],
            )
            .map_err(sql)?;
            tx.execute(
                "UPDATE remote_scan_control SET pending=pending-1 WHERE id=1",
                [],
            )
            .map_err(sql)?;
        }
    }
    let (mut total, mut pending): (u64, u64) = tx
        .query_row(
            "SELECT total,pending FROM remote_scan_control WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(sql)?;
    for folder in &update.discovered {
        let inserted = tx.execute("INSERT INTO remote_scan(folder,token,done) VALUES(?1,NULL,0) ON CONFLICT(folder) DO NOTHING", [folder]).map_err(sql)?;
        total += inserted as u64;
        pending += inserted as u64;
        if total > 100_000 {
            return Err(QueueFailure::new(QueueError::CapacityExceeded));
        }
    }
    tx.execute(
        "UPDATE remote_scan_control SET total=?1,pending=?2 WHERE id=1",
        params![total, pending],
    )
    .map_err(sql)?;
    Ok(())
}
