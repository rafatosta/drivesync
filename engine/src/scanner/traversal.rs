//! Iterative observation and conservative absence confirmation through Inventory's public API.
use super::*;
use crate::models::File as Observation;
use std::{collections::BTreeMap, io, io::Read, os::unix::ffi::OsStrExt};
fn missing(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ENOTDIR)
}
struct Run<'a> {
    inner: &'a Inner,
    cancel: &'a AtomicBool,
    id: String,
    sequence: u64,
    report: ScanReport,
    coverage: BTreeMap<Vec<u8>, filesystem::Stamp>,
}
impl Run<'_> {
    fn check(&self) -> ScannerResult<()> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(failure(ScannerErrorKind::Cancelled));
        }
        Ok(())
    }
    fn issue(&mut self, path: &[u8], kind: ScanIssueKind, impact: ScanIssueImpact) {
        if impact != ScanIssueImpact::ItemIncomplete {
            self.report.status = ScanStatus::Partial;
        }
        let location = LocalRelativePath::new(path.to_vec())
            .map(ScanIssueLocation::RelativePath)
            .unwrap_or(ScanIssueLocation::ScanRoot);
        self.report.issues.push(ScanIssue {
            location,
            kind,
            impact,
            message: "local observation requires review".into(),
        });
        self.inner.status_update(|s| s.issues_recorded += 1);
    }
    fn io_issue(&mut self, path: &[u8], error: &io::Error, impact: ScanIssueImpact) {
        let kind = match error.kind() {
            io::ErrorKind::PermissionDenied => ScanIssueKind::AccessDenied,
            io::ErrorKind::NotFound => ScanIssueKind::EntryDisappeared,
            _ => match error.raw_os_error() {
                Some(libc::EXDEV) => ScanIssueKind::MountBoundary,
                Some(libc::ESTALE | libc::ENOTDIR | libc::ELOOP) => ScanIssueKind::EntryChanged,
                Some(libc::EMFILE | libc::ENFILE | libc::ENOMEM) => {
                    ScanIssueKind::ResourceExhausted
                }
                _ => ScanIssueKind::ReadFailed,
            },
        };
        self.issue(path, kind, impact);
    }
    fn persist(
        &mut self,
        causal_order: u64,
        path: LocalRelativePath,
        data: Option<LocalFile>,
        presence: PresenceState,
        complete: bool,
        existing: Option<SyncItem>,
    ) -> ScannerResult<()> {
        self.check()?;
        self.inner.validate()?;
        if existing.is_none() && data.is_none() {
            self.issue(
                path.as_bytes(),
                ScanIssueKind::EntryChanged,
                ScanIssueImpact::ObservationNotPersisted,
            );
            return Ok(());
        }
        #[cfg(test)]
        if let Some(hook) = &self.inner.hook {
            hook(Stage::BeforeRecord, path.as_bytes());
        }
        self.check()?;
        self.inner
            .filesystem
            .validate()
            .map_err(|_| failure(ScannerErrorKind::RootInaccessible))?;
        self.sequence += 1;
        let observed_at = now();
        let observed = Observation {
            id: FileId(format!("scan:{}:{}", self.id, self.sequence)),
            causal_order: Some(causal_order),
            sync_id: self.inner.root.sync_id.clone(),
            observed_at,
            scope: FileScope::Local(path.clone()),
            coverage: if complete {
                ObservationCoverage::Complete
            } else {
                ObservationCoverage::Partial
            },
            event: FileEvent {
                action: FileEventAction::RecheckRequired,
                local: None,
                local_presence: None,
                remote: None,
                uncertainty: Some(ObservationUncertainty::UnclassifiedSignal),
                previous: None,
                current: data.as_ref().map(|_| FileLocation::Local(path.clone())),
            },
            data: data.map(FileData::Local),
        };
        let request = RecordLocalObservation {
            selector: existing
                .as_ref()
                .map(|v| ObservationTarget::Existing(v.id.clone()))
                .unwrap_or(ObservationTarget::NewItem),
            expected_revision: existing.as_ref().map(|v| v.revision),
            observation: crate::inventory::LocalObservation {
                observed,
                presence,
                proof: ObservationProof {
                    source: ObservationSource::LocalScan,
                    observed_at,
                    scan_or_generation: Some(self.id.clone()),
                    complete_scope: complete,
                },
            },
        };
        let result = self.inner.inventory.record_local(request);
        let (item, incomparable) = match result {
            Ok(value) => (
                value.item,
                value.disposition == ObservationDisposition::Incomparable,
            ),
            Err(error) => return self.record_failure(path.as_bytes(), error),
        };
        if incomparable {
            self.issue(
                path.as_bytes(),
                ScanIssueKind::InventoryRecordFailed,
                ScanIssueImpact::ObservationNotPersisted,
            );
            self.inner
                .order_source
                .request_local_revalidation(path.clone())
                .map_err(|_| failure(ScannerErrorKind::InventoryUnavailable))?;
        }
        self.report.items_recorded += 1;
        self.inner.status_update(|s| s.items_recorded += 1);
        // Pending is durable and does not claim a transfer or change baseline/remote facts.
        if !incomparable && item.status != SyncStatus::PendingReconciliation {
            if let Err(error) = self
                .inner
                .inventory
                .update_item_status(UpdateItemStatusRequest {
                    item_id: item.id,
                    expected_revision: item.revision,
                    change: ItemStatusChange {
                        status: SyncStatus::PendingReconciliation,
                        conflict_reason: FieldUpdate::Keep,
                    },
                })
            {
                self.record_failure(path.as_bytes(), error)?;
            }
        }
        Ok(())
    }
    fn record_failure(&mut self, path: &[u8], error: InventoryFailure) -> ScannerResult<()> {
        if matches!(
            error.kind,
            InventoryError::VersionMismatch
                | InventoryError::DuplicateAssociation
                | InventoryError::InvalidArgument
                | InventoryError::MissingObservation
                | InventoryError::NotFound
        ) {
            self.issue(
                path,
                ScanIssueKind::InventoryRecordFailed,
                ScanIssueImpact::ObservationNotPersisted,
            );
            Ok(())
        } else {
            Err(inventory_error(error))
        }
    }
    fn walk(&mut self) -> ScannerResult<()> {
        let mut directories = vec![Vec::new()];
        while let Some(path) = directories.pop() {
            self.check()?;
            self.inner
                .filesystem
                .validate()
                .map_err(|_| failure(ScannerErrorKind::RootInaccessible))?;
            self.inner
                .status_update(|s| s.current_directory = LocalRelativePath::new(path.clone()).ok());
            let directory = match self.inner.filesystem.entry(&path) {
                Ok(v) => v,
                Err(e) => {
                    if path.is_empty() {
                        return Err(failure(ScannerErrorKind::RootInaccessible));
                    }
                    self.io_issue(&path, &e, ScanIssueImpact::SubtreeUnobserved);
                    continue;
                }
            };
            let metadata = match directory.metadata() {
                Ok(metadata) if metadata.is_dir() => metadata,
                Ok(_) => {
                    self.issue(
                        &path,
                        ScanIssueKind::EntryChanged,
                        ScanIssueImpact::SubtreeUnobserved,
                    );
                    continue;
                }
                Err(error) if path.is_empty() => {
                    return Err(failure(if error.kind() == io::ErrorKind::NotFound {
                        ScannerErrorKind::RootNotFound
                    } else {
                        ScannerErrorKind::RootInaccessible
                    }))
                }
                Err(error) => {
                    self.io_issue(&path, &error, ScanIssueImpact::SubtreeUnobserved);
                    continue;
                }
            };
            self.report.directories_observed += 1;
            self.inner.status_update(|s| s.directories_observed += 1);
            let before = filesystem::Stamp::from(&metadata);
            let entries = match directory.children() {
                Ok(v) => v,
                Err(e) => {
                    if path.is_empty() {
                        return Err(failure(ScannerErrorKind::RootInaccessible));
                    }
                    self.io_issue(&path, &e, ScanIssueImpact::SubtreeUnobserved);
                    continue;
                }
            };
            let mut complete = true;
            for entry in entries {
                self.check()?;
                let entry = match entry {
                    Ok(v) => v,
                    Err(e) => {
                        complete = false;
                        self.io_issue(&path, &e, ScanIssueImpact::SubtreeUnobserved);
                        continue;
                    }
                };
                let name = entry.file_name();
                if name == ".drivesync" {
                    continue;
                }
                let child = if path.is_empty() {
                    name.as_bytes().to_vec()
                } else {
                    [path.as_slice(), b"/", name.as_bytes()].concat()
                };
                self.observe(&child, &mut directories)?;
            }
            #[cfg(test)]
            if let Some(hook) = &self.inner.hook {
                hook(Stage::DirectoryRead, &path);
            }
            self.inner
                .filesystem
                .validate()
                .map_err(|_| failure(ScannerErrorKind::RootInaccessible))?;
            if !complete {
                continue;
            }
            if directory.validate().is_ok() && directory.stamp().is_ok_and(|after| before == after)
            {
                self.coverage.insert(path, before);
            } else {
                self.issue(
                    &path,
                    ScanIssueKind::EntryChanged,
                    ScanIssueImpact::SubtreeUnobserved,
                );
            }
        }
        Ok(())
    }
    fn observe(&mut self, path: &[u8], directories: &mut Vec<Vec<u8>>) -> ScannerResult<()> {
        let causal_order = self
            .inner
            .order_source
            .reserve_causal_order()
            .map_err(|_| failure(ScannerErrorKind::InventoryUnavailable))?;
        let relative = LocalRelativePath::new(path.to_vec())
            .map_err(|_| failure(ScannerErrorKind::Internal))?;
        let existing = self
            .inner
            .inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::LocalRelativePath(relative.clone()),
            })
            .map_err(inventory_error)?
            .item;
        let entry = match self.inner.filesystem.entry(path) {
            Ok(v) => v,
            Err(e) => {
                self.io_issue(path, &e, ScanIssueImpact::SubtreeUnobserved);
                return Ok(());
            }
        };
        let metadata = match entry.metadata() {
            Ok(v) => v,
            Err(e) => {
                self.io_issue(path, &e, ScanIssueImpact::SubtreeUnobserved);
                return Ok(());
            }
        };
        if metadata.is_dir() {
            directories.push(path.to_vec());
        } else if metadata.is_file() {
            self.report.files_observed += 1;
            self.inner.status_update(|s| s.files_observed += 1);
        }
        let file = match filesystem::collect(&entry, relative.clone(), self.inner.native) {
            Ok(Some(file)) => file,
            Ok(None) => {
                self.issue(
                    path,
                    ScanIssueKind::EntryChanged,
                    ScanIssueImpact::ItemIncomplete,
                );
                return self.persist(
                    causal_order,
                    relative,
                    None,
                    PresenceState::Unknown,
                    false,
                    existing,
                );
            }
            Err(e) => {
                self.io_issue(path, &e, ScanIssueImpact::ItemIncomplete);
                return self.persist(
                    causal_order,
                    relative,
                    None,
                    PresenceState::Unknown,
                    false,
                    existing,
                );
            }
        };
        let existing = if let Some(id) = &file.local_id {
            let by_id = self
                .inner
                .inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::LocalId(id.clone()),
                })
                .map_err(inventory_error)?
                .item;
            if let Some(previous) = &existing {
                if previous.local_file_id.as_ref().is_some_and(|old| old != id) {
                    self.issue(
                        path,
                        ScanIssueKind::EntryChanged,
                        ScanIssueImpact::ItemIncomplete,
                    );
                    return self.persist(
                        causal_order,
                        relative,
                        None,
                        PresenceState::Unknown,
                        false,
                        existing,
                    );
                }
            }
            by_id.or(existing)
        } else {
            existing
        };
        #[cfg(test)]
        if let Some(hook) = &self.inner.hook {
            hook(Stage::Observed, path);
        }
        self.check()?;
        fn unavailable<T>(value: &MetadataValue<T>) -> bool {
            matches!(
                value,
                MetadataValue::PermissionDenied
                    | MetadataValue::Unavailable
                    | MetadataValue::Unsupported
            )
        }
        if !file.basic_profile_complete()
            || unavailable(&file.times.created_at)
            || unavailable(&file.flags.statx_attributes)
        {
            self.issue(
                path,
                ScanIssueKind::MetadataUnavailable,
                ScanIssueImpact::ItemIncomplete,
            );
        }
        if !metadata.is_dir() && !metadata.is_file() && !metadata.file_type().is_symlink() {
            self.issue(
                path,
                ScanIssueKind::UnsupportedEntryType,
                ScanIssueImpact::ItemIncomplete,
            );
        }
        if entry.validate().is_err()
            || !entry
                .stamp()
                .is_ok_and(|stamp| stamp == filesystem::Stamp::from(&metadata))
        {
            self.issue(
                path,
                ScanIssueKind::EntryChanged,
                ScanIssueImpact::ItemIncomplete,
            );
            return self.persist(
                causal_order,
                relative,
                None,
                PresenceState::Unknown,
                false,
                existing,
            );
        }
        self.persist(
            causal_order,
            relative,
            Some(file),
            PresenceState::Present,
            false,
            existing,
        )
    }
    /// Rechecks each known location instead of retaining every SyncItem/metadata snapshot in memory.
    fn absences(&mut self) -> ScannerResult<()> {
        let mut after = None;
        loop {
            self.check()?;
            self.inner.validate()?;
            let page = self
                .inner
                .inventory
                .list_items(ListItemsRequest {
                    filter: ItemFilter::default(),
                    pagination: Pagination { limit: 100, after },
                })
                .map_err(inventory_error)?
                .page;
            for item in page.entries {
                self.check()?;
                let Some(local) = item.local.clone() else {
                    continue;
                };
                let path = local.relative_path.as_bytes();
                if filesystem::reserved(path) {
                    continue;
                }
                let causal_order = self
                    .inner
                    .order_source
                    .reserve_causal_order()
                    .map_err(|_| failure(ScannerErrorKind::InventoryUnavailable))?;
                match self.inner.filesystem.entry(path) {
                    Ok(_) => continue,
                    Err(e) if missing(&e) => {}
                    Err(e) => {
                        self.io_issue(path, &e, ScanIssueImpact::SubtreeUnobserved);
                        continue;
                    }
                }
                // Find a fully enumerated, still-bound nearest ancestor. Never use incomplete subtrees as evidence.
                let mut parent = path.rsplitn(2, |b| *b == b'/').nth(1).unwrap_or(&[]);
                let mut proven = false;
                loop {
                    match self.inner.filesystem.entry(parent) {
                        Ok(entry) => {
                            // A positively observed non-directory is a leaf of this namespace.
                            // Its old descendants cannot be reached; do not follow a symlink target.
                            if !parent.is_empty() && entry.metadata().is_ok_and(|m| !m.is_dir()) {
                                parent = parent.rsplitn(2, |b| *b == b'/').nth(1).unwrap_or(&[]);
                                continue;
                            }
                            proven = self.coverage.get(parent).is_some_and(|stamp| {
                                entry.validate().is_ok()
                                    && entry.stamp().is_ok_and(|current| &current == stamp)
                            });
                            break;
                        }
                        Err(e) if missing(&e) && !parent.is_empty() => {
                            parent = parent.rsplitn(2, |b| *b == b'/').nth(1).unwrap_or(&[]);
                        }
                        _ => break,
                    }
                }
                if proven {
                    // Revalidate immediately before writing; a concurrent reappearance is never treated as absent.
                    if self
                        .inner
                        .filesystem
                        .entry(path)
                        .is_err_and(|e| missing(&e))
                    {
                        self.persist(
                            causal_order,
                            local.relative_path,
                            None,
                            PresenceState::AbsentConfirmed,
                            true,
                            Some(item.clone()),
                        )?;
                    }
                } else if item.local_presence != PresenceState::AbsentConfirmed {
                    self.issue(
                        path,
                        ScanIssueKind::EntryChanged,
                        ScanIssueImpact::SubtreeUnobserved,
                    );
                }
            }
            after = page.next_cursor;
            if after.is_none() {
                break;
            }
        }
        Ok(())
    }
}
pub(super) fn run(inner: &Inner, cancel: &AtomicBool) -> ScannerResult<ScanReport> {
    inner.validate()?;
    let mut random = [0u8; 24];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|_| failure(ScannerErrorKind::Internal))?;
    let id = random.iter().map(|b| format!("{b:02x}")).collect();
    let mut run = Run {
        inner,
        cancel,
        id,
        sequence: 0,
        report: ScanReport {
            sync_id: inner.root.sync_id.clone(),
            status: ScanStatus::Completed,
            files_observed: 0,
            directories_observed: 0,
            items_recorded: 0,
            issues: Vec::new(),
        },
        coverage: BTreeMap::new(),
    };
    run.walk()?;
    run.check()?;
    run.absences()?;
    inner.validate()?;
    run.check()?;
    Ok(run.report)
}
