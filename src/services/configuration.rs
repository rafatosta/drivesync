//! Completed setup choices, atomically persisted only after fresh validation.
//! These settings do not start or claim a running synchronization process.
use adw::gtk::{gio, glib};
use drivesync_engine::{AccountId, AccountState, Engine, ListAccountsRequest};
use gettextrs::gettext;
use gio::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Configuration {
    pub account_id: AccountId,
    pub local_uri: String,
    pub remote_id: String,
    pub remote_path: String,
}
fn invalid() -> glib::Error {
    glib::Error::new(
        gio::IOErrorEnum::InvalidData,
        "Invalid completed setup configuration",
    )
}
impl Configuration {
    fn encode(&self) -> String {
        let key = glib::KeyFile::new();
        key.set_integer("Configuration", "version", 1);
        key.set_string("Configuration", "account-id", &self.account_id.0);
        key.set_string("Configuration", "local-uri", &self.local_uri);
        key.set_string("Configuration", "remote-id", &self.remote_id);
        key.set_string("Configuration", "remote-path", &self.remote_path);
        key.to_data().into()
    }
    fn decode(account_id: &AccountId, text: &str) -> Result<Self, glib::Error> {
        let key = glib::KeyFile::new();
        key.load_from_data(text, glib::KeyFileFlags::NONE)?;
        if key.integer("Configuration", "version")? != 1
            || key.string("Configuration", "account-id")?.as_str() != account_id.0
        {
            return Err(invalid());
        }
        let value = Self {
            account_id: account_id.clone(),
            local_uri: key.string("Configuration", "local-uri")?.into(),
            remote_id: key.string("Configuration", "remote-id")?.into(),
            remote_path: key.string("Configuration", "remote-path")?.into(),
        };
        if value.remote_id.is_empty()
            || value.remote_path.is_empty()
            || !value.local_uri.starts_with("file://")
            || gio::File::for_uri(&value.local_uri).path().is_none()
        {
            return Err(invalid());
        }
        Ok(value)
    }
}
fn file(id: &AccountId) -> gio::File {
    let name: String = id.0.bytes().map(|b| format!("{b:02x}")).collect();
    gio::File::for_path(
        glib::user_config_dir()
            .join("drivesync/connections")
            .join(format!("{name}.ini")),
    )
}
async fn read(file: &gio::File, id: &AccountId) -> Result<Option<Configuration>, glib::Error> {
    match file.load_contents_future().await {
        Ok((bytes, _)) => {
            Configuration::decode(id, std::str::from_utf8(&bytes).map_err(|_| invalid())?).map(Some)
        }
        Err(error) if error.matches(gio::IOErrorEnum::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}
/// Absence is incomplete setup; unreadable or malformed settings are never absence.
pub(crate) async fn load(id: &AccountId) -> Result<Option<Configuration>, glib::Error> {
    read(&file(id), id).await
}
async fn write(file: &gio::File, value: &Configuration) -> Result<(), glib::Error> {
    use std::os::unix::fs::DirBuilderExt;
    let parent = file.parent().and_then(|p| p.path()).ok_or_else(invalid)?;
    gio::spawn_blocking(move || {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
    })
    .await
    .map_err(|_| invalid())?
    .map_err(|_| {
        glib::Error::new(
            gio::IOErrorEnum::Failed,
            "Could not prepare configuration storage",
        )
    })?;
    file.replace_contents_future(
        value.encode().into_bytes(),
        None,
        false,
        gio::FileCreateFlags::PRIVATE | gio::FileCreateFlags::REPLACE_DESTINATION,
    )
    .await
    .map(|_| ())
    .map_err(|(_, e)| e)
}
/// Rechecks account authorization, local access and the exact remote folder before
/// replacing saved choices. Failure leaves any previously completed record intact.
pub(crate) async fn save_checked(
    engine: &Engine,
    value: Configuration,
) -> Result<Configuration, String> {
    let target = file(&value.account_id);
    save_checked_to(engine, value, &target, None).await
}
/// Final setup also applies an optional local nickname through the Accounts API.
pub(crate) async fn save_checked_with_nickname(
    engine: &Engine,
    value: Configuration,
    nickname: &str,
) -> Result<Configuration, String> {
    let target = file(&value.account_id);
    save_checked_to(engine, value, &target, Some(nickname)).await
}
async fn save_checked_to(
    engine: &Engine,
    value: Configuration,
    target: &gio::File,
    nickname: Option<&str>,
) -> Result<Configuration, String> {
    Configuration::decode(&value.account_id, &value.encode())
        .map_err(|_| gettext("The configuration is incomplete."))?;
    let accounts = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .map_err(|e| crate::core::messages::engine_error(&e))?;
    let Some(account) = accounts
        .accounts
        .iter()
        .find(|a| a.id == value.account_id && a.state == AccountState::Connected)
    else {
        return Err(gettext("Authorize your account before completing setup."));
    };
    super::setup::check_local_folder(&value.local_uri)
        .await
        .map_err(|_| gettext("The selected folder is unavailable. Choose another folder."))?;
    engine
        .remote_folder(value.account_id.clone(), value.remote_id.clone())
        .await
        .map_err(|e| crate::core::messages::engine_error(&e))?;
    let name = nickname.map(str::trim).filter(|name| !name.is_empty());
    let changed = name.is_some() && name != account.display_name.as_deref();
    if changed {
        super::accounts::rename(engine, value.account_id.clone(), name.map(str::to_owned))
            .await
            .map_err(|e| crate::core::messages::engine_error(&e))?;
    }
    if write(target, &value).await.is_err() {
        if changed
            && super::accounts::rename(
                engine,
                value.account_id.clone(),
                account.display_name.clone(),
            )
            .await
            .is_err()
        {
            return Err(gettext("The configuration could not be saved or the previous account name restored. Try again."));
        }
        return Err(gettext("The configuration could not be saved. Try again."));
    }
    Ok(value)
}
/// Called only after the owning account has been disconnected successfully.
pub(crate) async fn remove(id: &AccountId) -> Result<(), glib::Error> {
    match file(id).delete_future(glib::Priority::DEFAULT).await {
        Ok(()) => Ok(()),
        Err(e) if e.matches(gio::IOErrorEnum::NotFound) => Ok(()),
        Err(e) => Err(e),
    }
}
#[cfg(test)]
pub(crate) use tests::seed as seed_for_test;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Configuration {
        Configuration {
            account_id: AccountId("fixture/account".into()),
            local_uri: "file:///tmp/with%20spaces".into(),
            remote_id: "opaque:/folder".into(),
            remote_path: "My Drive / Folder".into(),
        }
    }
    #[test]
    fn completed_record_rejects_legacy_drafts_wrong_accounts_and_invalid_fields() {
        let value = fixture();
        assert_eq!(
            Configuration::decode(&value.account_id, &value.encode()).unwrap(),
            value
        );
        assert!(Configuration::decode(&AccountId("other".into()), &value.encode()).is_err());
        assert!(
            Configuration::decode(&value.account_id, "[Setup]\nlocal-uri=file:///tmp").is_err()
        );
        for bad in [
            Configuration {
                remote_id: String::new(),
                ..value.clone()
            },
            Configuration {
                local_uri: "https://example.invalid".into(),
                ..value.clone()
            },
        ] {
            assert!(Configuration::decode(&bad.account_id, &bad.encode()).is_err());
        }
        assert!(file(&AccountId("../../escape".into()))
            .basename()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("2e2e2f"));
    }
    #[test]
    fn completed_records_replace_atomically_and_corruption_is_not_absence() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "drivesync-completion-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("connections/account.ini");
        let file = gio::File::for_path(&path);
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    let mut value = fixture();
                    assert!(read(&file, &value.account_id).await.unwrap().is_none());
                    write(&file, &value).await.unwrap();
                    assert_eq!(
                        read(&file, &value.account_id).await.unwrap(),
                        Some(value.clone())
                    );
                    value.remote_id = "new-id".into();
                    write(&file, &value).await.unwrap();
                    assert_eq!(
                        read(&file, &value.account_id)
                            .await
                            .unwrap()
                            .unwrap()
                            .remote_id,
                        "new-id"
                    );
                    assert_eq!(
                        std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                        0
                    );
                    std::fs::write(&path, "broken").unwrap();
                    assert!(read(&file, &value.account_id).await.is_err());
                })
            })
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn final_validation_rejects_an_absent_account_without_persisting_choices() {
        let dir = std::env::temp_dir().join(format!(
            "drivesync-validation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    let engine = Engine::open(drivesync_engine::EngineConfig {
                        data_directory: dir.clone(),
                        application_id: "com.rtosta.ConfigurationFixture".into(),
                        google_oauth_file: None,
                    })
                    .await
                    .unwrap();
                    let target = gio::File::for_path(dir.join("completed/record.ini"));
                    let original = fixture();
                    write(&target, &original).await.unwrap();
                    let mut changed = original.clone();
                    changed.remote_id = "different-folder".into();
                    let result = save_checked_to(&engine, changed, &target, Some("Work")).await;
                    assert_eq!(
                        result.unwrap_err(),
                        gettext("Authorize your account before completing setup.")
                    );
                    assert_eq!(
                        read(&target, &original.account_id).await.unwrap(),
                        Some(original)
                    );
                })
            })
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    /// UI fixtures may seed already completed records without touching real accounts.
    pub(crate) async fn seed(value: &Configuration) {
        write(&file(&value.account_id), value).await.unwrap();
    }
}
