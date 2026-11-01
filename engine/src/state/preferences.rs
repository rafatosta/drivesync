//! Scoped consent kept separately from observed plans and accepted intentions.
use super::{failed, Store};
use crate::{ConflictPreference, EngineError, FolderConfiguration};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

pub(crate) struct Preferences {
    db: Store,
    scope: String,
}
impl Preferences {
    pub fn open(
        path: &Path,
        client: &str,
        folder: &FolderConfiguration,
    ) -> Result<Self, EngineError> {
        let scope =
            serde_json::to_string(&(client, &folder.account_id, &folder.remote.id, &folder.local))
                .map_err(failed)?;
        Ok(Self {
            db: Store::open_format(
                path,
                0x44535046,
                1,
                "CREATE TABLE preferences(scope TEXT PRIMARY KEY, rule TEXT NOT NULL);",
            )?,
            scope,
        })
    }
    pub fn load(&self) -> Result<ConflictPreference, EngineError> {
        let value: Option<String> = self
            .db
            .0
            .query_row(
                "SELECT rule FROM preferences WHERE scope=?1",
                [&self.scope],
                |r| r.get(0),
            )
            .optional()
            .map_err(failed)?;
        value
            .map(|v| serde_json::from_str(&v).map_err(failed))
            .transpose()
            .map(|v| v.unwrap_or_default())
    }
    /// Commits consent atomically. Callers retain the association/root locks.
    pub fn save(&self, rule: ConflictPreference) -> Result<(), EngineError> {
        self.db.0.execute(
            "INSERT INTO preferences VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET rule=excluded.rule",
            params![self.scope, serde_json::to_string(&rule).map_err(failed)?]
        ).map_err(failed)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_is_scoped_persistent_and_invalid_data_is_never_ask() {
        let dir = std::env::temp_dir().join(format!(
            "drivesync-preferences-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("preferences.sqlite3");
        let mut folder = FolderConfiguration {
            account_id: crate::AccountId("one".into()),
            remote: crate::DriveFolder {
                id: "root".into(),
                name: "Root".into(),
            },
            local: dir.clone(),
            local_display: dir.clone(),
        };
        let store = Preferences::open(&path, "client", &folder).unwrap();
        assert_eq!(store.load().unwrap(), ConflictPreference::Ask);
        store.save(ConflictPreference::KeepRemote).unwrap();
        drop(store);
        assert_eq!(
            Preferences::open(&path, "client", &folder)
                .unwrap()
                .load()
                .unwrap(),
            ConflictPreference::KeepRemote
        );
        assert_eq!(
            Preferences::open(&path, "other", &folder)
                .unwrap()
                .load()
                .unwrap(),
            ConflictPreference::Ask
        );
        folder.account_id.0 = "two".into();
        assert_eq!(
            Preferences::open(&path, "client", &folder)
                .unwrap()
                .load()
                .unwrap(),
            ConflictPreference::Ask
        );
        folder.account_id.0 = "one".into();
        folder.remote.id = "other-root".into();
        assert_eq!(
            Preferences::open(&path, "client", &folder)
                .unwrap()
                .load()
                .unwrap(),
            ConflictPreference::Ask
        );
        folder.remote.id = "root".into();
        folder.local = dir.join("other");
        assert_eq!(
            Preferences::open(&path, "client", &folder)
                .unwrap()
                .load()
                .unwrap(),
            ConflictPreference::Ask
        );
        folder.local = dir.clone();
        let store = Preferences::open(&path, "client", &folder).unwrap();
        store
            .db
            .0
            .execute("UPDATE preferences SET rule='invalid'", [])
            .unwrap();
        assert_eq!(store.load(), Err(EngineError::StateStorage));
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
