//! Initial materialization of a configured Drive tree.
use crate::{
    configuration::valid_id, remote, EngineError, FolderConfiguration, ItemId, ItemSnapshot,
    ItemState, LocalItemKind, LocalScope, LocalStateError, LocalStateStore,
};
use rustix::fs::{self, Mode, OFlags, RenameFlags, ResolveFlags, CWD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    collections::HashSet,
    fs::File,
    io::{self, Read, Seek, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
    time::{Duration, Instant},
};

const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const FOLDER: &str = "application/vnd.google-apps.folder";
const FIELDS: &str =
    "id,name,mimeType,version,size,trashed,explicitlyTrashed,parents,sha256Checksum,modifiedTime";
const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::CLOEXEC);

/// Progress on a completed initial download. Earlier commits survive a failure.
#[derive(Debug, Default)]
pub struct InitialDownload {
    pub files: usize,
    pub directories: usize,
    pub already_registered: usize,
}

/// Progress for one real file download. Completion means the identity was saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadProgress {
    Started { total: u64 },
    Receiving { received: u64, total: u64 },
    Completed,
    Failed,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    name: String,
    mime_type: String,
    version: String,
    size: Option<String>,
    sha256_checksum: Option<String>,
    modified_time: Option<String>,
    #[serde(default)]
    trashed: bool,
    #[serde(default)]
    explicitly_trashed: bool,
    #[serde(default)]
    parents: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    files: Vec<Entry>,
    next_page_token: Option<String>,
    #[serde(default)]
    incomplete_search: bool,
}

fn metadata(token: &str, id: &str) -> Result<Entry, EngineError> {
    if !valid_id(id) {
        return Err(EngineError::DownloadIncomplete);
    }
    let body = remote::request(
        token,
        &format!("{FILES}/{id}"),
        &[("fields", FIELDS), ("supportsAllDrives", "true")],
    )?;
    serde_json::from_slice(&body).map_err(|_| EngineError::DownloadIncomplete)
}

fn children(token: &str, parent: &str) -> Result<Vec<Entry>, EngineError> {
    let query = format!("'{parent}' in parents and trashed = false");
    let fields = format!("nextPageToken,incompleteSearch,files({FIELDS})");
    let mut entries = Vec::new();
    let mut pages = HashSet::new();
    let mut names = HashSet::new();
    let mut next = None;
    loop {
        let mut params = vec![
            ("q", query.as_str()),
            ("fields", fields.as_str()),
            ("pageSize", "200"),
            ("supportsAllDrives", "true"),
            ("includeItemsFromAllDrives", "true"),
        ];
        if let Some(page) = next.as_deref() {
            params.push(("pageToken", page));
        }
        let page: Page = serde_json::from_slice(&remote::request(token, FILES, &params)?)
            .map_err(|_| EngineError::DownloadIncomplete)?;
        if page.incomplete_search || entries.len() + page.files.len() >= 10_000 {
            return Err(EngineError::DownloadIncomplete);
        }
        for entry in page.files {
            if !entry.valid_child(parent) || !names.insert(entry.name.clone()) {
                return Err(EngineError::DownloadConflict);
            }
            entries.push(entry);
        }
        match page.next_page_token {
            None => return Ok(entries),
            Some(page)
                if !page.is_empty()
                    && page.len() <= 4096
                    && pages.len() < 100
                    && pages.insert(page.clone()) =>
            {
                next = Some(page)
            }
            Some(_) => return Err(EngineError::DownloadIncomplete),
        }
    }
}

impl Entry {
    fn valid_child(&self, parent: &str) -> bool {
        valid_id(&self.id)
            && !self.trashed
            && self.parents.iter().any(|id| id == parent)
            && valid_name(&self.name)
    }

    fn kind(&self) -> Result<LocalItemKind, EngineError> {
        if self.mime_type == FOLDER {
            Ok(LocalItemKind::Directory)
        } else if self.mime_type.starts_with("application/vnd.google-apps.") || self.size.is_none()
        {
            Err(EngineError::DownloadUnsupported)
        } else {
            Ok(LocalItemKind::File)
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.contains(['/', '\0'])
        && name != ".drivesync"
        && !name.starts_with(".drivesync-download-")
}

pub(crate) fn run(
    token: &str,
    folder: &FolderConfiguration,
    scope: &LocalScope,
    notify: &dyn Fn(&Path, DownloadProgress),
) -> Result<InitialDownload, EngineError> {
    run_selected(token, folder, scope, notify, None)
}

pub(crate) fn run_selected(
    token: &str,
    folder: &FolderConfiguration,
    scope: &LocalScope,
    notify: &dyn Fn(&Path, DownloadProgress),
    targets: Option<&HashSet<String>>,
) -> Result<InitialDownload, EngineError> {
    let directory = fs::openat2(
        CWD,
        &folder.local,
        DIR,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|_| EngineError::DownloadLocal)?;
    fs::flock(&directory, fs::FlockOperation::NonBlockingLockExclusive).map_err(|error| {
        if error == rustix::io::Errno::WOULDBLOCK {
            EngineError::DownloadBusy
        } else {
            EngineError::DownloadLocal
        }
    })?;
    let store = LocalStateStore::portable(&folder.local, scope, &folder.remote.id)
        .map_err(EngineError::DownloadState)?;
    let completed = store
        .initial_download_complete(scope)
        .map_err(EngineError::DownloadState)?;
    let root = metadata(token, &folder.remote.id)?;
    if root.trashed || root.mime_type != FOLDER || !valid_id(&root.id) {
        return Err(EngineError::DownloadIncomplete);
    }
    let paths = if let Some(targets) = targets {
        let checkpoint = store.load(scope).map_err(EngineError::DownloadState)?;
        let known: HashSet<_> = checkpoint
            .iter()
            .flat_map(|state| &state.items)
            .filter_map(|item| item.remote_id.as_deref())
            .collect();
        let mut paths = HashSet::new();
        for target in targets
            .iter()
            .filter(|target| !known.contains(target.as_str()))
        {
            let mut id = target.clone();
            let mut chain = HashSet::new();
            while id != root.id && !paths.contains(&id) {
                if chain.len() >= 64 || !chain.insert(id.clone()) {
                    return Err(EngineError::DownloadIncomplete);
                }
                let entry = metadata(token, &id)?;
                if entry.id != id || entry.trashed {
                    return Err(EngineError::DownloadIncomplete);
                }
                let [parent] = entry.parents.as_slice() else {
                    return Err(EngineError::DownloadIncomplete);
                };
                id = parent.clone();
            }
            paths.extend(chain);
        }
        if paths.is_empty() {
            return Ok(InitialDownload::default());
        }
        Some(paths)
    } else {
        None
    };
    let mut job = Job {
        token,
        notify,
        selected: paths.as_ref(),
        revisiting: completed && paths.is_none(),
        scope,
        store,
        root: directory.into(),
        seen: HashSet::from([root.id.clone()]),
        report: InitialDownload::default(),
    };
    let id = job.register(&root, None, LocalItemKind::Directory, None, || Ok(None))?;
    job.walk(&root.id, &id, Path::new("."), 0)?;
    if let Some(paths) = &paths {
        if !paths.is_subset(&job.seen) {
            return Err(EngineError::DownloadIncomplete);
        }
        return Ok(job.report);
    }
    let checkpoint = job.store.load(scope).map_err(EngineError::DownloadState)?;
    if !completed
        && checkpoint
            .iter()
            .flat_map(|state| &state.items)
            .any(|item| {
                item.remote_id
                    .as_ref()
                    .is_some_and(|id| !job.seen.contains(id))
            })
    {
        return Err(EngineError::DownloadConflict);
    }
    if !completed {
        job.store
            .finish_initial_download(scope, &id)
            .map_err(EngineError::DownloadState)?;
    }
    Ok(job.report)
}

struct Job<'a> {
    token: &'a str,
    notify: &'a dyn Fn(&Path, DownloadProgress),
    selected: Option<&'a HashSet<String>>,
    revisiting: bool,
    scope: &'a LocalScope,
    store: LocalStateStore,
    root: File,
    seen: HashSet<String>,
    report: InitialDownload,
}

impl Job<'_> {
    fn walk(
        &mut self,
        remote_id: &str,
        id: &ItemId,
        path: &Path,
        depth: usize,
    ) -> Result<(), EngineError> {
        if depth >= 64 {
            return Err(EngineError::DownloadIncomplete);
        }
        for entry in children(self.token, remote_id)? {
            if self.selected.is_some_and(|ids| !ids.contains(&entry.id)) {
                continue;
            }
            if self.seen.len() >= 10_000 || !self.seen.insert(entry.id.clone()) {
                return Err(EngineError::DownloadIncomplete);
            }
            let kind = entry.kind()?;
            let child = path.join(&entry.name);
            let directory = self.open_directory(path)?;
            let conflict = || EngineError::DownloadConflictAt(child.clone());
            let pending = PendingFile::new(&entry, &directory);
            let notify = self.notify;
            let transfer = Transfer {
                notify,
                path: child.strip_prefix(".").unwrap_or(&child),
                active: Cell::new(false),
            };
            let registered_before = self.report.already_registered;
            let installed_before = self.report.files + self.report.directories;
            let item = self
                .register(&entry, Some(id), kind, Some(&pending), || {
                    if kind == LocalItemKind::Directory {
                        fs::mkdirat(&directory, entry.name.as_str(), Mode::RWXU)
                            .map_err(local_error)?;
                        directory
                            .sync_all()
                            .map_err(|_| EngineError::DownloadLocal)?;
                    } else {
                        return pending
                            .install(self.token, |progress| transfer.emit(progress))
                            .map(Some);
                    }
                    Ok(None)
                })
                .map_err(|error| match error {
                    EngineError::DownloadConflict => conflict(),
                    other => other,
                })?;
            if self.revisiting
                && kind == LocalItemKind::File
                && registered_before != self.report.already_registered
            {
                continue;
            }
            let local = fs::statat(
                &directory,
                entry.name.as_str(),
                fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| conflict())?;
            let expected = if kind == LocalItemKind::Directory {
                fs::FileType::Directory
            } else {
                fs::FileType::RegularFile
            };
            if fs::FileType::from_raw_mode(local.st_mode) != expected
                || (kind == LocalItemKind::File
                    && entry
                        .size
                        .as_deref()
                        .and_then(|size| size.parse::<i64>().ok())
                        != Some(local.st_size))
            {
                return Err(conflict());
            }
            transfer.complete();
            if installed_before != self.report.files + self.report.directories {
                crate::ActivityHistory::append_remote(
                    self.scope,
                    crate::LocalNotice {
                        path: child.strip_prefix(".").unwrap_or(&child).into(),
                        object_type: kind,
                        activity: crate::LocalActivity::Created,
                    },
                    crate::RemoteActivity::Added,
                )
                .map_err(EngineError::ActivityHistory)?;
            }
            if kind == LocalItemKind::Directory {
                self.walk(&entry.id, &item, &child, depth + 1)?;
            } else {
                pending.clear()?;
            }
        }
        Ok(())
    }

    fn open_directory(&self, path: &Path) -> Result<File, EngineError> {
        fs::openat2(
            &self.root,
            path,
            DIR,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )
        .map(File::from)
        .map_err(|_| EngineError::DownloadLocal)
    }

    fn register(
        &mut self,
        entry: &Entry,
        parent: Option<&ItemId>,
        kind: LocalItemKind,
        pending: Option<&PendingFile<'_>>,
        install: impl FnOnce() -> Result<Option<String>, EngineError>,
    ) -> Result<ItemId, EngineError> {
        let mut snapshot = ItemSnapshot {
            parent: parent.cloned(),
            name: parent.map_or_else(Vec::new, |_| entry.name.as_bytes().to_vec()),
            kind,
            content_version: None,
            remote_version: (kind == LocalItemKind::File).then(|| entry.version.clone()),
        };
        let mut checkpoint = self
            .store
            .load(self.scope)
            .map_err(EngineError::DownloadState)?;
        if let Some(existing) = checkpoint.iter().flat_map(|c| &c.items).find(|item| {
            item.remote_id.as_deref() == Some(&entry.id)
                || matches!(&item.observed, ItemState::Present(s) if s.parent == snapshot.parent && s.name == snapshot.name)
        }) {
            if let ItemState::Present(current) = &existing.observed {
                snapshot.content_version = current.content_version.clone();
                if self.revisiting && kind == LocalItemKind::File {
                    snapshot.remote_version = current.remote_version.clone();
                }
            }
            if self.revisiting && kind == LocalItemKind::File
                && existing.remote_id.as_deref() == Some(&entry.id)
                && matches!(&existing.observed, ItemState::Present(current)
                    if current.kind == LocalItemKind::File
                        && (current.name != snapshot.name || current.parent != snapshot.parent))
            {
                self.report.already_registered += 1;
                return Ok(existing.id.clone());
            }
            let expected = ItemState::Present(snapshot);
            if existing.remote_id.as_deref() != Some(&entry.id)
                || existing.confirmed.as_ref() != Some(&expected) || existing.observed != expected
            {
                return Err(EngineError::DownloadConflict);
            }
            if let (Some(pending), Some(digest), ItemState::Present(current)) =
                (pending, entry.sha256_checksum.as_ref(), &existing.observed)
            {
                if current.kind == LocalItemKind::File && current.content_version.is_none()
                    && current.remote_version.as_ref() == Some(&entry.version)
                    && verify_digest(pending, &entry.name, digest).is_ok()
                {
                    let mut confirmed = current.clone();
                    confirmed.content_version = Some(digest.clone());
                    self.store.commit_remote_file(
                        self.scope, &existing.id, &existing.observed, ItemState::Present(confirmed),
                    ).map_err(EngineError::DownloadState)?;
                }
            }
            self.report.already_registered += usize::from(parent.is_some());
            return Ok(existing.id.clone());
        }
        snapshot.content_version = install()?;
        for _ in 0..8 {
            match self.store.register_verified_item(
                self.scope,
                checkpoint.as_ref().map(|c| c.generation),
                Some(entry.id.clone()),
                snapshot.clone(),
                true,
            ) {
                Ok((id, _)) => {
                    if parent.is_some() {
                        if kind == LocalItemKind::Directory {
                            self.report.directories += 1;
                        } else {
                            self.report.files += 1;
                        }
                    }
                    return Ok(id);
                }
                Err(LocalStateError::Stale) => {
                    checkpoint = self
                        .store
                        .load(self.scope)
                        .map_err(EngineError::DownloadState)?;
                }
                Err(error) => return Err(EngineError::DownloadState(error)),
            }
        }
        Err(EngineError::DownloadState(LocalStateError::Stale))
    }
}

/// Reconciles a registered file within the same parent. Ambiguous
/// disappearances, changed local content and unverified baselines remain pending.
pub(crate) fn reconcile_file(
    token: &str,
    folder: &FolderConfiguration,
    scope: &LocalScope,
    event: &crate::RemoteEvent,
    notify: &dyn Fn(&Path, DownloadProgress),
) -> Result<bool, EngineError> {
    let Some(remote) = &event.current else {
        return Ok(false);
    };
    if event.file_id.as_deref() != Some(&remote.id)
        || event.before != crate::RemoteMembership::Inside
        || (remote.trashed != Some(true) && event.after != crate::RemoteMembership::Inside)
    {
        return Ok(false);
    }
    let root = fs::openat2(
        CWD,
        &folder.local,
        DIR,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|_| EngineError::DownloadLocal)?;
    fs::flock(&root, fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| EngineError::DownloadBusy)?;
    let store = LocalStateStore::portable(&folder.local, scope, &folder.remote.id)
        .map_err(EngineError::DownloadState)?;
    let checkpoint = store.load(scope).map_err(EngineError::DownloadState)?;
    let Some(state) = checkpoint else {
        return Ok(false);
    };
    let Some(item) = state
        .items
        .iter()
        .find(|item| item.remote_id.as_deref() == Some(&remote.id))
    else {
        return Ok(false);
    };
    if item.confirmed.as_ref() != Some(&item.observed) {
        return Ok(false);
    }
    let ItemState::Present(snapshot) = &item.observed else {
        return Ok(remote.trashed == Some(true));
    };
    let (Some(old_digest), Some(old_version), Some(parent)) = (
        &snapshot.content_version,
        &snapshot.remote_version,
        &snapshot.parent,
    ) else {
        return Ok(false);
    };
    if snapshot.kind != LocalItemKind::File {
        return Ok(false);
    }
    let Some(parent_remote) = state
        .items
        .iter()
        .find(|item| &item.id == parent)
        .and_then(|item| item.remote_id.as_deref())
    else {
        return Ok(false);
    };
    let entry = match metadata(token, &remote.id) {
        Ok(entry) => entry,
        Err(EngineError::RemoteFolderUnavailable | EngineError::RemoteForbidden) => {
            return Ok(false)
        }
        Err(error) => return Err(error),
    };
    if entry.id != remote.id
        || !valid_name(&entry.name)
        || (entry.trashed && (!entry.explicitly_trashed || entry.name.as_bytes() != snapshot.name))
        || entry.parents.as_slice() != [parent_remote]
        || entry.trashed != (remote.trashed == Some(true))
        || entry.mime_type.starts_with("application/vnd.google-apps.")
        || (!entry.trashed && entry.kind()? != LocalItemKind::File)
    {
        return Ok(false);
    }
    if !entry.trashed && old_version == &entry.version {
        return Ok(true);
    }
    let renamed = entry.name.as_bytes() != snapshot.name;
    let content_changed = if entry.trashed {
        false
    } else if let Some(digest) = &entry.sha256_checksum {
        digest != old_digest
    } else {
        return Ok(false);
    };
    if !entry.trashed && !renamed && !content_changed {
        let mut next = snapshot.clone();
        next.remote_version = Some(entry.version);
        store
            .commit_remote_file(scope, &item.id, &item.observed, ItemState::Present(next))
            .map_err(EngineError::DownloadState)?;
        return Ok(true);
    }
    let remote_time = entry
        .modified_time
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|date| date.to_utc());
    if !entry.trashed && remote_time.is_none() {
        return Ok(false);
    }
    let paths = store
        .item_paths(scope, true)
        .map_err(EngineError::DownloadState)?;
    let Some((path, _)) = paths.iter().find(|(_, id)| *id == &item.id) else {
        return Ok(false);
    };
    let path = Path::new(std::ffi::OsStr::from_bytes(path));
    let parent_path = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = File::from(
        fs::openat2(
            &root,
            parent_path,
            DIR,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )
        .map_err(|_| EngineError::DownloadConflictAt(path.into()))?,
    );
    let old_name = std::str::from_utf8(&snapshot.name)
        .map_err(|_| EngineError::DownloadConflictAt(path.into()))?;
    let destination = path.with_file_name(&entry.name);
    let pending = PendingFile::new(&entry, &directory);
    let backup = format!(
        ".drivesync-download-{:x}.backup",
        Sha256::digest(format!("{}\0{}\0{}", entry.id, old_version, old_name).as_bytes())
    );
    let transfer = Transfer {
        notify,
        path: &destination,
        active: Cell::new(false),
    };
    let recovering = pending.stat(&backup)?.is_some();
    let local_name = if recovering { &backup } else { old_name };
    if !entry.trashed && !is_remote_newer(&pending.open(local_name)?, remote_time)? {
        return Ok(false);
    }
    if !recovering {
        verify_digest(&pending, old_name, old_digest)?;
        if renamed && pending.stat(&entry.name)?.is_some() {
            return Err(EngineError::DownloadConflictAt(destination.clone()));
        }
        if content_changed && pending.stat(&pending.staged)?.is_none() {
            download_file(token, &pending, |progress| transfer.emit(progress))?;
        }
        if renamed && !content_changed && metadata(token, &entry.id)? != entry {
            return Ok(false);
        }
        if !entry.trashed && !is_remote_newer(&pending.open(old_name)?, remote_time)? {
            return Ok(false);
        }
        preserve_confirmed_file(&pending, old_name, &backup, old_digest)?;
    } else {
        verify_digest(&pending, &backup, old_digest)?;
        if renamed && pending.stat(old_name)?.is_some() {
            return Err(EngineError::DownloadConflictAt(path.into()));
        }
    }
    let after = if entry.trashed {
        if pending.stat(old_name)?.is_some() {
            return Err(EngineError::DownloadConflictAt(path.into()));
        }
        ItemState::Absent
    } else {
        let digest = if content_changed {
            pending.install(token, |progress| transfer.emit(progress))?
        } else {
            publish_rename(&pending, &backup)?;
            old_digest.clone()
        };
        let mut next = snapshot.clone();
        next.name = entry.name.as_bytes().to_vec();
        next.content_version = Some(digest);
        next.remote_version = Some(entry.version.clone());
        ItemState::Present(next)
    };
    store
        .commit_remote_file(scope, &item.id, &item.observed, after)
        .map_err(EngineError::DownloadState)?;
    use crate::{LocalActivity, RemoteActivity};
    let (action, activity) = match (entry.trashed, renamed, content_changed) {
        (true, _, _) => (RemoteActivity::Removed, LocalActivity::Removed),
        (false, true, true) => (
            RemoteActivity::RenamedAndUpdated,
            LocalActivity::Moved { from: path.into() },
        ),
        (false, true, false) => (
            RemoteActivity::Renamed,
            LocalActivity::Moved { from: path.into() },
        ),
        _ => (RemoteActivity::Updated, LocalActivity::Written),
    };
    transfer.complete();
    if renamed && !content_changed {
        fs::unlinkat(&directory, backup.as_str(), fs::AtFlags::empty()).map_err(local_error)?;
    }
    crate::ActivityHistory::append_remote(
        scope,
        crate::LocalNotice {
            path: destination.clone(),
            object_type: LocalItemKind::File,
            activity,
        },
        action,
    )
    .map_err(EngineError::ActivityHistory)?;
    pending.clear()?;
    Ok(true)
}

fn is_remote_newer(
    file: &File,
    remote: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<bool, EngineError> {
    let local = file
        .metadata()
        .and_then(|metadata| metadata.modified())
        .map_err(|_| EngineError::DownloadLocal)?;
    Ok(remote.is_some_and(|remote| remote > chrono::DateTime::<chrono::Utc>::from(local)))
}

fn publish_rename(pending: &PendingFile<'_>, backup: &str) -> Result<(), EngineError> {
    if let Some(target) = pending.stat(&pending.entry.name)? {
        let source = pending.stat(backup)?.ok_or(EngineError::DownloadConflict)?;
        if (target.st_dev, target.st_ino) != (source.st_dev, source.st_ino) {
            return Err(EngineError::DownloadConflict);
        }
    } else {
        fs::linkat(
            pending.directory,
            backup,
            pending.directory,
            pending.entry.name.as_str(),
            fs::AtFlags::empty(),
        )
        .map_err(local_error)?;
    }
    pending
        .directory
        .sync_all()
        .map_err(|_| EngineError::DownloadLocal)
}

fn preserve_confirmed_file(
    pending: &PendingFile<'_>,
    source: &str,
    backup: &str,
    expected: &str,
) -> Result<(), EngineError> {
    verify_digest(pending, source, expected)?;
    fs::renameat_with(
        pending.directory,
        source,
        pending.directory,
        backup,
        RenameFlags::NOREPLACE,
    )
    .map_err(local_error)?;
    pending
        .directory
        .sync_all()
        .map_err(|_| EngineError::DownloadLocal)?;
    if let Err(error) = verify_digest(pending, backup, expected) {
        let _ = fs::renameat_with(
            pending.directory,
            backup,
            pending.directory,
            source,
            RenameFlags::NOREPLACE,
        );
        return Err(error);
    }
    Ok(())
}

fn verify_digest(pending: &PendingFile<'_>, name: &str, expected: &str) -> Result<(), EngineError> {
    if hex_digest(checksum(pending.open(name)?)?) != expected {
        return Err(EngineError::DownloadConflict);
    }
    Ok(())
}

fn local_error(error: rustix::io::Errno) -> EngineError {
    if error == rustix::io::Errno::EXIST {
        EngineError::DownloadConflict
    } else {
        EngineError::DownloadLocal
    }
}

struct Transfer<'a> {
    notify: &'a dyn Fn(&Path, DownloadProgress),
    path: &'a Path,
    active: Cell<bool>,
}

impl Transfer<'_> {
    fn emit(&self, progress: DownloadProgress) {
        self.active.set(true);
        (self.notify)(self.path, progress);
    }

    fn complete(&self) {
        if self.active.replace(false) {
            (self.notify)(self.path, DownloadProgress::Completed);
        }
    }
}

impl Drop for Transfer<'_> {
    fn drop(&mut self) {
        if self.active.get() {
            (self.notify)(self.path, DownloadProgress::Failed);
        }
    }
}

/// A verified staged file and its digest survive until the identity commits.
struct PendingFile<'a> {
    entry: &'a Entry,
    directory: &'a File,
    staged: String,
    digest: String,
}

impl<'a> PendingFile<'a> {
    fn new(entry: &'a Entry, directory: &'a File) -> Self {
        let key =
            Sha256::digest(format!("{}\0{}\0{}", entry.id, entry.version, entry.name).as_bytes());
        Self {
            entry,
            directory,
            staged: format!(".drivesync-download-{key:x}.ready"),
            digest: format!(".drivesync-download-{key:x}.digest"),
        }
    }

    fn install(
        &self,
        token: &str,
        notify: impl Fn(DownloadProgress),
    ) -> Result<String, EngineError> {
        let target = self.stat(&self.entry.name)?;
        let staged = self.stat(&self.staged)?;
        let digest = if let Some(target) = target {
            let Some(staged) = staged else {
                return Err(EngineError::DownloadConflict);
            };
            if (target.st_dev, target.st_ino) != (staged.st_dev, staged.st_ino) {
                return Err(EngineError::DownloadConflict);
            }
            self.verify()?
        } else {
            if staged.is_none() {
                download_file(token, self, notify)?;
            }
            let digest = self.verify()?;
            fs::linkat(
                self.directory,
                self.staged.as_str(),
                self.directory,
                self.entry.name.as_str(),
                fs::AtFlags::empty(),
            )
            .map_err(local_error)?;
            digest
        };
        self.directory
            .sync_all()
            .map_err(|_| EngineError::DownloadLocal)?;
        Ok(digest)
    }

    fn stat(&self, name: &str) -> Result<Option<fs::Stat>, EngineError> {
        match fs::statat(self.directory, name, fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => Ok(Some(stat)),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(error) => Err(local_error(error)),
        }
    }

    fn verify(&self) -> Result<String, EngineError> {
        let staged = self.open(&self.staged)?;
        let size = self
            .entry
            .size
            .as_deref()
            .and_then(|size| size.parse::<u64>().ok())
            .ok_or(EngineError::DownloadIncomplete)?;
        if staged
            .metadata()
            .map_err(|_| EngineError::DownloadLocal)?
            .len()
            != size
        {
            return Err(EngineError::DownloadConflict);
        }
        let mut expected = Vec::new();
        self.open(&self.digest)?
            .take(33)
            .read_to_end(&mut expected)
            .map_err(|_| EngineError::DownloadLocal)?;
        let actual = checksum(staged.take(size.saturating_add(1)))?;
        if actual.as_slice() != expected {
            return Err(EngineError::DownloadConflict);
        }
        Ok(hex_digest(actual))
    }

    fn open(&self, name: &str) -> Result<File, EngineError> {
        let file = fs::openat2(
            self.directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )
        .map(File::from)
        .map_err(|_| EngineError::DownloadConflict)?;
        if !file.metadata().is_ok_and(|m| m.is_file()) {
            return Err(EngineError::DownloadConflict);
        }
        Ok(file)
    }

    fn clear(&self) -> Result<(), EngineError> {
        for name in [&self.staged, &self.digest] {
            match fs::unlinkat(self.directory, name.as_str(), fs::AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(local_error(error)),
            }
        }
        Ok(())
    }
}

fn hex_digest(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn checksum(mut input: impl Read) -> Result<[u8; 32], EngineError> {
    let mut digest = Sha256::new();
    io::copy(&mut input, &mut digest).map_err(|_| EngineError::DownloadLocal)?;
    Ok(digest.finalize().into())
}

fn copy_with_progress(
    mut input: impl Read,
    mut output: impl Write,
    total: u64,
    notify: impl Fn(DownloadProgress),
) -> Result<u64, EngineError> {
    let mut buffer = [0; 64 * 1024];
    let mut received = 0;
    let mut last_update = Instant::now();
    loop {
        let count = match input.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result.map_err(|_| EngineError::Authentication(crate::AuthError::Network))?,
        };
        if count == 0 {
            return Ok(received);
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| EngineError::DownloadLocal)?;
        received += count as u64;
        if received <= total
            && (received == total || last_update.elapsed() >= Duration::from_millis(100))
        {
            notify(DownloadProgress::Receiving { received, total });
            last_update = Instant::now();
        }
    }
}

fn download_file(
    token: &str,
    pending: &PendingFile<'_>,
    notify: impl Fn(DownloadProgress),
) -> Result<(), EngineError> {
    let entry = pending.entry;
    let directory = pending.directory;
    let size = entry
        .size
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .and_then(|size| size.checked_add(1))
        .ok_or(EngineError::DownloadIncomplete)?;
    notify(DownloadProgress::Started { total: size - 1 });
    let response = remote::stream(
        token,
        &format!("{FILES}/{}", entry.id),
        &[("alt", "media"), ("supportsAllDrives", "true")],
        Duration::from_secs(600),
    )?;
    let status = response.status().as_u16();
    if status != 200 {
        remote::response_status(status, &[])?;
        return Err(EngineError::DownloadIncomplete);
    }
    let suffix = oauth2::CsrfToken::new_random();
    let temporary = format!(".drivesync-download-{}", suffix.secret());
    let mut file = File::from(
        fs::openat(
            directory,
            temporary.as_str(),
            OFlags::CREATE | OFlags::EXCL | OFlags::RDWR | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(local_error)?,
    );
    let result = (|| {
        let copied = copy_with_progress(response.take(size), &mut file, size - 1, notify)?;
        if copied != size - 1 || metadata(token, &entry.id)? != *entry {
            return Err(EngineError::DownloadIncomplete);
        }
        if let Some(time) = entry
            .modified_time
            .as_deref()
            .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
        {
            file.set_times(std::fs::FileTimes::new().set_modified(time.into()))
                .map_err(|_| EngineError::DownloadLocal)?;
        }
        file.sync_all().map_err(|_| EngineError::DownloadLocal)?;
        file.rewind().map_err(|_| EngineError::DownloadLocal)?;
        let digest = checksum(&file)?;
        if entry
            .sha256_checksum
            .as_ref()
            .is_some_and(|expected| *expected != hex_digest(digest))
        {
            return Err(EngineError::DownloadIncomplete);
        }
        // A proof without a ready file cannot authorize adopting a destination.
        match fs::unlinkat(directory, pending.digest.as_str(), fs::AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(local_error(error)),
        }
        let mut proof = File::from(
            fs::openat(
                directory,
                pending.digest.as_str(),
                OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(local_error)?,
        );
        proof
            .write_all(&digest)
            .and_then(|()| proof.sync_all())
            .map_err(|_| EngineError::DownloadLocal)?;
        fs::renameat_with(
            directory,
            temporary.as_str(),
            directory,
            pending.staged.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(local_error)?;
        directory.sync_all().map_err(|_| EngineError::DownloadLocal)
    })();
    if result.is_err() {
        let _ = fs::unlinkat(directory, temporary.as_str(), fs::AtFlags::empty());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_progress_counts_written_bytes_without_marking_installation_complete() {
        let input = b"download body";
        let updates = std::cell::RefCell::new(Vec::new());
        let mut destination = Vec::new();
        let total = input.len() as u64;
        let copied = copy_with_progress(&input[..], &mut destination, total, |progress| {
            updates.borrow_mut().push(progress);
        })
        .unwrap();
        assert_eq!(copied, total);
        assert_eq!(destination, input);
        assert_eq!(
            updates.into_inner(),
            [DownloadProgress::Receiving {
                received: total,
                total
            }]
        );
    }

    #[test]
    fn remote_names_cannot_escape_or_collide_with_staging() {
        for name in [
            "",
            ".",
            "..",
            "/root",
            "a/b",
            "a\0b",
            ".drivesync-download-temp",
            ".drivesync",
        ] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("Relatório final.pdf"));
    }

    #[test]
    fn interrupted_publication_is_recovered_without_adopting_a_collision_or_edit() {
        let suffix = oauth2::CsrfToken::new_random();
        let path = std::env::temp_dir().join(format!("drivesync-recovery-{}", suffix.secret()));
        std::fs::create_dir(&path).unwrap();
        let directory = File::open(&path).unwrap();
        let entry = Entry {
            id: "drive-id".into(),
            name: "file".into(),
            mime_type: "application/octet-stream".into(),
            version: "1".into(),
            size: Some("4".into()),
            sha256_checksum: None,
            modified_time: None,
            trashed: false,
            explicitly_trashed: false,
            parents: vec![],
        };
        let pending = PendingFile::new(&entry, &directory);
        std::fs::write(path.join(&pending.staged), b"data").unwrap();
        std::fs::write(path.join(&pending.digest), checksum(&b"data"[..]).unwrap()).unwrap();

        let target = path.join(&entry.name);
        std::fs::write(&target, b"data").unwrap();
        assert_eq!(
            pending.install("", |_| {}),
            Err(EngineError::DownloadConflict)
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"data");
        std::fs::remove_file(&target).unwrap();

        pending.install("", |_| {}).unwrap();
        // Simulate restart before committing the identity.
        PendingFile::new(&entry, &directory)
            .install("", |_| {})
            .unwrap();
        std::fs::write(&target, b"edit").unwrap();
        assert_eq!(
            pending.install("", |_| {}),
            Err(EngineError::DownloadConflict)
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"edit");
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn remote_removal_preserves_the_confirmed_file_and_refuses_local_edits() {
        let suffix = oauth2::CsrfToken::new_random();
        let path = std::env::temp_dir().join(format!("drivesync-remote-trash-{}", suffix.secret()));
        std::fs::create_dir(&path).unwrap();
        let directory = File::open(&path).unwrap();
        let entry = Entry {
            id: "drive-id".into(),
            name: "file".into(),
            mime_type: "application/octet-stream".into(),
            version: "2".into(),
            size: Some("4".into()),
            sha256_checksum: None,
            modified_time: None,
            trashed: true,
            explicitly_trashed: true,
            parents: vec![],
        };
        let pending = PendingFile::new(&entry, &directory);
        let digest = hex_digest(checksum(&b"base"[..]).unwrap());
        std::fs::write(path.join("file"), b"edit").unwrap();
        assert_eq!(
            preserve_confirmed_file(&pending, "file", "backup", &digest),
            Err(EngineError::DownloadConflict)
        );
        assert_eq!(std::fs::read(path.join("file")).unwrap(), b"edit");
        std::fs::write(path.join("file"), b"base").unwrap();
        preserve_confirmed_file(&pending, "file", "backup", &digest).unwrap();
        assert!(!path.join("file").exists());
        verify_digest(&pending, "backup", &digest).unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rename_uses_the_original_bytes_and_requires_a_strictly_newer_remote_time() {
        let suffix = oauth2::CsrfToken::new_random();
        let path = std::env::temp_dir().join(format!("drivesync-rename-{}", suffix.secret()));
        std::fs::create_dir(&path).unwrap();
        let directory = File::open(&path).unwrap();
        let entry = Entry {
            id: "drive-id".into(),
            name: "new".into(),
            mime_type: "application/octet-stream".into(),
            version: "2".into(),
            size: Some("4".into()),
            sha256_checksum: None,
            modified_time: None,
            trashed: false,
            explicitly_trashed: false,
            parents: vec![],
        };
        let pending = PendingFile::new(&entry, &directory);
        std::fs::write(path.join("old"), b"base").unwrap();
        let original = pending.open("old").unwrap();
        let local =
            chrono::DateTime::<chrono::Utc>::from(original.metadata().unwrap().modified().unwrap());
        assert!(!is_remote_newer(&original, None).unwrap());
        assert!(!is_remote_newer(&original, Some(local)).unwrap());
        assert!(
            is_remote_newer(&original, Some(local + chrono::TimeDelta::milliseconds(1))).unwrap()
        );

        let digest = hex_digest(checksum(&b"base"[..]).unwrap());
        preserve_confirmed_file(&pending, "old", "backup", &digest).unwrap();
        std::fs::write(path.join("new"), b"edit").unwrap();
        assert_eq!(
            publish_rename(&pending, "backup"),
            Err(EngineError::DownloadConflict)
        );
        assert_eq!(std::fs::read(path.join("new")).unwrap(), b"edit");
        std::fs::remove_file(path.join("new")).unwrap();
        publish_rename(&pending, "backup").unwrap();
        publish_rename(&pending, "backup").unwrap();
        assert_eq!(std::fs::read(path.join("new")).unwrap(), b"base");
        assert!(!path.join("old").exists());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn retry_keeps_the_registered_identity_and_does_not_install_again() {
        let suffix = oauth2::CsrfToken::new_random();
        let directory =
            std::env::temp_dir().join(format!("drivesync-download-test-{}", suffix.secret()));
        let destination = directory.join("destination");
        let state = directory.join("state");
        std::fs::create_dir_all(&destination).unwrap();
        let scope = LocalScope::new("client", "account", &destination).unwrap();
        let mut job = Job {
            token: "",
            notify: &|_, _| {},
            selected: None,
            revisiting: false,
            scope: &scope,
            store: LocalStateStore::open(&state).unwrap(),
            root: File::open(&destination).unwrap(),
            seen: HashSet::new(),
            report: InitialDownload::default(),
        };
        let entry = Entry {
            id: "remote-id".into(),
            name: "My Drive".into(),
            mime_type: FOLDER.into(),
            version: "1".into(),
            size: None,
            sha256_checksum: None,
            modified_time: None,
            trashed: false,
            explicitly_trashed: false,
            parents: vec![],
        };
        let first = job
            .register(&entry, None, LocalItemKind::Directory, None, || Ok(None))
            .unwrap();
        assert!(!job.store.initial_download_complete(&scope).unwrap());
        job.store.finish_initial_download(&scope, &first).unwrap();
        drop(job.store);
        job.store = LocalStateStore::open(&state).unwrap();
        assert!(job.store.initial_download_complete(&scope).unwrap());
        let second = job
            .register(&entry, None, LocalItemKind::Directory, None, || {
                panic!("already registered")
            })
            .unwrap();
        assert_eq!(first, second);
        let other = Entry {
            id: "other-id".into(),
            ..entry
        };
        assert_eq!(
            job.register(&other, None, LocalItemKind::Directory, None, || panic!(
                "conflicting root"
            )),
            Err(EngineError::DownloadConflict)
        );
        drop(job);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
