//! Resolution intentions and atomic final baselines in the operation database.
use super::{failed, Store};
use crate::{
    resolution::{Intent, Stage},
    DownloadPlan, EngineError,
};
use rusqlite::{params, Transaction, TransactionBehavior};

fn scope<'a>(plan: &'a DownloadPlan, client: &str) -> Result<&'a str, EngineError> {
    if client.is_empty()
        || plan.account_id.0.is_empty()
        || !crate::configuration::valid_id(&plan.remote_root)
        || !plan.local_root.is_absolute()
    {
        return Err(EngineError::InvalidPlan);
    }
    plan.local_root.to_str().ok_or(EngineError::InvalidPlan)
}
impl Store {
    /// Loads all intentions, including confirmed records for idempotent decisions.
    /// Invalid evidence is never silently excluded from the recovery work list.
    pub(crate) fn resolutions(
        &self,
        client: &str,
        plan: &DownloadPlan,
    ) -> Result<Vec<Intent>, EngineError> {
        let local = scope(plan, client)?;
        let mut statement = self.0.prepare("SELECT file,record FROM resolutions WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 ORDER BY file").map_err(failed)?;
        let rows = statement
            .query_map(
                params![client, plan.account_id.0, plan.remote_root, local],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(failed)?;
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for row in rows {
            let (file, json) = row.map_err(failed)?;
            bytes = bytes.saturating_add(json.len());
            if bytes > 64 * 1024 * 1024 || result.len() >= 100_000 {
                return Err(EngineError::StateStorage);
            }
            let intent: Intent = serde_json::from_str(&json).map_err(failed)?;
            if !intent.valid() || intent.plan.remote.item.id != file {
                return Err(EngineError::StateStorage);
            }
            result.push(intent);
        }
        Ok(result)
    }

    /// Persists an exact-plan CAS before the next effect. Replacing an unfinished
    /// decision or regressing a durable boundary is rejected even across workers.
    pub(crate) fn save_resolution(
        &self,
        client: &str,
        plan: &DownloadPlan,
        expected: Option<&Intent>,
        next: &Intent,
    ) -> Result<(), EngineError> {
        let local = scope(plan, client)?;
        if !next.valid() {
            return Err(EngineError::InvalidPlan);
        }
        if let Some(old) = expected {
            if old.plan.remote.item.id != next.plan.remote.item.id
                || (old.id != next.id
                    && (!matches!(old.stage, Stage::Confirmed | Stage::Aborted)
                        || next.stage != Stage::Preparing))
                || (old.id == next.id
                    && (old.plan != next.plan
                        || old.root != next.root
                        || old.parent != next.parent
                        || (old.stage > next.stage
                            && !(next.stage == Stage::Aborted && old.stage <= Stage::Preserved))
                        || old.retry.attempts > next.retry.attempts
                        || old.retry.not_before > next.retry.not_before))
            {
                return Err(EngineError::InvalidPlan);
            }
        }
        let json = serde_json::to_string(next).map_err(failed)?;
        let tx =
            Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate).map_err(failed)?;
        let changed = if let Some(old) = expected {
            tx.execute("UPDATE resolutions SET record=?6 WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND file=?5 AND record=?7",params![client,plan.account_id.0,plan.remote_root,local,next.plan.remote.item.id,json,serde_json::to_string(old).map_err(failed)?]).map_err(failed)?
        } else {
            tx.execute(
                "INSERT INTO resolutions VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT DO NOTHING",
                params![
                    client,
                    plan.account_id.0,
                    plan.remote_root,
                    local,
                    next.plan.remote.item.id,
                    json
                ],
            )
            .map_err(failed)?
        };
        if changed != 1 {
            return Err(EngineError::StateStorage);
        }
        tx.commit().map_err(failed)
    }

    /// Commits both active-file baselines and the intention's terminal state in
    /// one SQLite transaction, only after all remote/local effects were verified.
    pub(crate) fn confirm_resolution(
        &self,
        client: &str,
        plan: &DownloadPlan,
        intent: &Intent,
        parent: crate::download::Identity,
    ) -> Result<Intent, EngineError> {
        if intent.stage != Stage::Verified || !intent.valid() {
            return Err(EngineError::InvalidPlan);
        }
        let local = scope(plan, client)?;
        let tx =
            Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate).map_err(failed)?;
        let mut confirmed = intent.clone();
        confirmed.stage = Stage::Confirmed;
        let changed=tx.execute("UPDATE resolutions SET record=?6 WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND file=?5 AND record=?7",params![client,plan.account_id.0,plan.remote_root,local,intent.plan.remote.item.id,serde_json::to_string(&confirmed).map_err(failed)?,serde_json::to_string(intent).map_err(failed)?]).map_err(failed)?;
        if changed != 1 {
            return Err(EngineError::StateStorage);
        }
        let result = intent.result.as_ref().ok_or(EngineError::StateStorage)?;
        self.put_operation(
            client,
            plan,
            &crate::download::Operation {
                id: format!("resolution-{}", intent.id),
                item: result.item.clone(),
                relative: intent.plan.target.clone(),
                temporary: intent.temporary("result"),
                root: intent.root,
                parent,
                phase: crate::download::Phase::Installed {
                    identity: intent.installed.ok_or(EngineError::StateStorage)?,
                    sha256: intent.winner()?.sha256.clone(),
                },
            },
        )?;
        if let Some(copy) = &intent.copy_result {
            let path = intent.plan.target.with_file_name(
                intent
                    .plan
                    .copy_name
                    .as_ref()
                    .ok_or(EngineError::StateStorage)?,
            );
            self.put_operation(
                client,
                plan,
                &crate::download::Operation {
                    id: format!("resolution-copy-{}", intent.id),
                    item: copy.item.clone(),
                    relative: path,
                    temporary: intent.temporary("copy"),
                    root: intent.root,
                    parent,
                    phase: crate::download::Phase::Installed {
                        identity: intent.copy_installed.ok_or(EngineError::StateStorage)?,
                        sha256: intent
                            .remote
                            .as_ref()
                            .ok_or(EngineError::StateStorage)?
                            .sha256
                            .clone(),
                    },
                },
            )?;
        }
        tx.commit().map_err(failed)?;
        Ok(confirmed)
    }
}
