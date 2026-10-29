//! Full scans recover changes missed while the application or network was offline.
use super::{Evidence, Inventory, Stamp};
use crate::{
    download::filesystem::{self, Root},
    AuthError, Cancellation, EngineError,
};
use std::{
    collections::BTreeMap,
    fs::File,
    os::unix::ffi::OsStrExt,
    path::Path,
    time::{Duration, Instant},
};

pub(crate) fn check(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}
/// Internal evidence and preserved copies must never become upload candidates.
pub(crate) fn ignored(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str().to_str().is_none_or(|s| {
            s == ".drivesync"
                || s.starts_with(".drivesync-")
                || (s.starts_with("DriveSync backup ") && s.ends_with(".bak"))
        })
    })
}
/// Explicit, conservative patterns; ordinary hidden files and `.tmp` are allowed.
pub(crate) fn temporary(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str().to_str().is_some_and(|s| {
            s.ends_with(".swp")
                || s.ends_with(".swo")
                || s.ends_with('~')
                || s.ends_with(".part")
                || s.ends_with(".crdownload")
                || s.starts_with(".goutputstream-")
                || s.starts_with("~$")
                || (s.starts_with(".~lock.") && s.ends_with('#'))
        })
    })
}
/// Reads one regular-file signature through a validated descriptor chain.
pub(crate) fn stamp(root: &Root, path: &Path) -> Result<Stamp, EngineError> {
    let (parent, name) = root.parent(path)?;
    let parent_id = filesystem::identity(&parent)?;
    let file = filesystem::read(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
    let stamp = Stamp::read(&file)?;
    root.validate_parent(path, parent_id)?;
    Ok(stamp)
}
/// Waits once for the entire candidate set, checking cancellation every 25 ms.
pub(crate) fn scan(root: &Root, cancel: &Cancellation) -> Result<Inventory, EngineError> {
    scan_with_wait(root, cancel, || {
        let start = Instant::now();
        let duration = Duration::from_millis(500);
        while start.elapsed() < duration {
            check(cancel)?;
            std::thread::sleep(
                Duration::from_millis(25).min(duration.saturating_sub(start.elapsed())),
            );
        }
        check(cancel)
    })
}
/// The injected wait makes writes inside the quiescence window deterministic in tests.
pub(super) fn scan_with_wait(
    root: &Root,
    cancel: &Cancellation,
    wait: impl FnOnce() -> Result<(), EngineError>,
) -> Result<Inventory, EngineError> {
    check(cancel)?;
    let mut entries = BTreeMap::new();
    let mut candidates = Vec::new();
    let mut pending = vec![std::path::PathBuf::new()];
    while let Some(path) = pending.pop() {
        check(cancel)?;
        root.validate()?;
        let dir = if path.as_os_str().is_empty() {
            root.file.try_clone().map_err(filesystem::io)?
        } else {
            let (p, n) = root.parent(&path)?;
            rustix::fs::openat(
                &p,
                &n,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(filesystem::io)?
        };
        let dir_id = filesystem::identity(&dir)?;
        for entry in rustix::fs::Dir::read_from(&dir).map_err(filesystem::io)? {
            check(cancel)?;
            let entry = entry.map_err(filesystem::io)?;
            let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
            if name == "." || name == ".." {
                continue;
            }
            let child = path.join(name);
            if ignored(&child) {
                continue;
            }
            if entries.len() >= 100_000 {
                return Err(EngineError::InventoryLimit);
            }
            let stat = rustix::fs::statat(&dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(filesystem::io)?;
            let kind = rustix::fs::FileType::from_raw_mode(stat.st_mode);
            let evidence = if temporary(&child) {
                Evidence::Temporary
            } else if kind == rustix::fs::FileType::Directory {
                pending.push(child.clone());
                Evidence::Directory
            } else if kind == rustix::fs::FileType::RegularFile {
                match stamp(root, &child) {
                    Ok(s) => {
                        candidates.push((child.clone(), s));
                        Evidence::Writing
                    }
                    Err(_) => Evidence::Unknown,
                }
            } else {
                Evidence::Unsupported
            };
            entries.insert(child, evidence);
        }
        if !path.as_os_str().is_empty() {
            root.validate_parent(&path, filesystem::identity(&root.parent(&path)?.0)?)?;
            root.check_directory(&path)?;
            let (p, n) = root.parent(&path)?;
            let current = rustix::fs::openat(
                &p,
                &n,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(filesystem::io)?;
            if filesystem::identity(&current)? != dir_id {
                return Err(EngineError::DownloadConflict);
            }
        }
    }
    if !candidates.is_empty() {
        wait()?;
    }
    for (path, before) in candidates {
        check(cancel)?;
        let result = (|| {
            if stamp(root, &path)? != before {
                return Ok(Evidence::Writing);
            }
            let fingerprint = filesystem::fingerprint_root(root, &path, cancel)?
                .ok_or(EngineError::DownloadConflict)?;
            if stamp(root, &path)? != before {
                return Ok(Evidence::Writing);
            }
            Ok(Evidence::Stable {
                fingerprint,
                stamp: before,
            })
        })();
        let evidence = match result {
            Ok(e) => e,
            Err(EngineError::Authentication(AuthError::Cancelled)) => {
                return Err(EngineError::Authentication(AuthError::Cancelled))
            }
            Err(EngineError::DownloadConflict) => Evidence::Writing,
            Err(_) => Evidence::Unknown,
        };
        entries.insert(path, evidence);
    }
    check(cancel)?;
    root.validate()?;
    Ok(Inventory { entries })
}
