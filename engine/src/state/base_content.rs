//! Bounded common-base bytes, committed with verified installation evidence.
//! Historical revisions are immutable and scoped by client, account and root pair.
use super::{failed, Store};
use crate::{
    download::{
        filesystem::{self, Root},
        Operation, Phase,
    },
    Cancellation, ConfirmedBase, DownloadPlan, EngineError,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::io::Read;

impl Store {
    /// Captures only bounded plain text from the exact verified installed inode.
    /// SQLite FULL transactions atomically publish bytes and their confirmed operation.
    /// Missing/changed files fail closed; unsupported or large files retain metadata only.
    pub(crate) fn put_verified_operation(
        &self,
        client: &str,
        scope: &DownloadPlan,
        op: &Operation,
        root: &Root,
        cancel: &Cancellation,
    ) -> Result<(), EngineError> {
        let evidence = match &op.phase {
            Phase::Installed { identity, sha256 }
            | Phase::Updated {
                identity, sha256, ..
            } if op.item.mime_type == "text/plain"
                && op.item.size.is_some_and(|n| n <= crate::comparison::LIMIT) =>
            {
                Some((identity, sha256))
            }
            _ => None,
        };
        let Some((identity, sha)) = evidence else {
            return self.put_operation(client, scope, op);
        };
        let (parent, name) = root.parent(&op.relative)?;
        let parent_id = filesystem::identity(&parent)?;
        let mut file =
            filesystem::read_transfer(&parent, &name)?.ok_or(EngineError::DownloadConflict)?;
        let stamp = crate::local::Stamp::read(&file)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(crate::comparison::LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(filesystem::io)?;
        crate::local::check(cancel)?;
        root.validate_parent(&op.relative, parent_id)?;
        if bytes.len() as u64 > crate::comparison::LIMIT
            || Some(bytes.len() as u64) != op.item.size
            || filesystem::identity(&file)? != *identity
            || crate::local::Stamp::read(&file)? != stamp
            || filesystem::read_transfer(&parent, &name)?
                .as_ref()
                .map(crate::local::Stamp::read)
                .transpose()?
                != Some(stamp)
            || format!("{:x}", Sha256::digest(&bytes)) != *sha
        {
            return Err(EngineError::DownloadConflict);
        }
        let tx =
            Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate).map_err(failed)?;
        self.retain_base(
            client,
            scope,
            &op.item.id,
            op.item.version.ok_or(EngineError::InvalidPlan)?,
            sha,
            &bytes,
        )?;
        self.put_operation(client, scope, op)?;
        tx.commit().map_err(failed)
    }

    /// Retains an exact verified revision, never overwriting historical evidence.
    /// A 64 MiB budget per scope blocks growth without evicting pending evidence.
    pub(crate) fn retain_base(
        &self,
        client: &str,
        scope: &DownloadPlan,
        id: &str,
        version: u64,
        sha: &str,
        bytes: &[u8],
    ) -> Result<(), EngineError> {
        if bytes.len() as u64 > crate::comparison::LIMIT
            || version == 0
            || format!("{:x}", Sha256::digest(bytes)) != sha
        {
            return Err(EngineError::DownloadIntegrity);
        }
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        let old = self.base_bytes(client, scope, id, version, sha)?;
        if let Some(old) = old {
            return if old == bytes {
                Ok(())
            } else {
                Err(EngineError::DownloadIntegrity)
            };
        }
        let used: u64 = self.0.query_row("SELECT COALESCE(SUM(length(bytes)),0) FROM base_content WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4", params![client,scope.account_id.0,scope.remote_root,local], |r| r.get(0)).map_err(failed)?;
        if used.saturating_add(bytes.len() as u64) > 64 * 1024 * 1024 {
            return Err(EngineError::InventoryLimit);
        }
        self.0
            .execute(
                "INSERT INTO base_content VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    client,
                    scope.account_id.0,
                    scope.remote_root,
                    local,
                    id,
                    version,
                    sha,
                    bytes
                ],
            )
            .map_err(failed)?;
        Ok(())
    }

    /// Requires the caller's confirmed base; a current revision is never a substitute.
    pub(crate) fn common_base(
        &self,
        client: &str,
        scope: &DownloadPlan,
        id: &str,
        base: &ConfirmedBase,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        let (sha, _, _) = crate::planner::fingerprint_parts(&base.local_fingerprint)
            .ok_or(EngineError::StateStorage)?;
        self.base_bytes(client, scope, id, base.remote_version, sha)
    }
    fn base_bytes(
        &self,
        client: &str,
        scope: &DownloadPlan,
        id: &str,
        version: u64,
        sha: &str,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        let bytes: Option<Vec<u8>> = self.0.query_row("SELECT bytes FROM base_content WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND file=?5 AND version=?6 AND sha=?7 AND length(bytes)<=1048576", params![client,scope.account_id.0,scope.remote_root,scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?,id,version,sha], |r| r.get(0)).optional().map_err(failed)?;
        // An oversized row is corruption, not an absent base.
        let exists: bool = self.0.query_row("SELECT EXISTS(SELECT 1 FROM base_content WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND file=?5 AND version=?6 AND sha=?7)", params![client,scope.account_id.0,scope.remote_root,scope.local_root.to_str(),id,version,sha], |r| r.get(0)).map_err(failed)?;
        if exists
            && bytes
                .as_ref()
                .is_none_or(|b| format!("{:x}", Sha256::digest(b)) != sha)
        {
            return Err(EngineError::DownloadIntegrity);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisions_reopen_corruption_and_scope_isolation() {
        let dir = std::env::temp_dir().join(format!(
            "drivesync-base-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let path = dir.join("state.sqlite3");
        let scope = DownloadPlan {
            account_id: crate::AccountId("account".into()),
            remote_root: "root".into(),
            local_root: dir.join("files"),
            entries: vec![],
        };
        let sha = format!("{:x}", Sha256::digest(b"base"));
        let base = ConfirmedBase {
            remote_version: 1,
            local_fingerprint: format!("{sha}:1:2"),
            location: None,
        };
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.common_base("client", &scope, "file", &base).unwrap(),
            None
        );
        store
            .retain_base("client", &scope, "file", 1, &sha, b"base")
            .unwrap();
        store
            .retain_base("client", &scope, "file", 1, &sha, b"base")
            .unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.common_base("client", &scope, "file", &base).unwrap(),
            Some(b"base".to_vec())
        );
        for other in [
            DownloadPlan {
                account_id: crate::AccountId("other".into()),
                ..scope.clone()
            },
            DownloadPlan {
                remote_root: "other".into(),
                ..scope.clone()
            },
            DownloadPlan {
                local_root: dir.join("other"),
                ..scope.clone()
            },
        ] {
            assert_eq!(
                store.common_base("client", &other, "file", &base).unwrap(),
                None
            );
        }
        assert_eq!(
            store.common_base("other", &scope, "file", &base).unwrap(),
            None
        );
        assert_eq!(
            store.common_base("client", &scope, "other", &base).unwrap(),
            None
        );
        assert_eq!(
            store
                .common_base(
                    "client",
                    &scope,
                    "file",
                    &ConfirmedBase {
                        remote_version: 2,
                        ..base.clone()
                    }
                )
                .unwrap(),
            None
        );
        store
            .0
            .execute(
                "UPDATE base_content SET bytes=?1",
                params![b"broken".as_slice()],
            )
            .unwrap();
        assert_eq!(
            store.common_base("client", &scope, "file", &base),
            Err(EngineError::DownloadIntegrity)
        );
        assert!(store
            .retain_base("client", &scope, "file", 1, &sha, b"base")
            .is_err());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    #[test]
    fn verified_installation_and_base_commit_together_or_roll_back() {
        let dir = std::env::temp_dir().join(format!(
            "drivesync-base-atomic-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        let local = dir.join("files");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("file.txt"), b"base").unwrap();
        let root = Root::open(&local).unwrap();
        let cancel = Cancellation::new();
        let scope = DownloadPlan {
            account_id: crate::AccountId("account".into()),
            remote_root: "root".into(),
            local_root: local.clone(),
            entries: vec![],
        };
        let mut store = Store::open(&dir.join("state.sqlite3")).unwrap();
        store
            .save(
                "client",
                &crate::RemoteInventory {
                    account_id: scope.account_id.clone(),
                    root_id: "root".into(),
                    entries: vec![],
                },
                &scope,
                &cancel,
            )
            .unwrap();
        let sha = format!("{:x}", Sha256::digest(b"base"));
        let id =
            filesystem::identity(&std::fs::File::open(local.join("file.txt")).unwrap()).unwrap();
        let op = Operation {
            id: "operation".into(),
            item: crate::InventoryEntry {
                id: "file".into(),
                parent_id: "root".into(),
                name: "file.txt".into(),
                mime_type: "text/plain".into(),
                kind: crate::InventoryKind::Binary,
                version: Some(1),
                size: Some(4),
                md5_checksum: Some(crate::resolution::text::md5(b"base")),
                web_view_link: None,
            },
            relative: "file.txt".into(),
            temporary: ".drivesync-operation.part".into(),
            root: filesystem::identity(&root.file).unwrap(),
            parent: filesystem::identity(&root.file).unwrap(),
            phase: Phase::Installed {
                identity: id,
                sha256: sha.clone(),
            },
        };
        let base = ConfirmedBase {
            remote_version: 1,
            local_fingerprint: id.fingerprint(&sha),
            location: None,
        };
        store.0.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON downloads BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert_eq!(
            store.put_verified_operation("client", &scope, &op, &root, &cancel),
            Err(EngineError::StateStorage)
        );
        assert!(store.operations("client", &scope).unwrap().is_empty());
        assert_eq!(
            store.common_base("client", &scope, "file", &base).unwrap(),
            None
        );
        store.0.execute_batch("DROP TRIGGER fail").unwrap();
        store
            .put_verified_operation("client", &scope, &op, &root, &cancel)
            .unwrap();
        assert_eq!(
            store.common_base("client", &scope, "file", &base).unwrap(),
            Some(b"base".to_vec())
        );
        std::fs::write(local.join("file.txt"), b"race").unwrap();
        assert_eq!(
            store.put_verified_operation("client", &scope, &op, &root, &cancel),
            Err(EngineError::DownloadConflict)
        );
        assert_eq!(
            store.common_base("client", &scope, "file", &base).unwrap(),
            Some(b"base".to_vec())
        );
        drop(store);
        drop(root);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
