//! Post-scan decision coordination through public Inventory and Analyzer contracts.
//! No event leases, execution commands, SQL, hashes or filesystem writes live here.
use super::{Orchestrator, PreparationFailure};
use crate::{
    analyzer::{AnalyzeRequest, Analyzer, AnalyzerConfig, AnalyzerDependencies},
    inventory::*,
    models::{MetadataValue, RemoteFileRef, Timestamp},
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn current(epoch: &AtomicU64, expected: u64) -> Result<(), PreparationFailure> {
    if epoch.load(Ordering::Acquire) != expected {
        Err(PreparationFailure::Internal)
    } else {
        Ok(())
    }
}
pub(super) fn now() -> Timestamp {
    let unix_millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(time) => time.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    };
    Timestamp { unix_millis }
}
/// Resolves parent identity only through inventory indexes and explicit root binding.
/// No name-based local/remote pairing or content comparison is inferred here.
fn evidence(
    inventory: &InventoryManager,
    item: &SyncItem,
    identity: &SyncIdentity,
) -> Result<AnalysisEvidence, PreparationFailure> {
    let local = item.local_latest.as_ref().or(item.local.as_ref());
    let parent_path = local.and_then(|file| {
        file.relative_path
            .as_bytes()
            .iter()
            .rposition(|b| *b == b'/')
            .map(|slash| file.relative_path.as_bytes()[..slash].to_vec())
    });
    let parent = parent_path
        .as_ref()
        .map(|path| {
            let relative = crate::models::LocalRelativePath::new(path.clone())
                .map_err(|_| PreparationFailure::Internal)?;
            inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::LocalRelativePath(relative),
                })
                .map_err(PreparationFailure::Inventory)
                .map(|result| result.item)
        })
        .transpose()?
        .flatten();
    let remote = item.remote_latest.as_ref().or(item.remote.as_ref());
    let remote_parent = remote.and_then(|file| match &file.parent_ids {
        MetadataValue::Known(ids) if ids.len() == 1 => Some(RemoteFileRef {
            account_id: file.reference.account_id.clone(),
            file_id: ids[0].clone(),
        }),
        _ => None,
    });
    let mapped_parent = if local.is_some() && parent_path.is_none() {
        Some(RemoteFileRef {
            account_id: identity.account_id.clone(),
            file_id: crate::models::RemoteFileId(identity.remote_root_id.clone()),
        })
    } else {
        parent
            .as_ref()
            .filter(|p| p.kind == ItemKind::Directory && p.local_presence == PresenceState::Present)
            .and_then(|p| p.remote_ref.clone())
    };
    let current_parents = match (mapped_parent, remote_parent) {
        (Some(a), Some(b)) if a == b => EvidenceRelation::Equal,
        // A configured root may be a provider alias (for example "root").
        // A textual mismatch cannot prove different logical roots without resolution.
        (Some(_), Some(_)) if parent_path.is_some() => EvidenceRelation::Different,
        _ => EvidenceRelation::Unknown,
    };
    Ok(AnalysisEvidence {
        baseline_confirmed: item.baseline.is_some(),
        // These describe the persisted item presence, not a complete remote tree
        // or fresh content measurement. Unknown sides remain explicitly unknown.
        local_scope_complete: item.local_presence != PresenceState::Unknown,
        remote_scope_complete: item.remote_presence != PresenceState::Unknown,
        local_content: EvidenceRelation::Unknown,
        remote_content: EvidenceRelation::Unknown,
        current_content: EvidenceRelation::Unknown,
        local_parent: parent
            .filter(|p| p.kind == ItemKind::Directory && p.local_presence == PresenceState::Present)
            .map(|p| p.id),
        current_parents,
    })
}
/// Visits every persisted item, regardless of status: observations can invalidate
/// an old analysis without changing status. Keyset pages survive our revision bumps.
pub(super) async fn analyze_all(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
) -> Result<(), PreparationFailure> {
    current(&epoch, expected)?;
    let analyzer = Analyzer::new(AnalyzerConfig {
        identity: identity.clone(),
        dependencies: AnalyzerDependencies {
            inventory: inventory.clone(),
        },
    })
    .map_err(PreparationFailure::Analyzer)?;
    let mut pagination = Pagination::default();
    loop {
        current(&epoch, expected)?;
        let page = inventory
            .list_items(ListItemsRequest {
                filter: ItemFilter::default(),
                pagination: pagination.clone(),
            })
            .map_err(PreparationFailure::Inventory)?
            .page;
        for item in page.entries {
            current(&epoch, expected)?;
            analyze_item(&analyzer, &inventory, &identity, &epoch, expected, item.id).await?;
        }
        let Some(after) = page.next_cursor else {
            return Ok(());
        };
        pagination.after = Some(after);
    }
}
pub(super) async fn analyze_selected(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    ids: std::collections::BTreeSet<SyncItemId>,
) -> Result<(), PreparationFailure> {
    let analyzer = Analyzer::new(AnalyzerConfig {
        identity: identity.clone(),
        dependencies: AnalyzerDependencies {
            inventory: inventory.clone(),
        },
    })
    .map_err(PreparationFailure::Analyzer)?;
    for id in ids {
        analyze_item(&analyzer, &inventory, &identity, &epoch, expected, id).await?;
    }
    Ok(())
}
async fn analyze_item(
    analyzer: &Analyzer,
    inventory: &InventoryManager,
    identity: &SyncIdentity,
    epoch: &AtomicU64,
    expected: u64,
    id: SyncItemId,
) -> Result<(), PreparationFailure> {
    current(epoch, expected)?;
    let item = inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(id),
        })
        .map_err(PreparationFailure::Inventory)?
        .item
        .ok_or(PreparationFailure::Internal)?;
    let evidence = evidence(inventory, &item, identity)?;
    current(epoch, expected)?;
    analyzer
        .analyze(AnalyzeRequest {
            sync_id: identity.sync_id.clone(),
            item,
            evidence,
            recorded_at: now(),
        })
        .await
        .map_err(PreparationFailure::Analyzer)?;
    current(epoch, expected)
}
impl Orchestrator {
    /// Keeps runtime setup and all Inventory calls off callers such as GLib.
    /// One finite pass, not a background event consumer or a transfer scheduler.
    pub(super) async fn analyze_inventory(
        &self,
        inventory: Arc<InventoryManager>,
        expected: u64,
    ) -> Result<(), PreparationFailure> {
        let scope = self.detector.scope();
        let identity = SyncIdentity {
            sync_id: scope.sync_id.clone(),
            account_id: scope.account_id.clone(),
            remote_root_id: scope.remote_root_id.clone(),
        };
        let epoch = self.preparation.epoch.clone();
        let control = self.preparation.control.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("sync-analysis".into())
            .spawn(move || {
                let result = run_pass(inventory, identity, epoch, expected, control);
                let _ = tx.send(result);
            })
            .map_err(|_| PreparationFailure::Internal)?;
        rx.await.map_err(|_| PreparationFailure::Internal)?
    }
}
fn run_pass(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    control: Arc<Mutex<()>>,
) -> Result<(), PreparationFailure> {
    let _control = control.lock().map_err(|_| PreparationFailure::Internal)?;
    current(&epoch, expected)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| PreparationFailure::Internal)?;
    runtime.block_on(analyze_all(inventory, identity, epoch, expected))
}
