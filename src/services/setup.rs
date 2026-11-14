//! In-memory setup choices and cleanup of legacy persisted drafts.
//! New setup does not persist incomplete choices or grant dashboard readiness.
use adw::gtk::{gio, glib};
use drivesync_engine::{providers, AccountId};
use gio::prelude::*;
pub(crate) use providers::{ConnectionMethod, RegisteredProvider};

/// Reads the owning provider catalog, without a second UI registry.
pub(crate) fn list_providers() -> Vec<RegisteredProvider> {
    providers::list_providers()
}

/// Current-window choices, containing no credentials or completion flag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SetupDraft {
    pub local_uri: Option<String>,
}
#[cfg(test)]
impl SetupDraft {
    fn encode(&self) -> glib::GString {
        let file = glib::KeyFile::new();
        if let Some(uri) = &self.local_uri {
            file.set_string("Setup", "local-uri", uri);
        }
        file.to_data()
    }
    fn decode(data: &str) -> Result<Self, glib::Error> {
        let file = glib::KeyFile::new();
        file.load_from_data(data, glib::KeyFileFlags::NONE)?;
        Ok(Self {
            local_uri: file.string("Setup", "local-uri").ok().map(|v| v.into()),
        })
    }
}
fn draft_file(id: &AccountId) -> gio::File {
    // Encode every byte so even untrusted identifiers cannot escape this directory.
    let name: String = id.0.bytes().map(|byte| format!("{byte:02x}")).collect();
    gio::File::for_path(
        glib::user_config_dir()
            .join("drivesync")
            .join("setup")
            .join(format!("{name}.ini")),
    )
}
/// Reads an existing draft. Missing files are normal; malformed files are errors.
#[cfg(test)]
pub(crate) async fn load_draft(id: &AccountId) -> Result<SetupDraft, glib::Error> {
    match draft_file(id).load_contents_future().await {
        Ok((bytes, _)) => {
            let text = std::str::from_utf8(&bytes).map_err(|_| {
                glib::Error::new(gio::IOErrorEnum::InvalidData, "Invalid setup draft")
            })?;
            SetupDraft::decode(text)
        }
        Err(error) if error.matches(gio::IOErrorEnum::NotFound) => Ok(SetupDraft::default()),
        Err(error) => Err(error),
    }
}
/// Creates legacy fixtures; production setup never writes incomplete choices.
#[cfg(test)]
pub(crate) async fn save_draft(id: &AccountId, draft: &SetupDraft) -> Result<(), glib::Error> {
    let file = draft_file(id);
    if let Some(parent) = file.parent().and_then(|parent| parent.path()) {
        gio::spawn_blocking(move || std::fs::create_dir_all(parent))
            .await
            .map_err(|_| {
                glib::Error::new(gio::IOErrorEnum::Failed, "Could not prepare setup storage")
            })?
            .map_err(|_| {
                glib::Error::new(gio::IOErrorEnum::Failed, "Could not prepare setup storage")
            })?;
    }
    file.replace_contents_future(
        draft.encode().as_bytes().to_vec(),
        None,
        false,
        gio::FileCreateFlags::PRIVATE,
    )
    .await
    .map(|_| ())
    .map_err(|(_, error)| error)
}
/// Checks directory type and advertised write permission through GIO without
/// modifying files. This does not prove durable portal access or sync readiness.
pub(crate) async fn check_local_folder(uri: &str) -> Result<(), glib::Error> {
    let file = gio::File::for_uri(uri);
    if file.path().is_none() {
        return Err(glib::Error::new(
            gio::IOErrorEnum::NotSupported,
            "A local folder is required",
        ));
    }
    let info = file
        .query_info_future(
            "standard::type,access::can-write",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            glib::Priority::DEFAULT,
        )
        .await?;
    if info.file_type() != gio::FileType::Directory {
        return Err(glib::Error::new(
            gio::IOErrorEnum::NotDirectory,
            "The selected location is not a folder",
        ));
    }
    if !info.has_attribute("access::can-write") || !info.boolean("access::can-write") {
        return Err(glib::Error::new(
            gio::IOErrorEnum::PermissionDenied,
            "The selected folder is not writable",
        ));
    }
    Ok(())
}
/// Human-readable path; the stored URI preserves non-UTF-8 filename bytes.
pub(crate) fn display_path(uri: &str) -> String {
    super::local_folder::display(uri)
}
/// Removes only this account's setup draft, without traversing user directories.
pub(crate) async fn discard_draft(account_id: &AccountId) -> bool {
    match draft_file(account_id)
        .delete_future(glib::Priority::DEFAULT)
        .await
    {
        Ok(()) => true,
        Err(error) => error.matches(gio::IOErrorEnum::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_folder_validation_checks_directory_type_and_current_access() {
        let path = std::env::temp_dir().join(format!(
            "drivesync-configuration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        let file = path.join("document.txt");
        std::fs::write(&file, b"preserved").unwrap();
        let link = path.join("directory-link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    assert!(
                        check_local_folder(gio::File::for_path(&path).uri().as_str())
                            .await
                            .is_ok()
                    );
                    for invalid in [&file, &link, &path.join("missing")] {
                        assert!(
                            check_local_folder(gio::File::for_path(invalid).uri().as_str())
                                .await
                                .is_err()
                        );
                    }
                    assert!(check_local_folder("https://example.invalid/folder")
                        .await
                        .is_err());
                })
            })
            .unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"preserved");
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn draft_round_trip_preserves_uri_without_completing_setup() {
        let draft = SetupDraft {
            local_uri: Some("file:///tmp/with%20spaces/%FF".into()),
        };
        assert_eq!(SetupDraft::decode(&draft.encode()).unwrap(), draft);
        assert!(SetupDraft::decode("[broken").is_err());
    }
    #[test]
    fn account_identifiers_cannot_change_draft_directory() {
        let file = draft_file(&AccountId("../../other/account".into()));
        assert_eq!(
            file.parent().unwrap().path().unwrap(),
            glib::user_config_dir().join("drivesync/setup")
        );
    }
}
