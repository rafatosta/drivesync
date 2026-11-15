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
    epoch: &AtomicU64,
    expected: u64,
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
    let root = inventory
        .get_sync_info(GetSyncInfoRequest {})
        .map_err(PreparationFailure::Inventory)?
        .local_root;
    let remote_hash = remote.and_then(|r| known_sha256(&r.sha256_checksum));
    let baseline_hash = item
        .baseline
        .as_ref()
        .and_then(|b| b.local.checksum.as_deref());
    let measurement_needed = remote_hash.is_some()
        || compare_hash(baseline_hash, baseline_hash) == EvidenceRelation::Equal;
    let local_hash = local
        .filter(|_| {
            measurement_needed
                && item.kind == ItemKind::File
                && item.local_presence == PresenceState::Present
        })
        .and_then(|file| {
            crate::filesystem::content::sha256(
                std::path::Path::new(
                    <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(&root),
                ),
                file,
                || epoch.load(Ordering::Acquire) != expected,
            )
        });
    Ok(AnalysisEvidence {
        baseline_confirmed: item.baseline.is_some(),
        // These describe the persisted item presence, not a complete remote tree
        // or fresh content measurement. Unknown sides remain explicitly unknown.
        local_scope_complete: item.local_presence != PresenceState::Unknown,
        remote_scope_complete: item.remote_presence != PresenceState::Unknown,
        local_content: compare_hash(
            local_hash.as_deref(),
            item.baseline
                .as_ref()
                .and_then(|b| b.local.checksum.as_deref()),
        ),
        remote_content: compare_hash(
            remote_hash.as_deref(),
            item.baseline
                .as_ref()
                .and_then(|b| b.remote.checksum.as_deref()),
        ),
        current_content: compare_hash(local_hash.as_deref(), remote_hash.as_deref()),
        local_parent: parent
            .filter(|p| p.kind == ItemKind::Directory && p.local_presence == PresenceState::Present)
            .map(|p| p.id),
        current_parents,
    })
}
fn known_sha256(value: &MetadataValue<String>) -> Option<String> {
    match value {
        MetadataValue::Known(v) if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Some(format!("sha256:{}", v.to_ascii_lowercase()))
        }
        _ => None,
    }
}
fn compare_hash(a: Option<&str>, b: Option<&str>) -> EvidenceRelation {
    match (a, b) {
        (Some(a), Some(b))
            if [a, b].iter().all(|s| {
                s.strip_prefix("sha256:")
                    .is_some_and(|v| v.len() == 64 && v.bytes().all(|c| c.is_ascii_hexdigit()))
            }) =>
        {
            if a.eq_ignore_ascii_case(b) {
                EvidenceRelation::Equal
            } else {
                EvidenceRelation::Different
            }
        }
        _ => EvidenceRelation::Unknown,
    }
}
/// Visits every persisted item, regardless of status: observations can invalidate
/// an old analysis without changing status. Keyset pages survive our revision bumps.
pub(super) async fn analyze_all(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    connector: crate::Engine,
) -> Result<(), PreparationFailure> {
    current(&epoch, expected)?;
    let provider = account_provider(&connector, &identity).await?;
    let analyzer = Analyzer::new(AnalyzerConfig {
        provider: provider.clone(),
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
    connector: crate::Engine,
) -> Result<(), PreparationFailure> {
    let provider = account_provider(&connector, &identity).await?;
    let analyzer = Analyzer::new(AnalyzerConfig {
        provider: provider.clone(),
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
    let evidence = evidence(inventory, &item, identity, epoch, expected)?;
    current(epoch, expected)?;
    analyzer
        .analyze(AnalyzeRequest {
            directive: None,
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
        let connector = self.connector.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("sync-analysis".into())
            .spawn(move || {
                let result = run_pass(inventory, identity, epoch, expected, control, connector);
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
    connector: crate::Engine,
) -> Result<(), PreparationFailure> {
    let _control = control.lock().map_err(|_| PreparationFailure::Internal)?;
    current(&epoch, expected)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| PreparationFailure::Internal)?;
    runtime.block_on(analyze_all(inventory, identity, epoch, expected, connector))
}

/// Resolves local persisted account metadata, without credentials or Provider calls.
async fn account_provider(
    connector: &crate::Engine,
    identity: &SyncIdentity,
) -> Result<crate::ProviderId, PreparationFailure> {
    connector
        .list_accounts(crate::ListAccountsRequest { provider: None })
        .await
        .map_err(|_| PreparationFailure::Internal)?
        .accounts
        .into_iter()
        .find(|a| a.id == identity.account_id)
        .map(|a| a.provider)
        .ok_or(PreparationFailure::Internal)
}
