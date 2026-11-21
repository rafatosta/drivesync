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
    fn direct_folder_keeps_its_display_name() {
        let uri = "file:///tmp/normal%20folder";
        assert_eq!(display(uri), "/tmp/normal folder");
    }
}
