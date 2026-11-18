//! Post-scan decision coordination through public Inventory and Analyzer contracts.
//! No execution policy, direct Operator dispatch or SQL lives here.
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
    execution: Arc<super::execution::ExecutionControl>,
    observations: Vec<crate::models::File>,
) -> Result<(), PreparationFailure> {
    current(&epoch, expected)?;
    let provider = account_provider(&connector, &identity).await?;
    let analyzer = Analyzer::new(AnalyzerConfig {
        provider: provider.clone(),
        identity: identity.clone(),
        dependencies: AnalyzerDependencies {
            inventory: inventory.clone(),
            execution: super::execution::configure(
                execution.clone(),
                inventory.clone(),
                connector.clone(),
                epoch.clone(),
                expected,
            )?,
        },
    })
    .map_err(PreparationFailure::Analyzer)?;
    loop {
        let completed = execution.snapshot().completed;
        let mut pagination = Pagination::default();
        let mut items = Vec::new();
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
                let bound = inventory
                    .get_creation(GetCreationRequest {
                        item_id: item.id.clone(),
                    })
                    .map_err(PreparationFailure::Inventory)?
                    .is_some();
                items.push((!bound, item.kind != ItemKind::Directory, item.id));
            }
            let Some(after) = page.next_cursor else {
                break;
            };
            pagination.after = Some(after);
        }
        items.sort();
        let total = items.len() as u64;
        execution.update_analysis_progress(0, Some(total));
        let context = ItemAnalysis {
            analyzer: &analyzer,
            inventory: &inventory,
            identity: &identity,
            epoch: &epoch,
            expected,
            execution: &execution,
            observations: &observations,
        };
        for (index, (_, _, id)) in items.into_iter().enumerate() {
            analyze_item(&context, id, index as u64, Some(total)).await?;
            execution.update_analysis_progress(index as u64 + 1, Some(total));
        }
        if execution.snapshot().completed == completed {
            return Ok(());
        }
    }
}
pub(super) async fn analyze_selected(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    selected: SelectedAnalysisInput,
    connector: crate::Engine,
    execution: Arc<super::execution::ExecutionControl>,
) -> Result<(), PreparationFailure> {
    let provider = account_provider(&connector, &identity).await?;
    let analyzer = Analyzer::new(AnalyzerConfig {
        provider: provider.clone(),
        identity: identity.clone(),
        dependencies: AnalyzerDependencies {
            inventory: inventory.clone(),
            execution: super::execution::configure(
                execution.clone(),
                inventory.clone(),
                connector.clone(),
                epoch.clone(),
                expected,
            )?,
        },
    })
    .map_err(PreparationFailure::Analyzer)?;
    if execution.snapshot().enabled {
        return analyze_all(
            inventory,
            identity,
            epoch,
            expected,
            connector,
            execution,
            selected.observations,
        )
        .await;
    }
    let total = selected.ids.len() as u64;
    execution.update_analysis_progress(0, Some(total));
    let context = ItemAnalysis {
        analyzer: &analyzer,
        inventory: &inventory,
        identity: &identity,
        epoch: &epoch,
        expected,
        execution: &execution,
        observations: &selected.observations,
    };
    for (index, id) in selected.ids.into_iter().enumerate() {
        analyze_item(&context, id, index as u64, Some(total)).await?;
        execution.update_analysis_progress(index as u64 + 1, Some(total));
    }
    Ok(())
}

pub(super) struct SelectedAnalysisInput {
    pub(super) ids: std::collections::BTreeSet<SyncItemId>,
    pub(super) observations: Vec<crate::models::File>,
}

struct ItemAnalysis<'a> {
    analyzer: &'a Analyzer,
    inventory: &'a InventoryManager,
    identity: &'a SyncIdentity,
    epoch: &'a AtomicU64,
    expected: u64,
    execution: &'a super::execution::ExecutionControl,
    observations: &'a [crate::models::File],
}

async fn analyze_item(
    context: &ItemAnalysis<'_>,
    id: SyncItemId,
    processed: u64,
    total: Option<u64>,
) -> Result<(), PreparationFailure> {
    current(context.epoch, context.expected)?;
    let item = context
        .inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(id),
        })
        .map_err(PreparationFailure::Inventory)?
        .item
        .ok_or(PreparationFailure::Internal)?;
    if matches!(
        item.conflict_reason.as_deref(),
        Some("ReplicaObservationRetired" | "AssociationRetired")
    ) {
        return Ok(());
    }
    context.execution.publish_analysis(
        &item,
        crate::analyzer::AnalysisPhase::Validating,
        processed,
        total,
    );
    let evidence = evidence(
        context.inventory,
        &item,
        context.identity,
        context.epoch,
        context.expected,
    )?;
    current(context.epoch, context.expected)?;
    let item_id = item.id.clone();
    let current_item = item.clone();
    let analysis = context.analyzer.analyze(AnalyzeRequest {
        directive: None,
        observations: context.observations.to_vec(),
        sync_id: context.identity.sync_id.clone(),
        item,
        evidence,
        recorded_at: now(),
    });
    tokio::pin!(analysis);
    loop {
        tokio::select! {
            result = &mut analysis => {
                result.map_err(PreparationFailure::Analyzer)?;
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(40)) => {
                if let Ok(status) = context.analyzer.status(crate::analyzer::AnalysisStatusRequest {
                    item_id: item_id.clone(),
                }) {
                    if let Some(active) = status.active {
                        context.execution.publish_analysis(&current_item, active.phase, processed, total);
                    }
                }
                current(context.epoch, context.expected)?;
            }
        }
    }
    current(context.epoch, context.expected)
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
        let execution = self.execution.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("sync-analysis".into())
            .spawn(move || {
                let result = run_pass(
                    inventory, identity, epoch, expected, control, connector, execution,
                );
                let _ = tx.send(result);
            })
            .map_err(|_| PreparationFailure::Internal)?;
        tokio::pin!(rx);
        if tokio::runtime::Handle::try_current().is_ok() {
            loop {
                tokio::select! {
                    result = &mut rx => return result.map_err(|_| PreparationFailure::Internal)?,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        if let Ok(status) = self.detector.status(crate::detectors::DetectorStatusRequest {}) {
                            self.preparation.snapshot.publish_detector(&status.status);
                        }
                    }
                }
            }
        } else {
            rx.await.map_err(|_| PreparationFailure::Internal)?
        }
    }
}
fn run_pass(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    control: Arc<Mutex<()>>,
    connector: crate::Engine,
    execution: Arc<super::execution::ExecutionControl>,
) -> Result<(), PreparationFailure> {
    let _control = control.lock().map_err(|_| PreparationFailure::Internal)?;
    current(&epoch, expected)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| PreparationFailure::Internal)?;
    runtime.block_on(analyze_all(
        inventory,
        identity,
        epoch,
        expected,
        connector,
        execution,
        Vec::new(),
    ))
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

/// Revisit only identity-bound interrupted operations; no new intent is dispatched here.
pub(super) async fn retry_pending(
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    epoch: Arc<AtomicU64>,
    expected: u64,
    connector: crate::Engine,
    execution: Arc<super::execution::ExecutionControl>,
) -> Result<(), PreparationFailure> {
    current(&epoch, expected)?;
    let due = !inventory
        .due_creations(DueCreationsRequest {
            at: now(),
            limit: 64,
        })
        .map_err(PreparationFailure::Inventory)?
        .is_empty()
        || execution.probe_retry_due();
    if due {
        analyze_all(
            inventory,
            identity,
            epoch,
            expected,
            connector,
            execution,
            Vec::new(),
        )
        .await?;
    }
    Ok(())
}
