//! Read-only, bounded developer inspection of approved inventory tables.
//! This diagnostic API never exposes SQL/connections and is separate from the domain facade.
use super::*;
use rusqlite::types::ValueRef;
use std::{os::fd::AsRawFd, path::PathBuf};

/// Maximum records returned per diagnostic page; cells also have bounded previews.
pub const PAGE_SIZE: usize = 100;
const PREVIEW_BYTES: usize = 4096;

/// Explicit allowlist excludes SQLite internals and prevents arbitrary SQL identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryTable {
    SyncInfo,
    Items,
    LocalFiles,
    RemoteFiles,
    RemoteParents,
    MetadataFields,
    Receipts,
    Operations,
    Baselines,
    Activities,
    Confirmations,
    Analyses,
    AnalysisFindings,
    AnalysisRequirements,
    AnalysisDecisions,
    AnalysisPlanOrder,
    AnalysisPlanRequests,
    AnalysisPlanNamespaces,
    ItemAssociations,
    AnalysisPlans,
    AnalysisPlanSteps,
    AnalysisPlanBlocks,
    AnalysisPlanDependencies,
    AnalysisPlanGuards,
    AnalysisPlanManual,
    AnalysisPlanResources,
}
impl InventoryTable {
    /// All supported tables in presentation order.
    pub const ALL: [Self; 26] = [
        Self::SyncInfo,
        Self::Items,
        Self::LocalFiles,
        Self::RemoteFiles,
        Self::RemoteParents,
        Self::MetadataFields,
        Self::Receipts,
        Self::Operations,
        Self::Baselines,
        Self::Activities,
        Self::Confirmations,
        Self::Analyses,
        Self::AnalysisFindings,
        Self::AnalysisRequirements,
        Self::AnalysisDecisions,
        Self::AnalysisPlanOrder,
        Self::AnalysisPlanRequests,
        Self::AnalysisPlanNamespaces,
        Self::ItemAssociations,
        Self::AnalysisPlans,
        Self::AnalysisPlanSteps,
        Self::AnalysisPlanBlocks,
        Self::AnalysisPlanDependencies,
        Self::AnalysisPlanGuards,
        Self::AnalysisPlanManual,
        Self::AnalysisPlanResources,
    ];
    /// Physical name for diagnostic presentation; not a caller-controlled SQL fragment.
    pub fn name(self) -> &'static str {
        match self {
            Self::SyncInfo => "sync_info",
            Self::Items => "items",
            Self::LocalFiles => "local_files",
            Self::RemoteFiles => "remote_files",
            Self::RemoteParents => "remote_parents",
            Self::MetadataFields => "metadata_fields",
            Self::Receipts => "receipts",
            Self::Operations => "operations",
            Self::Baselines => "baselines",
            Self::Activities => "activities",
            Self::Confirmations => "confirmations",
            Self::Analyses => "analyses",
            Self::AnalysisFindings => "analysis_findings",
            Self::AnalysisRequirements => "analysis_requirements",
            Self::AnalysisDecisions => "analysis_decisions",
            Self::AnalysisPlanOrder => "analysis_plan_order",
            Self::AnalysisPlanRequests => "analysis_plan_requests",
            Self::AnalysisPlanNamespaces => "analysis_plan_namespaces",
            Self::ItemAssociations => "item_associations",
            Self::AnalysisPlans => "analysis_plans",
            Self::AnalysisPlanSteps => "analysis_plan_steps",
            Self::AnalysisPlanBlocks => "analysis_plan_blocks",
            Self::AnalysisPlanDependencies => "analysis_plan_dependencies",
            Self::AnalysisPlanGuards => "analysis_plan_guards",
            Self::AnalysisPlanManual => "analysis_plan_manual",
            Self::AnalysisPlanResources => "analysis_plan_resources",
        }
    }
}
/// Selected configured scope. The persisted SyncId is returned, never guessed from a detector session.
#[derive(Debug, Clone)]
pub struct InspectInventoryRequest {
    pub local_root: PathBuf,
    pub account_id: AccountId,
    pub remote_root_id: String,
    pub table: InventoryTable,
    /// Exclusive physical row key from the preceding page; zero starts a fresh inspection.
    pub after_rowid: i64,
}
/// SQLite value category, distinguishing NULL, text and binary without lossy decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Null,
    Integer,
    Real,
    Text,
    BlobText,
    BlobHex,
}
/// Bounded display preview. Length is the original byte count, not a character count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionCell {
    pub kind: CellKind,
    pub display: String,
    pub byte_length: usize,
    pub truncated: bool,
}
/// One consistent read transaction. Further pages are live reads, not a historical snapshot.
#[derive(Debug, Clone)]
pub struct InspectionPage {
    pub info: SyncInfo,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<InspectionCell>>,
    pub next_after: Option<i64>,
}
fn text_prefix(bytes: &[u8]) -> InventoryResult<(String, bool)> {
    let text = std::str::from_utf8(bytes).map_err(|_| failure(InventoryError::CorruptDatabase))?;
    let mut end = text.len().min(PREVIEW_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Ok((text[..end].to_owned(), end < bytes.len()))
}
fn cell(value: ValueRef<'_>) -> InventoryResult<InspectionCell> {
    let (kind, display, byte_length, truncated) = match value {
        ValueRef::Null => (CellKind::Null, "NULL".into(), 0, false),
        ValueRef::Integer(v) => (CellKind::Integer, v.to_string(), 8, false),
        ValueRef::Real(v) => (CellKind::Real, v.to_string(), 8, false),
        ValueRef::Text(bytes) => {
            let (text, truncated) = text_prefix(bytes)?;
            (CellKind::Text, text, bytes.len(), truncated)
        }
        ValueRef::Blob(bytes) => {
            if std::str::from_utf8(bytes)
                .is_ok_and(|s| !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'))
            {
                let (text, truncated) = text_prefix(bytes)?;
                (CellKind::BlobText, text, bytes.len(), truncated)
            } else {
                let end = bytes.len().min(PREVIEW_BYTES / 2);
                let hex = bytes[..end]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                (CellKind::BlobHex, hex, bytes.len(), end < bytes.len())
            }
        }
    };
    Ok(InspectionCell {
        kind,
        display,
        byte_length,
        truncated,
    })
}
/// Reads an existing inventory using SQLITE_OPEN_READ_ONLY and query_only.
/// Validates account, remote root, local root and schema within the read transaction.
/// Missing databases remain missing; incompatible/corrupt/busy databases return typed errors.
/// Blocking: call off the GTK main loop. No observation, initialization, migration or recovery occurs.
pub fn inspect(request: InspectInventoryRequest) -> InventoryResult<InspectionPage> {
    if request.after_rowid < 0
        || request.account_id.0.trim().is_empty()
        || request.remote_root_id.trim().is_empty()
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    let mut opening = OpenInventoryRequest {
        local_root: request.local_root,
        expected: SyncIdentity {
            sync_id: SyncId(String::new()),
            account_id: request.account_id,
            remote_root_id: request.remote_root_id,
        },
        create_if_missing: false,
    };
    let (directory, _file, _) = filesystem::open_readonly(&opening)?;
    let path = format!("/proc/self/fd/{}/inventory.sqlite3", directory.as_raw_fd());
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(sql)?;
    db.busy_timeout(std::time::Duration::from_millis(1000))
        .map_err(sql)?;
    db.pragma_update(None, "query_only", "ON").map_err(sql)?;
    db.pragma_update(None, "trusted_schema", "OFF")
        .map_err(sql)?;
    let tx = db.transaction().map_err(sql)?;
    // Read the owned identity only after checking the exact schema version.
    let version: u32 = tx
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(sql)?;
    if version != 5 {
        return Err(failure(InventoryError::UnsupportedSchema));
    }
    let info = sync_info(&tx)?;
    if info.sync_id.0.trim().is_empty() {
        return Err(failure(InventoryError::InvalidIdentity));
    }
    opening.expected.sync_id = info.sync_id.clone();
    verify(&tx, &opening)?;
    let mut statement = tx
        .prepare(&format!(
            "SELECT rowid,* FROM {} WHERE rowid>?1 ORDER BY rowid LIMIT ?2",
            request.table.name()
        ))
        .map_err(sql)?;
    let columns = statement
        .column_names()
        .into_iter()
        .skip(1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut query = statement
        .query(rusqlite::params![
            request.after_rowid,
            (PAGE_SIZE + 1) as i64
        ])
        .map_err(sql)?;
    let mut rows = Vec::new();
    let mut last = 0;
    let mut more = false;
    while let Some(row) = query.next().map_err(sql)? {
        if rows.len() == PAGE_SIZE {
            more = true;
            break;
        }
        last = row.get::<_, i64>(0).map_err(sql)?;
        let values = (1..=columns.len())
            .map(|index| cell(row.get_ref(index).map_err(sql)?))
            .collect::<InventoryResult<_>>()?;
        rows.push(values);
    }
    drop(query);
    drop(statement);
    tx.commit().map_err(sql)?;
    Ok(InspectionPage {
        info,
        columns,
        rows,
        next_after: more.then_some(last),
    })
}
