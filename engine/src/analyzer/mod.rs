//! Revision-checked coordination of pure algorithms and durable inventory receipts.
//!
//! Optional operation execution calls Operator only after live prerequisites;
//! conflicts and unresolved deletion authorization remain blocked decisions.
//! SQLite access goes through InventoryManager on a blocking worker, never the
//! async caller's thread. Algorithms and persistence mappings remain private.
//!
//! ```no_run
//! use std::sync::Arc;
//! use drivesync_engine::{analyzer::*, inventory::*, models::Timestamp};
//! # async fn example(inventory: Arc<InventoryManager>, identity: SyncIdentity,
//! # item: SyncItem, evidence: AnalysisEvidence, recorded_at: Timestamp)
//! # -> AnalyzerResult<()> {
//! let analyzer = Analyzer::new(AnalyzerConfig { provider: drivesync_engine::models::ProviderId("google".into()),
//!     identity: identity.clone(),
//!     dependencies: AnalyzerDependencies { inventory, execution: None },
//! })?;
//! let outcome = analyzer.analyze(AnalyzeRequest { directive: None, observations: vec![],
//!     sync_id: identity.sync_id, item, evidence, recorded_at,
//! }).await?;
//! let durable_receipt = outcome.analysis.id;
//! # let _ = durable_receipt;
//! # Ok(())
//! # }
//! ```
//!
//! Algorithms cannot be called by consumers of the facade:
//! ```compile_fail
//! use drivesync_engine::analyzer::conflicts::evaluate;
//! ```
//! ```compile_fail
//! use drivesync_engine::analyzer::reconciliation::reconcile;
//! ```
mod conflicts;
mod execution;
mod models;
mod persistence;
mod planning;
mod reconciliation;
pub use models::*;

use crate::inventory::{
    AnalysisSelector, GetAnalysisRequest, GetItemRequest, GetSyncInfoRequest, InventoryError,
    InventoryFailure, InventoryManager, ItemSelector, RecordAnalysisRequest, SyncIdentity,
    SyncItemId,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// Scoped facade with transient per-item activity. Durable state belongs to Inventory.
pub struct Analyzer {
    inventory: Arc<InventoryManager>,
    identity: SyncIdentity,
    provider: crate::models::ProviderId,
    execution: Option<AnalyzerExecution>,
    planning_cache: Arc<planning::Cache>,
    active: Arc<Mutex<BTreeMap<SyncItemId, ActiveAnalysis>>>,
}
fn error(kind: AnalyzerErrorKind) -> AnalyzerError {
    AnalyzerError {
        kind,
        inventory_error: None,
    }
}
fn inventory_error(cause: InventoryFailure) -> AnalyzerError {
    AnalyzerError {
        kind: match cause.kind {
            InventoryError::VersionMismatch => AnalyzerErrorKind::StaleItem,
            InventoryError::InvalidIdentity => AnalyzerErrorKind::WrongSyncScope,
            _ => AnalyzerErrorKind::InventoryFailure,
        },
        inventory_error: Some(cause.kind),
    }
}
/// The worker owns the reservation even if its awaiting future is dropped.
/// SQLite may still commit after cancellation; a retry recovers the durable receipt.
struct Reservation {
    active: Arc<Mutex<BTreeMap<SyncItemId, ActiveAnalysis>>>,
    item_id: SyncItemId,
}
impl Reservation {
    fn phase(&self, phase: AnalysisPhase) {
        if let Some(value) = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&self.item_id)
        {
            value.phase = phase;
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.item_id);
    }
}
impl Analyzer {
    /// Validates the configured inventory identity without starting an analysis.
    /// Blocking: construct off the GTK main loop; no database is opened or created here.
    pub fn new(config: AnalyzerConfig) -> AnalyzerResult<Self> {
        if config.provider.0.trim().is_empty()
            || config.identity.sync_id.0.trim().is_empty()
            || config.identity.account_id.0.trim().is_empty()
            || config.identity.remote_root_id.trim().is_empty()
        {
            return Err(error(AnalyzerErrorKind::InvalidConfiguration));
        }
        let info = config
            .dependencies
            .inventory
            .get_sync_info(GetSyncInfoRequest {})
            .map_err(inventory_error)?
            .info;
        if info.sync_id != config.identity.sync_id
            || info.account_id != config.identity.account_id
            || info.remote_root_id != config.identity.remote_root_id
        {
            return Err(error(AnalyzerErrorKind::WrongSyncScope));
        }
        Ok(Self {
            execution: config.dependencies.execution,
            inventory: config.dependencies.inventory,
            identity: config.identity,
            provider: config.provider,
            planning_cache: Arc::new(Mutex::new(None)),
            active: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }
    /// Evaluates one item and atomically persists facts and blocked work intents.
    /// Requires a Tokio runtime. Different items can progress independently;
    /// overlapping calls for this item are rejected locally, with revision checks
    /// providing database consistency across instances. Configured execution may
    /// create a replica; only live post-execution verification confirms a baseline.
    pub async fn analyze(&self, request: AnalyzeRequest) -> AnalyzerResult<AnalysisOutcome> {
        if request.sync_id != self.identity.sync_id {
            return Err(error(AnalyzerErrorKind::WrongSyncScope));
        }
        if request.item.id.0.trim().is_empty() || request.item.revision == 0 {
            return Err(error(AnalyzerErrorKind::InvalidRequest));
        }
        if request.observations.iter().any(|observation| {
            observation.sync_id != request.sync_id || observation.validate().is_err()
        }) {
            return Err(error(AnalyzerErrorKind::InvalidRequest));
        }
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| error(AnalyzerErrorKind::RuntimeUnavailable))?;
        let guard = self.reserve(&request.item.id, request.item.revision)?;
        let inventory = self.inventory.clone();
        let provider = self.provider.clone();
        let planning_cache = self.planning_cache.clone();
        let execution = self.execution.clone();
        let worker_runtime = runtime.clone();
        runtime
            .spawn_blocking(move || {
                analyze_one(
                    &inventory,
                    &provider,
                    &planning_cache,
                    request,
                    guard,
                    execution.as_ref(),
                    &worker_runtime,
                )
            })
            .await
            .map_err(|_| error(AnalyzerErrorKind::Internal))?
    }
    /// Reads a coherent local snapshot without SQL, awaiting work or calling algorithms.
    /// Inactive valid IDs return None; they are not interpreted as synchronized.
    pub fn status(&self, request: AnalysisStatusRequest) -> AnalyzerResult<AnalysisStatusResult> {
        if request.item_id.0.trim().is_empty() {
            return Err(error(AnalyzerErrorKind::InvalidRequest));
        }
        Ok(AnalysisStatusResult {
            active: self
                .active
                .lock()
                .map_err(|_| error(AnalyzerErrorKind::Internal))?
                .get(&request.item_id)
                .cloned(),
        })
    }
    fn reserve(&self, id: &SyncItemId, revision: u64) -> AnalyzerResult<Reservation> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| error(AnalyzerErrorKind::Internal))?;
        if active.contains_key(id) {
            return Err(error(AnalyzerErrorKind::AlreadyRunning));
        }
        active.insert(
            id.clone(),
            ActiveAnalysis {
                item_id: id.clone(),
                item_revision: revision,
                phase: AnalysisPhase::Validating,
            },
        );
        Ok(Reservation {
            active: self.active.clone(),
            item_id: id.clone(),
        })
    }
}
fn analyze_one(
    inventory: &InventoryManager,
    provider: &crate::models::ProviderId,
    planning_cache: &planning::Cache,
    request: AnalyzeRequest,
    guard: Reservation,
    executor: Option<&AnalyzerExecution>,
    runtime: &tokio::runtime::Handle,
) -> AnalyzerResult<AnalysisOutcome> {
    if request
        .item
        .remote_latest
        .as_ref()
        .or(request.item.remote.as_ref())
        .is_some_and(|r| r.provider != *provider)
    {
        return Err(error(AnalyzerErrorKind::WrongSyncScope));
    }
    let current = inventory
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(request.item.id.clone()),
        })
        .map_err(inventory_error)?
        .item
        .ok_or_else(|| error(AnalyzerErrorKind::InvalidRequest))?;
    if current != request.item {
        // A lost reply may leave the exact input's durable receipt behind. Return
        // it with current staleness; never rerun an old intent to make it current.
        if let Some(receipt) = inventory
            .get_analysis(GetAnalysisRequest {
                selector: AnalysisSelector::LatestForItem(request.item.id.clone()),
            })
            .map_err(inventory_error)?
            .analysis
        {
            let bytes = serde_json::to_vec(&request.item)
                .map_err(|_| error(AnalyzerErrorKind::InvalidRequest))?;
            if receipt.analyzed_item_revision == request.item.revision
                && receipt.input_fingerprint == Sha256::digest(bytes)[..]
                && receipt.evidence == request.evidence
                && receipt
                    .plan
                    .as_ref()
                    .and_then(|p| p.restore_request.as_ref())
                    == request.directive.as_ref()
            {
                return Ok(AnalysisOutcome { analysis: receipt });
            }
        }
        return Err(error(AnalyzerErrorKind::StaleItem));
    }
    if let Some(executor) = executor {
        if let Some(binding) = inventory
            .get_creation(crate::inventory::GetCreationRequest {
                item_id: current.id.clone(),
            })
            .map_err(inventory_error)?
        {
            if binding.operation.state != crate::inventory::OperationState::Completed {
                runtime.block_on(execution::resume(
                    inventory,
                    executor,
                    &current,
                    binding,
                    request.recorded_at,
                    &guard,
                ))?;
                if let Some(analysis) = inventory
                    .get_analysis(GetAnalysisRequest {
                        selector: AnalysisSelector::LatestForItem(current.id.clone()),
                    })
                    .map_err(inventory_error)?
                    .analysis
                {
                    return Ok(AnalysisOutcome { analysis });
                }
            }
        }
    }
    guard.phase(AnalysisPhase::Reconciling);
    let reconciliation = persistence::reconcile(&request)?;
    guard.phase(AnalysisPhase::EvaluatingConflict);
    let conflicts = persistence::conflicts(&request)?;
    let mut assessment = persistence::assessment(reconciliation, conflicts)?;
    guard.phase(AnalysisPhase::Planning);
    let info = inventory
        .get_sync_info(GetSyncInfoRequest {})
        .map_err(inventory_error)?
        .info;
    planning::build(
        inventory,
        &SyncIdentity {
            sync_id: info.sync_id,
            account_id: info.account_id,
            remote_root_id: info.remote_root_id,
        },
        provider,
        planning_cache,
        &request,
        &mut assessment,
    )?;
    let candidate = if let Some(executor) = executor {
        runtime.block_on(execution::verify_candidate(
            executor,
            &request.item,
            &mut assessment,
        ))
    } else {
        None
    };
    guard.phase(AnalysisPhase::Recording);
    let analysis = inventory
        .record_analysis(RecordAnalysisRequest {
            expected_sync_id: request.sync_id.clone(),
            expected_item: request.item.clone(),
            evidence: request.evidence,
            assessment,
            recorded_at: request.recorded_at,
        })
        .map_err(inventory_error)?
        .analysis;
    if let (Some(executor), Some((context, command))) = (executor, candidate) {
        runtime.block_on(execution::start(
            inventory,
            executor,
            &analysis,
            &request.item,
            (context, command),
            request.recorded_at,
            &guard,
        ))?;
    }
    if analysis.disposition == crate::inventory::AnalysisDisposition::NoChanges {
        if let Some(executor) = executor {
            let current = inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::Id(request.item.id.clone()),
                })
                .map_err(inventory_error)?
                .item
                .ok_or_else(|| error(AnalyzerErrorKind::InvalidRequest))?;
            runtime.block_on(execution::reconfirm(inventory, executor, &current, &guard))?;
        }
    }
    Ok(AnalysisOutcome { analysis })
}
