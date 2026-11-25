//! Pure Linux projection policy. Workspace links are derived artifacts, never binary replicas.
use crate::{
    inventory::models::{LocalState, RemoteState},
    models::*,
};
use sha2::{Digest, Sha256};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteKind {
    Binary,
    Directory,
    Workspace,
    RemoteShortcut,
    Unsupported,
}
pub(crate) fn kind(mime: &str) -> RemoteKind {
    match mime {
        "application/vnd.google-apps.folder" => RemoteKind::Directory,
        "application/vnd.google-apps.shortcut" => RemoteKind::RemoteShortcut,
        "application/vnd.google-apps.document"
        | "application/vnd.google-apps.spreadsheet"
        | "application/vnd.google-apps.presentation"
        | "application/vnd.google-apps.drawing"
        | "application/vnd.google-apps.form" => RemoteKind::Workspace,
        m if m.starts_with("application/vnd.google-apps.") => RemoteKind::Unsupported,
        _ => RemoteKind::Binary,
    }
}
pub(crate) const PRIVATE_TRASH_PREFIX: &str = ".drivesync-trash-";

pub(crate) fn private_trash_name(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix(PRIVATE_TRASH_PREFIX))
        .is_some_and(|key| key.len() == 32 && key.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub(crate) fn libreoffice_lock_name(name: &std::ffi::OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = name.as_bytes();
    bytes.len() > b".~lock.#".len() && bytes.starts_with(b".~lock.") && bytes.ends_with(b"#")
}

pub(crate) fn libreoffice_lock_path(path: &std::path::Path) -> bool {
    path.components()
        .any(|part| libreoffice_lock_name(part.as_os_str()))
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name != ".drivesync"
        && !private_trash_name(name.as_ref())
        && !libreoffice_lock_name(name.as_ref())
        && !name.starts_with(".drivesync-transfer-")
        && !name.contains('/')
        && !name.chars().any(char::is_control)
        && name.len() <= 255
}
pub(crate) fn local_name(name: &str, mime: &str) -> Option<String> {
    if !valid_name(name) {
        return None;
    }
    let name = if kind(mime) == RemoteKind::Workspace {
        format!("{name}.html")
    } else {
        name.to_owned()
    };
    valid_name(&name).then_some(name)
}
pub(crate) fn link_bytes(id: &RemoteId) -> Vec<u8> {
    let encoded: String =
        id.0.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-_".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
    // Percent-encoding limits the ID to characters safe in both quoted HTML attributes.
    let url = format!("https://drive.google.com/open?id={encoded}");
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<meta charset=\"utf-8\">\n<meta name=\"referrer\" content=\"no-referrer\">\n<title>Google Drive</title>\n<meta http-equiv=\"refresh\" content=\"0;url={url}\">\n<a href=\"{url}\">Open in Google Drive</a>\n</html>\n"
    )
    .into_bytes()
}
pub(crate) fn workspace(remote: &RemoteState) -> bool {
    matches!(&remote.mime_type,Evidence::Known(mime) if kind(mime)==RemoteKind::Workspace)
}
/// Checks derived bytes and projected name; remote document hashes are not local link hashes.
pub(crate) fn equivalent(local: &LocalState, remote: &RemoteState) -> bool {
    let (Some(id), Evidence::Known(name), Evidence::Known(mime)) =
        (&remote.id, &remote.name, &remote.mime_type)
    else {
        return false;
    };
    let bytes = link_bytes(id);
    workspace(remote)
        && local.state == FileState::Present
        && remote.state == FileState::Present
        && remote.trashed == Evidence::Known(false)
        && local.kind == Evidence::Known(FileKind::Regular)
        && matches!(local.identity, Evidence::Known(_))
        && local_name(name, mime).map(Evidence::Known) == Some(local.name.clone())
        && local.path.as_ref().is_some_and(|p| {
            !p.is_absolute()
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| Evidence::Known(s.to_owned()))
                    == Some(local.name.clone())
        })
        && local.size == Evidence::Known(bytes.len() as u64)
        && local.checksum
            == Evidence::Known(Checksum {
                algorithm: "sha256".into(),
                value: format!("{:x}", Sha256::digest(bytes)),
            })
}

pub(crate) fn content_matches(local: &LocalState, id: &RemoteId) -> bool {
    let bytes = link_bytes(id);
    local.state == FileState::Present
        && local.kind == Evidence::Known(FileKind::Regular)
        && local.size == Evidence::Known(bytes.len() as u64)
        && local.checksum
            == Evidence::Known(Checksum {
                algorithm: "sha256".into(),
                value: format!("{:x}", Sha256::digest(bytes)),
            })
}

/// Document revisions do not change link bytes; names, scope and supported representation still must match.
pub(crate) fn same_representation(current: &RemoteState, previous: &RemoteState) -> bool {
    workspace(current)
        && workspace(previous)
        && current.state == FileState::Present
        && current.trashed == Evidence::Known(false)
        && current.id.is_some()
        && current.id == previous.id
        && current.name == previous.name
        && current.parents == previous.parents
        && matches!((&current.version,&previous.version),(Evidence::Known(a),Evidence::Known(b)) if a>=b && *b>0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn types_names_and_link_bytes_are_explicit_and_inert() {
        assert_eq!(
            kind("application/vnd.google-apps.document"),
            RemoteKind::Workspace
        );
        assert_eq!(
            kind("application/vnd.google-apps.folder"),
            RemoteKind::Directory
        );
        assert_eq!(
            kind("application/vnd.google-apps.shortcut"),
            RemoteKind::RemoteShortcut
        );
        assert_eq!(
            kind("application/vnd.google-apps.drive-sdk"),
            RemoteKind::Unsupported
        );
        assert_eq!(kind("application/octet-stream"), RemoteKind::Binary);
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\0b",
            "a\nb",
            ".drivesync",
            ".drivesync-transfer-x",
        ] {
            assert!(!valid_name(name), "{name:?}");
        }
        assert_eq!(
            local_name("Relatório", "application/vnd.google-apps.document"),
            Some("Relatório.html".into())
        );
        assert!(local_name(&"a".repeat(251), "application/vnd.google-apps.document").is_none());
        assert!(valid_name(&"a".repeat(255)));
        for mime in ["document", "spreadsheet", "presentation", "drawing", "form"] {
            let mime = format!("application/vnd.google-apps.{mime}");
            assert_eq!(
                local_name("Relatório", &mime).as_deref(),
                Some("Relatório.html")
            );
            assert!(local_name(&"é".repeat(125), &mime).is_some());
            assert!(local_name(&"é".repeat(126), &mime).is_none());
        }
        for (name, mime) in [
            (
                "Report.docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
            ("Report.pdf", "application/pdf"),
        ] {
            assert_eq!(kind(mime), RemoteKind::Binary);
            assert_eq!(local_name(name, mime).as_deref(), Some(name));
        }
        let text = String::from_utf8(link_bytes(&RemoteId("x\n\"<&/#%é".into()))).unwrap();
        assert!(text.contains("https://drive.google.com/open?id=x%0A%22%3C%26%2F%23%25%C3%A9"));
        assert!(!text.contains("<script"));
    }
}
