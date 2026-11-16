//! Read-only plan materialization using public Inventory and Operator data contracts.
//! No Operator instance is created or called; missing inputs stay explicit blocks.
use super::*;
use crate::{inventory::*, models::*, operations::*};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    sync::{Arc, Mutex},
};
fn known<T>(v: &MetadataValue<T>) -> Option<&T> {
    if let MetadataValue::Known(v) = v {
        Some(v)
    } else {
        None
    }
}
fn local(i: &SyncItem) -> Option<&LocalFile> {
    i.local_latest.as_ref().or(i.local.as_ref())
}
fn remote(i: &SyncItem) -> Option<&RemoteFile> {
    i.remote_latest.as_ref().or(i.remote.as_ref())
}
fn push<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x)
    }
}
fn lref(i: &SyncItem) -> Option<ResourceRef> {
    local(i).map(|f| ResourceRef::Local {
        relative_path: f.relative_path.clone(),
        local_id: i.local_file_id.clone(),
    })
}
fn rref(i: &SyncItem) -> Option<ResourceRef> {
    remote(i).map(|f| ResourceRef::Remote {
        provider_id: f.provider.clone(),
        reference: f.reference.clone(),
    })
}
fn refs(i: &SyncItem) -> Vec<ResourceRef> {
    [lref(i), rref(i)].into_iter().flatten().collect()
}
fn name(i: &SyncItem, upload: bool) -> Option<ItemName> {
    if upload {
        let bytes = local(i)?
            .relative_path
            .as_bytes()
            .rsplit(|b| *b == b'/')
            .next()?;
        std::str::from_utf8(bytes)
            .ok()
            .map(|s| ItemName::Remote(s.into()))
    } else {
        let n = known(&remote(i)?.name)?;
        if n.is_empty()
            || n.contains(['\0', '/'])
            || matches!(n.as_str(), "." | ".." | ".drivesync")
            || n.len() > 255
        {
            return None;
        }
        Some(ItemName::Local(OsString::from(n)))
    }
}
fn parent_path(i: &SyncItem) -> Option<Vec<u8>> {
    let p = local(i)?.relative_path.as_bytes();
    Some(
        p.iter()
            .rposition(|b| *b == b'/')
            .map(|n| p[..n].to_vec())
            .unwrap_or_default(),
    )
}
fn add_guard(plan: &mut AnalysisPlan, item: &SyncItem) -> AnalyzerResult<()> {
    push(
        &mut plan.guards,
        ItemGuard::for_item(item).map_err(inventory_error)?,
    );
    Ok(())
}
fn all(inventory: &InventoryManager) -> AnalyzerResult<Vec<SyncItem>> {
    let mut out = Vec::new();
    let mut pagination = Pagination::default();
    loop {
        let page = inventory
            .list_items(ListItemsRequest {
                filter: ItemFilter::default(),
                pagination: pagination.clone(),
            })
            .map_err(inventory_error)?
            .page;
        out.extend(page.entries);
        match page.next_cursor {
            Some(next) => pagination.after = Some(next),
            None => return Ok(out),
        }
    }
}
/// Shared immutable context for one observation epoch. Diagnostic-only analysis
/// writes do not invalidate it; changed facts force a fresh indexed snapshot.
pub(super) struct Snapshot {
    guard: NamespaceGuard,
    items: Vec<SyncItem>,
    locals: BTreeMap<Vec<u8>, usize>,
    remotes: BTreeMap<(AccountId, String), usize>,
    homonyms: BTreeMap<(AccountId, String, String), Vec<usize>>,
}
pub(super) type Cache = Mutex<Option<Arc<Snapshot>>>;
fn snapshot(
    inventory: &InventoryManager,
    provider: &ProviderId,
    cache: &Cache,
) -> AnalyzerResult<Arc<Snapshot>> {
    let mut cached = cache
        .lock()
        .map_err(|_| error(AnalyzerErrorKind::Internal))?;
    for _ in 0..3 {
        let before = inventory
            .get_namespace(GetNamespaceRequest {
                directory: DirectoryRef::LocalRoot,
            })
            .map_err(inventory_error)?
            .guard;
        if let Some(current) = cached.as_ref().filter(|c| c.guard == before) {
            return Ok(current.clone());
        }
        let items = all(inventory)?;
        let after = inventory
            .get_namespace(GetNamespaceRequest {
                directory: DirectoryRef::LocalRoot,
            })
            .map_err(inventory_error)?
            .guard;
        if before != after {
            continue;
        }
        let mut data = Snapshot {
            guard: before,
            items,
            locals: BTreeMap::new(),
            remotes: BTreeMap::new(),
            homonyms: BTreeMap::new(),
        };
        for (index, item) in data.items.iter().enumerate() {
            if let Some(f) = local(item) {
                data.locals
                    .insert(f.relative_path.as_bytes().to_vec(), index);
            }
            if let Some(r) = remote(item) {
                if r.provider != *provider {
                    return Err(error(AnalyzerErrorKind::WrongSyncScope));
                }
                data.remotes.insert(
                    (
                        r.reference.account_id.clone(),
                        r.reference.file_id.0.clone(),
                    ),
                    index,
                );
                if item.local.is_none()
                    && item.local_presence == PresenceState::Unknown
                    && item.remote_presence == PresenceState::Present
                {
                    if let (Some(parents), Some(n)) = (
                        known(&r.parent_ids).filter(|ids| ids.len() == 1),
                        known(&r.name),
                    ) {
                        data.homonyms
                            .entry((
                                r.reference.account_id.clone(),
                                parents[0].0.clone(),
                                n.clone(),
                            ))
                            .or_default()
                            .push(index);
                    }
                }
            }
        }
        let data = Arc::new(data);
        *cached = Some(data.clone());
        return Ok(data);
    }
    Err(error(AnalyzerErrorKind::StaleItem))
}
fn local_destination(directory: &DirectoryRef, n: &ItemName) -> Option<Vec<u8>> {
    let ItemName::Local(n) = n else { return None };
    let mut path = match directory {
        DirectoryRef::LocalRoot => vec![],
        DirectoryRef::Local { relative_path, .. } => relative_path.as_bytes().to_vec(),
        _ => return None,
    };
    if !path.is_empty() {
        path.push(b'/')
    }
    path.extend(n.as_bytes());
    Some(path)
}
struct World<'a> {
    data: &'a Snapshot,
    identity: &'a SyncIdentity,
    provider: &'a ProviderId,
}
impl World<'_> {
    fn provider(&self, _item: &SyncItem) -> Option<ProviderId> {
        Some(self.provider.clone())
    }
    fn destination(
        &self,
        i: &SyncItem,
        upload: bool,
        plan: &mut AnalysisPlan,
        step: &mut PlannedStep,
    ) -> AnalyzerResult<Option<DirectoryRef>> {
        if upload {
            let Some(path) = parent_path(i) else {
                return Ok(None);
            };
            if path.is_empty() {
                return Ok(self.provider(i).map(|provider_id| DirectoryRef::Remote {
                    provider_id,
                    reference: RemoteFileRef {
                        account_id: self.identity.account_id.clone(),
                        file_id: RemoteFileId(self.identity.remote_root_id.clone()),
                    },
                }));
            }
            if let Some(parent) = self
                .data
                .locals
                .get(&path)
                .map(|index| &self.data.items[*index])
            {
                add_guard(plan, parent)?;
                if parent.kind == ItemKind::Directory
                    && parent.remote_presence == PresenceState::Present
                    && remote(parent).is_some_and(|r| {
                        r.kind == MetadataValue::Known(RemoteEntryKind::Directory)
                            && r.trashed == MetadataValue::Known(false)
                    })
                {
                    if let Some(ResourceRef::Remote {
                        provider_id,
                        reference,
                    }) = rref(parent)
                    {
                        return Ok(Some(DirectoryRef::Remote {
                            provider_id,
                            reference,
                        }));
                    }
                }
                push(&mut step.depends_on, parent.id.clone());
            }
        } else {
            let Some(ids) = remote(i)
                .and_then(|r| known(&r.parent_ids))
                .filter(|ids| ids.len() == 1)
            else {
                return Ok(None);
            };
            if ids[0].0 == self.identity.remote_root_id {
                return Ok(Some(DirectoryRef::LocalRoot));
            }
            if let Some(parent) = self
                .data
                .remotes
                .get(&(self.identity.account_id.clone(), ids[0].0.clone()))
                .map(|index| &self.data.items[*index])
            {
                add_guard(plan, parent)?;
                if parent.kind == ItemKind::Directory
                    && parent.local_presence == PresenceState::Present
                    && local(parent)
                        .is_some_and(|l| l.kind == MetadataValue::Known(LocalEntryKind::Directory))
                {
                    if let Some(f) = local(parent) {
                        return Ok(Some(DirectoryRef::Local {
                            relative_path: f.relative_path.clone(),
                            local_id: parent.local_file_id.clone(),
                        }));
                    }
                }
                push(&mut step.depends_on, parent.id.clone());
            }
        }
        push(&mut step.blocked_by, PlanBlock::Parent);
        Ok(None)
    }
    fn collisions(
        &self,
        item: &SyncItem,
        directory: &DirectoryRef,
        n: &ItemName,
    ) -> Vec<&SyncItem> {
        let Some(path) = local_destination(directory, n) else {
            return vec![];
        };
        let mut result = Vec::new();
        if let Some(other) = self.data.locals.get(&path).map(|i| &self.data.items[*i]) {
            if other.id != item.id && other.local_presence == PresenceState::Present {
                result.push(other)
            }
        }
        if let Some(r) = remote(item) {
            if let (Some(parents), ItemName::Local(n)) =
                (known(&r.parent_ids).filter(|ids| ids.len() == 1), n)
            {
                if let Some(n) = n.to_str() {
                    let parent_path = if parents[0].0 == self.identity.remote_root_id {
                        Some(vec![])
                    } else {
                        self.data
                            .remotes
                            .get(&(r.reference.account_id.clone(), parents[0].0.clone()))
                            .and_then(|i| local(&self.data.items[*i]))
                            .map(|l| l.relative_path.as_bytes().to_vec())
                    };
                    let expected = parent_path.map(|mut p| {
                        if !p.is_empty() {
                            p.push(b'/')
                        }
                        p.extend(n.as_bytes());
                        p
                    });
                    if expected.as_ref() == Some(&path) {
                        if let Some(indices) = self.data.homonyms.get(&(
                            r.reference.account_id.clone(),
                            parents[0].0.clone(),
                            n.to_string(),
                        )) {
                            for index in indices {
                                let other = &self.data.items[*index];
                                if other.id != item.id && !result.iter().any(|i| i.id == other.id) {
                                    result.push(other)
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }
}
fn preconditions(i: &SyncItem, upload: bool) -> AnalyzerResult<Preconditions> {
    Ok(if upload {
        Preconditions {
            expected_version: remote(i).and_then(|r| known(&r.version)).cloned(),
            expected_revision: None,
        }
    } else {
        Preconditions {
            expected_version: None,
            expected_revision: Some(format!(
                "snapshot-sha256:{}",
                ItemGuard::for_item(i)
                    .map_err(inventory_error)?
                    .fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            )),
        }
    })
}
fn block_for(r: AnalysisRequirement) -> PlanBlock {
    match r {
        AnalysisRequirement::VerifyIdentity => PlanBlock::Identity,
        AnalysisRequirement::ResolveDestination => PlanBlock::Destination,
        AnalysisRequirement::VerifyParent => PlanBlock::Parent,
        AnalysisRequirement::AuthorizeDeletion => PlanBlock::UserAuthorization,
        AnalysisRequirement::VerifyKind | AnalysisRequirement::VerifyCapabilities => {
            PlanBlock::Capabilities
        }
        _ => PlanBlock::Evidence,
    }
}
/// Produces an exact command or a concrete reason that prevents constructing it.
/// Dependent mutations require reanalysis after a prior verified result: their
/// old snapshot/version must never be reused blindly for the next step.
pub(super) fn build(
    inventory: &InventoryManager,
    identity: &SyncIdentity,
    provider: &ProviderId,
    cache: &Cache,
    request: &AnalyzeRequest,
    assessment: &mut NewAnalysisAssessment,
) -> AnalyzerResult<()> {
    let item = &request.item;
    let context = snapshot(inventory, provider, cache)?;
    let items = &context.items;
    let world = World {
        data: &context,
        identity,
        provider,
    };
    let mut plan = AnalysisPlan {
        restore_request: request.directive.clone(),
        ..Default::default()
    };
    plan.namespaces.push(context.guard.clone());
    add_guard(&mut plan, item)?;
    // The configured remote root is a scope binding, not a folder to download.
    if item.local.is_none()
        && item.remote_ref.as_ref().is_some_and(|r| {
            r.account_id == identity.account_id && r.file_id.0 == identity.remote_root_id
        })
    {
        assessment.intents.clear();
        assessment.requirements.clear();
        assessment.findings = vec![AnalysisFinding {
            origin: AnalysisOrigin::Planning,
            dimension: AnalysisDimension::Presence,
            status: AnalysisFindingStatus::Compatible,
            reason: AnalysisReason::Equivalent,
        }];
    }
    if let Some(choice) = &request.directive {
        if item
            .remote_ref
            .as_ref()
            .is_some_and(|r| r.file_id.0 == identity.remote_root_id)
        {
            return Err(error(AnalyzerErrorKind::WrongSyncScope));
        }
        if choice.decision_id.trim().is_empty() || !refs(item).contains(&choice.target) {
            return Err(error(AnalyzerErrorKind::InvalidRequest));
        }
        let upload = matches!(choice.target, ResourceRef::Remote { .. });
        assessment.intents = vec![AnalysisIntent {
            direction: if upload {
                OperationDirection::Upload
            } else {
                OperationDirection::Download
            },
            kind: OperationKind::Restore,
        }];
        assessment.findings = vec![AnalysisFinding {
            origin: AnalysisOrigin::Planning,
            dimension: AnalysisDimension::Presence,
            status: AnalysisFindingStatus::Compatible,
            reason: AnalysisReason::UserRequested,
        }];
        assessment.requirements.clear();
        if !upload || !remote(item).is_some_and(|r| r.trashed == MetadataValue::Known(true)) {
            assessment
                .requirements
                .push(AnalysisRequirement::VerifyCapabilities);
        }
    }
    for f in &assessment.findings {
        if f.status == AnalysisFindingStatus::Divergent {
            push(
                &mut plan.manual,
                ManualIssue {
                    reason: if f.reason == AnalysisReason::DeleteVsChange {
                        ManualReason::DeleteVsChange
                    } else {
                        ManualReason::ConcurrentChanges
                    },
                    resources: refs(item),
                },
            );
        }
    }
    if item.local_presence == PresenceState::Present
        && item.remote_presence == PresenceState::Present
        && item.baseline.is_none()
    {
        let association = inventory
            .get_association(GetAssociationRequest {
                item_id: item.id.clone(),
            })
            .map_err(inventory_error)?
            .association;
        plan.manual.push(ManualIssue {
            reason: if association.is_some() {
                ManualReason::MissingBaseline
            } else {
                ManualReason::AssociationRequired
            },
            resources: refs(item),
        });
    }
    let intents = assessment.intents.clone();
    if world.provider(item).is_none_or(|p| p.0 != "google") && !intents.is_empty() {
        push(
            &mut assessment.requirements,
            AnalysisRequirement::VerifyCapabilities,
        );
    }
    if remote(item).is_some_and(|r| {
        r.provider.0 != "google"
            || item.kind == ItemKind::File
                && known(&r.mime_type)
                    .is_some_and(|m| m.starts_with("application/vnd.google-apps."))
    }) {
        push(
            &mut assessment.requirements,
            AnalysisRequirement::VerifyCapabilities,
        );
    }
    for (position, intent) in intents.iter().enumerate() {
        let upload = intent.direction == OperationDirection::Upload;
        let method = match intent.kind {
            OperationKind::Create if item.kind == ItemKind::Directory => {
                PlannedMethod::CreateDirectory
            }
            OperationKind::Create if upload => PlannedMethod::Upload,
            OperationKind::Create => PlannedMethod::Download,
            OperationKind::UpdateContent => PlannedMethod::UpdateContent,
            OperationKind::Rename => PlannedMethod::Rename,
            OperationKind::Move => PlannedMethod::Move,
            OperationKind::Trash => PlannedMethod::Trash,
            OperationKind::Restore => PlannedMethod::Restore,
            OperationKind::Delete => return Err(error(AnalyzerErrorKind::InvalidRequest)),
        };
        let mut step = PlannedStep {
            method,
            command: None,
            blocked_by: vec![],
            depends_on: vec![],
            depends_on_steps: vec![],
        };
        let target = if upload { rref(item) } else { lref(item) };
        let conditions = preconditions(item, upload)?;
        if upload
            && !matches!(intent.kind, OperationKind::Create)
            && conditions.expected_version.is_none()
        {
            push(
                &mut assessment.requirements,
                AnalysisRequirement::VerifyPreconditions,
            );
        }
        let mut destination = None;
        let mut new_name = None;
        step.command = match intent.kind {
            OperationKind::Create => {
                destination = world.destination(item, upload, &mut plan, &mut step)?;
                new_name = name(item, upload);
                match (destination.clone(), new_name.clone()) {
                    (Some(parent), Some(n)) if item.kind == ItemKind::Directory => {
                        Some(OperationCommand::CreateDirectory { parent, name: n })
                    }
                    (Some(destination), Some(n)) if upload => {
                        lref(item).map(|source| OperationCommand::Upload {
                            source,
                            destination,
                            name: n,
                        })
                    }
                    (Some(destination), Some(n)) => {
                        rref(item).map(|source| OperationCommand::Download {
                            source,
                            destination,
                            name: n,
                        })
                    }
                    _ => None,
                }
            }
            OperationKind::UpdateContent => match (
                target,
                if upload {
                    local(item).map(|f| ContentSource::LocalFile {
                        relative_path: f.relative_path.clone(),
                        local_id: item.local_file_id.clone(),
                    })
                } else {
                    rref(item).map(|resource| ContentSource::RemoteFile { resource })
                },
            ) {
                (Some(target), Some(source)) => Some(OperationCommand::UpdateContent {
                    target,
                    source,
                    preconditions: conditions,
                }),
                _ => None,
            },
            OperationKind::Rename => {
                new_name = name(item, upload);
                destination = if upload {
                    None
                } else {
                    parent_path(item).and_then(|p| {
                        if p.is_empty() {
                            Some(DirectoryRef::LocalRoot)
                        } else {
                            LocalRelativePath::new(p).ok().map(|relative_path| {
                                DirectoryRef::Local {
                                    relative_path,
                                    local_id: None,
                                }
                            })
                        }
                    })
                };
                target
                    .zip(new_name.clone())
                    .map(|(target, new_name)| OperationCommand::Rename {
                        target,
                        new_name,
                        preconditions: conditions,
                    })
            }
            OperationKind::Move => {
                destination = world.destination(item, upload, &mut plan, &mut step)?;
                // Preserve target name; moving is not an implicit rename.
                new_name = if upload {
                    remote(item)
                        .and_then(|r| known(&r.name))
                        .cloned()
                        .map(ItemName::Remote)
                } else {
                    local(item)
                        .and_then(|f| f.relative_path.as_bytes().rsplit(|b| *b == b'/').next())
                        .map(|n| ItemName::Local(OsString::from_vec(n.to_vec())))
                };
                target
                    .zip(destination.clone())
                    .map(|(target, destination)| OperationCommand::Move {
                        target,
                        destination,
                        preconditions: conditions,
                    })
            }
            OperationKind::Trash => {
                if item.kind == ItemKind::Directory {
                    push(
                        &mut assessment.requirements,
                        AnalysisRequirement::VerifySurvivingState,
                    );
                    let directory = match target.as_ref() {
                        Some(ResourceRef::Remote {
                            provider_id,
                            reference,
                        }) => Some(DirectoryRef::Remote {
                            provider_id: provider_id.clone(),
                            reference: reference.clone(),
                        }),
                        Some(ResourceRef::Local {
                            relative_path,
                            local_id,
                        }) => Some(DirectoryRef::Local {
                            relative_path: relative_path.clone(),
                            local_id: local_id.clone(),
                        }),
                        _ => None,
                    };
                    if let Some(directory) = directory {
                        push(
                            &mut plan.namespaces,
                            inventory
                                .get_namespace(GetNamespaceRequest { directory })
                                .map_err(inventory_error)?
                                .guard,
                        );
                    }
                    for child in items {
                        let local_child = !upload
                            && local(item)
                                .zip(local(child))
                                .is_some_and(|(parent, child)| {
                                    let mut prefix = parent.relative_path.as_bytes().to_vec();
                                    prefix.push(b'/');
                                    child.relative_path.as_bytes().starts_with(&prefix)
                                });
                        let remote_child = upload
                            && item.remote_ref.as_ref().is_some_and(|parent| {
                                remote(child)
                                    .and_then(|c| known(&c.parent_ids))
                                    .is_some_and(|ids| ids.contains(&parent.file_id))
                            });
                        if child.id != item.id && (local_child || remote_child) {
                            add_guard(&mut plan, child)?;
                            push(&mut step.depends_on, child.id.clone());
                            push(&mut step.blocked_by, PlanBlock::Parent);
                        }
                    }
                }
                push(
                    &mut assessment.requirements,
                    AnalysisRequirement::AuthorizeDeletion,
                );
                plan.manual.push(ManualIssue {
                    reason: ManualReason::DeletionAuthorization,
                    resources: refs(item),
                });
                target.map(|target| OperationCommand::Trash {
                    target,
                    preconditions: conditions,
                })
            }
            OperationKind::Restore => {
                if upload && remote(item).is_some_and(|r| r.trashed == MetadataValue::Known(true)) {
                    target.map(|target| OperationCommand::Restore {
                        target,
                        destination: None,
                        preconditions: conditions,
                    })
                } else {
                    None
                }
            }
            OperationKind::Delete => None,
        };
        if let Some(directory) = destination.as_ref() {
            push(
                &mut plan.namespaces,
                inventory
                    .get_namespace(GetNamespaceRequest {
                        directory: directory.clone(),
                    })
                    .map_err(inventory_error)?
                    .guard,
            );
            if let Some(n) = new_name.as_ref().filter(|_| position == 0) {
                if let Some(other) = local_destination(directory, n)
                    .and_then(|p| context.locals.get(&p))
                    .map(|i| &context.items[*i])
                {
                    if other.id != item.id && other.local_presence == PresenceState::Unknown {
                        add_guard(&mut plan, other)?;
                        push(&mut step.blocked_by, PlanBlock::Evidence);
                        push(
                            &mut assessment.requirements,
                            AnalysisRequirement::ObserveLocal,
                        );
                    }
                }
                for other in world.collisions(item, directory, n) {
                    add_guard(&mut plan, other)?;
                    push(&mut step.blocked_by, PlanBlock::Collision);
                    let mut resources = refs(item);
                    for r in refs(other) {
                        push(&mut resources, r)
                    }
                    if let Some(issue) = plan
                        .manual
                        .iter_mut()
                        .find(|issue| issue.reason == ManualReason::DestinationOccupied)
                    {
                        for resource in resources {
                            push(&mut issue.resources, resource);
                        }
                    } else {
                        plan.manual.push(ManualIssue {
                            reason: ManualReason::DestinationOccupied,
                            resources,
                        });
                    }
                    let f = AnalysisFinding {
                        origin: AnalysisOrigin::Planning,
                        dimension: AnalysisDimension::Location,
                        status: AnalysisFindingStatus::Divergent,
                        reason: AnalysisReason::DestinationOccupied,
                    };
                    push(&mut assessment.findings, f);
                }
            }
        }
        if step.command.is_none() {
            push(
                &mut step.blocked_by,
                if !step.depends_on.is_empty() {
                    PlanBlock::Parent
                } else {
                    PlanBlock::Destination
                },
            );
        }
        if position > 0 {
            step.depends_on_steps.push(position as u32 - 1);
            push(&mut step.blocked_by, PlanBlock::PriorStep);
        }
        plan.steps.push(step);
    }
    if !plan.steps.is_empty()
        && plan
            .steps
            .iter()
            .all(|s| s.command.is_some() && !s.blocked_by.contains(&PlanBlock::Collision))
    {
        assessment
            .requirements
            .retain(|r| *r != AnalysisRequirement::ResolveDestination);
    }
    let conflict = assessment
        .findings
        .iter()
        .any(|f| f.status == AnalysisFindingStatus::Divergent);
    for step in &mut plan.steps {
        for r in &assessment.requirements {
            push(&mut step.blocked_by, block_for(*r))
        }
        if conflict {
            push(&mut step.blocked_by, PlanBlock::Conflict)
        }
    }
    assessment.disposition = if conflict {
        AnalysisDisposition::PendingUserDecision
    } else if !assessment.requirements.is_empty()
        || plan.steps.iter().any(|s| !s.blocked_by.is_empty())
    {
        AnalysisDisposition::PendingEvidence
    } else if !assessment.intents.is_empty() {
        AnalysisDisposition::PendingOperations
    } else {
        AnalysisDisposition::NoChanges
    };
    assessment.plan = Some(plan);
    Ok(())
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn indexed_context_is_reused_until_the_observation_epoch_changes() {
        let root =
            std::env::temp_dir().join(format!("analyzer-context-cache-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let inventory = InventoryManager::open(OpenInventoryRequest {
            local_root: root.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("cache".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        })
        .unwrap()
        .inventory;
        let cache = Cache::default();
        let provider = ProviderId("google".into());
        let first = snapshot(&inventory, &provider, &cache).unwrap();
        let second = snapshot(&inventory, &provider, &cache).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let sql = rusqlite::Connection::open(root.join(".drivesync/inventory.sqlite3")).unwrap();
        sql.execute("UPDATE planning_clock SET epoch=epoch+1 WHERE id=1", [])
            .unwrap();
        let third = snapshot(&inventory, &provider, &cache).unwrap();
        assert!(!Arc::ptr_eq(&first, &third));
        drop(inventory);
        drop(sql);
        std::fs::remove_dir_all(root).unwrap();
    }
}
