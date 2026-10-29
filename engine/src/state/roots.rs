//! Durable root bindings and actionable suspension reasons outside user volumes.
use super::{failed, Store};
use crate::{EngineError, FolderConfiguration, RootSuspension};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

const SCHEMA: &str = "CREATE TABLE bindings(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,marker TEXT NOT NULL,reason TEXT,PRIMARY KEY(client,account,remote,local));";
/// Does not contain inventory, credentials or confirmed installation evidence.
pub(crate) struct Roots(Store);
impl Roots {
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        Ok(Self(Store::open_format(path, 0x44535254, 1, SCHEMA)?))
    }
    /// Explicit configuration is the only operation that can establish a binding.
    /// An existing binding cannot silently adopt a replacement marker.
    pub(crate) fn bind(
        &self,
        client: &str,
        folder: &FolderConfiguration,
        marker: &str,
    ) -> Result<(), EngineError> {
        let old = self.marker(client, folder)?;
        if marker.is_empty() || old.as_deref().is_some_and(|old| old != marker) {
            return Err(EngineError::RootSuspended(
                RootSuspension::IdentityUnverified,
            ));
        }
        self.0.0.execute("INSERT INTO bindings VALUES(?1,?2,?3,?4,?5,NULL) ON CONFLICT(client,account,remote,local) DO NOTHING", params![client,folder.account_id.0,folder.remote.id,folder.local.to_str().ok_or(EngineError::InvalidPlan)?,marker]).map_err(failed)?;
        if self.marker(client, folder)?.as_deref() != Some(marker) {
            return Err(EngineError::RootSuspended(
                RootSuspension::IdentityUnverified,
            ));
        }
        Ok(())
    }
    pub(crate) fn marker(
        &self,
        client: &str,
        folder: &FolderConfiguration,
    ) -> Result<Option<String>, EngineError> {
        self.0.0.query_row("SELECT marker FROM bindings WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4", params![client,folder.account_id.0,folder.remote.id,folder.local.to_str().ok_or(EngineError::InvalidPlan)?], |r|r.get(0)).optional().map_err(failed)
    }
    pub(crate) fn reason(
        &self,
        client: &str,
        folder: &FolderConfiguration,
    ) -> Result<Option<RootSuspension>, EngineError> {
        let reason: Option<Option<String>> = self.0.0.query_row("SELECT reason FROM bindings WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4", params![client,folder.account_id.0,folder.remote.id,folder.local.to_str().ok_or(EngineError::InvalidPlan)?], |r|r.get(0)).optional().map_err(failed)?;
        reason
            .flatten()
            .map(|s| serde_json::from_str(&s).map_err(failed))
            .transpose()
    }
    /// Updating availability never advances observations or baseline.
    pub(crate) fn suspend(
        &self,
        client: &str,
        folder: &FolderConfiguration,
        reason: Option<RootSuspension>,
    ) -> Result<(), EngineError> {
        let reason = reason
            .map(|r| serde_json::to_string(&r).map_err(failed))
            .transpose()?;
        let changed = self.0.0.execute("UPDATE bindings SET reason=?5 WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4", params![client,folder.account_id.0,folder.remote.id,folder.local.to_str().ok_or(EngineError::InvalidPlan)?,reason]).map_err(failed)?;
        if changed != 1 {
            return Err(EngineError::RootSuspended(
                RootSuspension::IdentityUnverified,
            ));
        }
        Ok(())
    }
}
