//! Same-filesystem freedesktop trash. Cross-device moves fail without a destructive fallback.
use super::*;

pub(super) fn move_to_trash(source: &Entry, original: &StdPath) -> Result<(), OperatorError> {
    prepare(source, original)?.commit(source)
}
pub(super) struct TrashDestination {
    destination: Entry,
}
impl TrashDestination {
    pub(super) fn commit(self, source: &Entry) -> Result<(), OperatorError> {
        rename(source, &self.destination, libc::RENAME_NOREPLACE)
    }
}
pub(super) fn prepare(
    source: &Entry,
    original: &StdPath,
) -> Result<TrashDestination, OperatorError> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or_else(invalid)?;
    prepare_to(source, original, &data)
}
fn private_child(parent: &File, name: &str) -> Result<File, OperatorError> {
    let parent_path = PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()));
    let path = parent_path.join(name);
    let child = CString::new(name).map_err(|_| invalid())?;
    if unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0 {
        let error_value = std::io::Error::last_os_error();
        if error_value.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error(error_value));
        }
    }
    let dir = directory(&path)?;
    let meta = dir.metadata().map_err(error)?;
    if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(OperatorError::new(OperatorErrorKind::PermissionDenied));
    }
    Ok(dir)
}
pub(super) fn prepare_to(
    source: &Entry,
    original: &StdPath,
    data: &StdPath,
) -> Result<TrashDestination, OperatorError> {
    let root = Filesystem::open(data)?;
    let trash = private_child(&root.root, "Trash")?;
    let files = private_child(&trash, "files")?;
    let info = private_child(&trash, "info")?;
    if std::fs::symlink_metadata(source.anchored())
        .map_err(error)?
        .dev()
        != files.metadata().map_err(error)?.dev()
    {
        return Err(OperatorError::new(OperatorErrorKind::Unavailable));
    }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let key = execution_key(&Id(format!(
        "{}-{}-{}",
        std::process::id(),
        super::super::now().unix_millis,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    let info_path = PathBuf::from(format!("/proc/self/fd/{}", info.as_raw_fd()))
        .join(format!("{key}.trashinfo"));
    let mut receipt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(info_path)
        .map_err(error)?;
    let encoded: String = original
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.~".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let seconds = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    if unsafe { libc::localtime_r(&seconds, tm.as_mut_ptr()) }.is_null() {
        return Err(invalid());
    }
    let tm = unsafe { tm.assume_init() };
    writeln!(
        receipt,
        "[Trash Info]\nPath={encoded}\nDeletionDate={:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
    .map_err(error)?;
    receipt.sync_all().map_err(error)?;
    let dest = Entry {
        parent: files,
        name: CString::new(key).unwrap(),
    };
    Ok(TrashDestination { destination: dest })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trash_preserves_bytes_and_original_location() {
        let base = std::env::temp_dir().join(format!("drivesync-trash-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        let root = Filesystem::open(&base).unwrap();
        std::fs::write(base.join("original"), b"recoverable").unwrap();
        prepare_to(
            &root.entry(StdPath::new("original")).unwrap(),
            &base.join("original"),
            &base,
        )
        .unwrap()
        .commit(&root.entry(StdPath::new("original")).unwrap())
        .unwrap();
        assert!(!base.join("original").exists());
        let file = std::fs::read_dir(base.join("Trash/files"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(file).unwrap(), b"recoverable");
        let info = std::fs::read_dir(base.join("Trash/info"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(std::fs::read_to_string(info)
            .unwrap()
            .contains(&format!("Path={}", base.join("original").display())));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn unavailable_trash_never_removes_source() {
        let base = std::env::temp_dir().join(format!("drivesync-no-trash-{}", std::process::id()));
        std::fs::create_dir(&base).unwrap();
        let root = Filesystem::open(&base).unwrap();
        std::fs::write(base.join("original"), b"preserved").unwrap();
        assert!(prepare_to(
            &root.entry(StdPath::new("original")).unwrap(),
            &base.join("original"),
            &base.join("missing")
        )
        .is_err());
        assert_eq!(std::fs::read(base.join("original")).unwrap(), b"preserved");
        std::fs::remove_dir_all(base).unwrap();
    }
}
