//! Resolve document-portal paths without treating a grant as native watch support.
use adw::gtk::{gio, glib};
use gio::prelude::*;
use std::{
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};
const HOST_PATH: &str = "xattr::document-portal.host-path";

fn is_portal(path: &Path) -> bool {
    path.starts_with(glib::user_runtime_dir().join("doc")) || path.starts_with("/run/flatpak/doc")
}
// GIO exposes xattr byte values using \xNN escaping, including literal backslashes.
fn decode_path(value: &str) -> Option<PathBuf> {
    let mut bytes = Vec::new();
    let mut input = value.as_bytes();
    while !input.is_empty() {
        if input.starts_with(b"\\x") {
            let pair = std::str::from_utf8(input.get(2..4)?).ok()?;
            bytes.push(u8::from_str_radix(pair, 16).ok()?);
            input = &input[4..];
        } else {
            bytes.push(input[0]);
            input = &input[1..];
        }
    }
    if bytes.contains(&0) {
        return None;
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    (path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir)))
    .then_some(path)
}
/// Presentation only; this does not grant access or select a watcher root.
pub(crate) fn display(uri: &str) -> String {
    let file = gio::File::for_uri(uri);
    let Some(path) = file.path() else {
        return uri.into();
    };
    if is_portal(&path) {
        if let Ok(info) = file.query_info(
            HOST_PATH,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            None::<&gio::Cancellable>,
        ) {
            if let Some(host) = info
                .attribute_string(HOST_PATH)
                .and_then(|s| decode_path(&s))
            {
                return host.display().to_string();
            }
        }
        return file
            .basename()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Selected folder".into());
    }
    path.display().to_string()
}
/// Resolves the native root for inotify. A portal-only grant is insufficient:
/// the host path must actually be accessible inside the current sandbox.
pub(crate) async fn watch_root(uri: &str) -> Result<PathBuf, String> {
    let file = gio::File::for_uri(uri);
    let path = file.path().ok_or("A local folder is required")?;
    if !is_portal(&path) {
        return Ok(path);
    }
    let info = file
        .query_info_future(
            HOST_PATH,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            glib::Priority::DEFAULT,
        )
        .await
        .map_err(|_| {
            "Could not resolve the original folder selected through the portal".to_owned()
        })?;
    let host = info.attribute_string(HOST_PATH).and_then(|s| decode_path(&s))
        .ok_or("Native monitoring requires direct access to the selected folder; the portal path does not relay external changes")?;
    let native = gio::File::for_path(&host);
    let info = native.query_info_future("standard::type", gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS, glib::Priority::DEFAULT).await
        .map_err(|_| format!("Direct access to {} is unavailable. Grant this folder in the development Flatpak manifest and restart Builder's application sandbox.", host.display()))?;
    if info.file_type() != gio::FileType::Directory {
        return Err("The original folder is not an accessible directory".into());
    }
    Ok(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_path_decoding_preserves_bytes_and_rejects_invalid_paths() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            decode_path("/tmp/with spaces/\\xff\\x5cname")
                .unwrap()
                .as_os_str()
                .as_bytes(),
            b"/tmp/with spaces/\xff\\name"
        );
        for path in [
            "relative",
            "/tmp/../other",
            "/tmp/\\x00",
            "/tmp/\\x",
            "/tmp/\\xzz",
        ] {
            assert!(decode_path(path).is_none());
        }
    }
    #[test]
    fn direct_folder_keeps_its_native_root_and_display_name() {
        let uri = "file:///tmp/normal%20folder";
        assert_eq!(display(uri), "/tmp/normal folder");
        let context = glib::MainContext::new();
        assert_eq!(
            context.block_on(watch_root(uri)).unwrap(),
            PathBuf::from("/tmp/normal folder")
        );
    }
}

#[cfg(test)]
mod portal_integration {
    use super::*;
    use drivesync_engine::{
        detectors::*,
        models::{File, FileScope, LocalRelativePath, SyncId},
        AccountId, Timestamp,
    };
    use std::{
        sync::{mpsc, Arc},
        time::{Duration, Instant},
    };
    struct Sink(mpsc::Sender<File>);
    impl LocalNotificationSink for Sink {
        fn notify(&self, event: File) -> LocalDetectorResult<()> {
            self.0.send(event).unwrap();
            Ok(())
        }
    }
    #[test]
    #[ignore = "requires a disposable document-portal export and explicit native-access fixture"]
    fn portal_display_and_native_watch_permission() {
        let uri = std::env::var("DRIVESYNC_TEST_PORTAL_URI").expect("isolated portal export");
        let original = std::env::var("DRIVESYNC_TEST_PORTAL_HOST").expect("isolated host fixture");
        let path = PathBuf::from(&original);
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".drivesync-portal-test-"));
        assert_eq!(display(&uri), original);
        let context = glib::MainContext::new();
        let root = context.block_on(watch_root(&uri));
        if std::env::var("DRIVESYNC_TEST_NATIVE_ACCESS").as_deref() == Ok("0") {
            assert!(root.unwrap_err().contains("Direct access"));
            return;
        }
        assert_eq!(root.unwrap(), path);
        let (sender, receiver) = mpsc::channel();
        let detector = LocalDetector::new(LocalDetectorCreateRequest {
            scope: LocalDetectorScope {
                account_id: AccountId("portal-test".into()),
                sync_id: SyncId("portal-test".into()),
                local_root: path.clone(),
            },
            notifications: Arc::new(Sink(sender)),
            settings: LocalDetectorSettings::default(),
        })
        .unwrap()
        .detector;
        detector
            .start(LocalStartRequest {
                mode: LocalStartMode::Initial,
                requested_at: Timestamp { unix_millis: 0 },
            })
            .unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap().scope,
            FileScope::WholeSync
        );
        std::fs::write(path.join("ready"), b"watching").unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let event = receiver
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("host-side change must reach the native detector");
            if event.scope
                == FileScope::Local(LocalRelativePath::new(b"host-only.txt".to_vec()).unwrap())
            {
                break;
            }
        }
        detector
            .stop(LocalStopRequest {
                reason: LocalStopReason::Shutdown,
                requested_at: Timestamp { unix_millis: 0 },
            })
            .unwrap();
    }
}
