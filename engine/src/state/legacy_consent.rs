//! Revokes old per-root upload consent on logout without enabling synchronization.
use super::{failed, Store};
use crate::{configuration::identity, EngineError, FolderConfiguration};
use rustix::fs::{self, Mode, OFlags};
use serde::Deserialize;
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};

const NAMES: [&str; 6] = [
    "state.sqlite3",
    "changes.sqlite3",
    "reconciliation.sqlite3",
    "conflict-history.sqlite3",
    "uploads.sqlite3",
    "conflict-preferences.sqlite3",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    format: u32,
    stores: BTreeMap<String, String>,
    uploads_blocked: bool,
}

/// Disables only this exact pair's former opt-in; intents and files are retained.
/// The caller must hold the account's root-association lock.
pub(crate) fn revoke_upload_consent(
    client: &str,
    folder: &FolderConfiguration,
) -> Result<(), EngineError> {
    identity::verify(folder, client)?;
    let parent = folder.local.join(".drivesync");
    let dir = fs::open(
        &parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(failed)?;
    let (name, selected) = upload_store(&dir)?;
    let path = parent.join(name);
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !selected => return Ok(()),
        Ok(metadata) if metadata.is_file() => {}
        _ => return Err(EngineError::StateStorage),
    }

    let local: PathBuf = folder.local.components().collect();
    let scope = serde_json::to_string(&(client, &folder.account_id, &folder.remote.id, &local))
        .map_err(failed)?;
    let db = Store::open_existing_format(&path, 0x44535550, 2)?;
    db.0.execute(
        "UPDATE settings SET enabled=0, root=NULL WHERE scope=?1",
        [&scope],
    )
    .map_err(failed)?;
    Ok(())
}

/// Selects the same verified store generation as the former engine.
fn upload_store(dir: &File) -> Result<(String, bool), EngineError> {
    let file = match fs::openat(
        dir,
        "recovery.json",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(rustix::io::Errno::NOENT) => {
            return match fs::openat(
                dir,
                "recovery-required",
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Err(rustix::io::Errno::NOENT) => Ok(("uploads.sqlite3".into(), false)),
                _ => Err(EngineError::StateStorage),
            };
        }
        Err(error) => return Err(failed(error)),
    };
    if !file.metadata().map_err(failed)?.is_file() {
        return Err(EngineError::StateStorage);
    }
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes).map_err(failed)?;
    if bytes.len() > 4096 {
        return Err(EngineError::StateStorage);
    }
    let selector: Selector = serde_json::from_slice(&bytes).map_err(failed)?;
    if selector.format != 1
        || selector.stores.iter().any(|(key, name)| {
            !NAMES.contains(&key.as_str())
                || !name.starts_with("recovered-")
                || !name.ends_with(key)
                || name.len() > 160
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err(EngineError::StateStorage);
    }
    let _ = selector.uploads_blocked;
    Ok(match selector.stores.get("uploads.sqlite3") {
        Some(name) => (name.clone(), true),
        None => ("uploads.sqlite3".into(), false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountId, DriveFolder};
    use rusqlite::params;

    const CLIENT: &str = "test-client";
    const SCHEMA: &str = "CREATE TABLE settings(scope TEXT PRIMARY KEY,enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),root TEXT); CREATE TABLE intents(scope TEXT NOT NULL,path TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(scope,path));";

    struct Fixture {
        path: PathBuf,
        folder: FolderConfiguration,
    }

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "drivesync-revoke-{}",
                oauth2::CsrfToken::new_random().secret()
            ));
            std::fs::create_dir(&path).unwrap();
            let folder = FolderConfiguration {
                account_id: AccountId::from_subject("test"),
                remote: DriveFolder {
                    id: "folder".into(),
                    name: "Folder".into(),
                },
                local: path.clone(),
                local_display: path.clone(),
            };
            identity::ensure(&folder, CLIENT).unwrap();
            Self { path, folder }
        }

        fn scope(&self) -> String {
            serde_json::to_string(&(
                CLIENT,
                &self.folder.account_id,
                &self.folder.remote.id,
                &self.folder.local,
            ))
            .unwrap()
        }

        fn db(&self, name: &str) -> Store {
            Store::open_format(
                &self.path.join(".drivesync").join(name),
                0x44535550,
                2,
                SCHEMA,
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn revokes_only_the_current_pair_and_preserves_intents() {
        let fixture = Fixture::new();
        let db = fixture.db("uploads.sqlite3");
        let scope = fixture.scope();
        db.0.execute(
            "INSERT INTO settings VALUES (?1,1,'identity'),('another-scope',1,'other')",
            [&scope],
        )
        .unwrap();
        db.0.execute(
            "INSERT INTO intents VALUES (?1,'file','evidence')",
            [&scope],
        )
        .unwrap();
        drop(db);

        revoke_upload_consent(CLIENT, &fixture.folder).unwrap();
        let db = fixture.db("uploads.sqlite3");
        let current: (i64, Option<String>) =
            db.0.query_row(
                "SELECT enabled,root FROM settings WHERE scope=?1",
                [&scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(current, (0, None));
        let other: i64 =
            db.0.query_row(
                "SELECT enabled FROM settings WHERE scope='another-scope'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(other, 1);
        let evidence: String =
            db.0.query_row(
                "SELECT record FROM intents WHERE scope=?1",
                params![scope],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(evidence, "evidence");
    }

    #[test]
    fn resolves_recovered_store_and_rejects_missing_or_invalid_evidence() {
        let fixture = Fixture::new();
        let parent = fixture.path.join(".drivesync");
        let recovered = "recovered-123-uploads.sqlite3";
        let db = fixture.db(recovered);
        db.0.execute(
            "INSERT INTO settings VALUES (?1,1,'identity')",
            [fixture.scope()],
        )
        .unwrap();
        drop(db);
        std::fs::write(
            parent.join("recovery.json"),
            format!(r#"{{"format":1,"stores":{{"uploads.sqlite3":"{recovered}"}},"uploads_blocked":false}}"#),
        )
        .unwrap();
        revoke_upload_consent(CLIENT, &fixture.folder).unwrap();
        let db = fixture.db(recovered);
        let enabled: i64 =
            db.0.query_row("SELECT enabled FROM settings", [], |r| r.get(0))
                .unwrap();
        assert_eq!(enabled, 0);
        drop(db);

        std::fs::write(parent.join("recovery.json"), b"invalid").unwrap();
        assert!(revoke_upload_consent(CLIENT, &fixture.folder).is_err());
        assert_eq!(
            std::fs::read(parent.join("recovery.json")).unwrap(),
            b"invalid"
        );
        std::fs::remove_file(parent.join("recovery.json")).unwrap();
        std::fs::write(parent.join("recovery-required"), b"").unwrap();
        assert!(revoke_upload_consent(CLIENT, &fixture.folder).is_err());
    }

    #[test]
    fn selected_missing_store_and_symlinks_fail_closed() {
        let fixture = Fixture::new();
        let parent = fixture.path.join(".drivesync");
        let selector = parent.join("recovery.json");
        std::fs::write(
            &selector,
            br#"{"format":1,"stores":{"uploads.sqlite3":"recovered-x-uploads.sqlite3"},"uploads_blocked":false}"#,
        )
        .unwrap();
        assert!(revoke_upload_consent(CLIENT, &fixture.folder).is_err());
        std::fs::remove_file(selector).unwrap();
        let target = fixture.path.join("user-file");
        std::fs::write(&target, b"preserve").unwrap();
        std::os::unix::fs::symlink(&target, parent.join("uploads.sqlite3")).unwrap();
        assert!(revoke_upload_consent(CLIENT, &fixture.folder).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"preserve");
    }

    #[test]
    fn absent_store_is_not_created_and_foreign_store_is_preserved() {
        let fixture = Fixture::new();
        let store = fixture.path.join(".drivesync/uploads.sqlite3");
        revoke_upload_consent(CLIENT, &fixture.folder).unwrap();
        assert!(!store.exists());
        std::fs::write(&store, b"foreign").unwrap();
        assert!(revoke_upload_consent(CLIENT, &fixture.folder).is_err());
        assert_eq!(std::fs::read(&store).unwrap(), b"foreign");
        assert!(revoke_upload_consent("other-client", &fixture.folder).is_err());
    }
}
