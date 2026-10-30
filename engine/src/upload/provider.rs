//! Authenticated create-only adapter; every request remains account-scoped.
use super::{execution::Remote, transport, Intent};
use crate::{
    auth::OAuthConfig, Cancellation, DriveSyncEngine, EngineError, FolderConfiguration,
    InventoryEntry, InventoryKind,
};
use std::collections::HashMap;
pub(crate) struct Provider<'a> {
    pub engine: &'a DriveSyncEngine,
    pub folder: &'a FolderConfiguration,
    pub config: &'a OAuthConfig,
    pub cancel: &'a Cancellation,
    pub parents: HashMap<String, String>,
    pub uploads: &'a crate::state::Uploads,
}
impl Provider<'_> {
    fn access<T>(&self, f: impl FnOnce(&str) -> Result<T, EngineError>) -> Result<T, EngineError> {
        super::local::check(self.cancel)?;
        self.engine
            .auth_upload_access(self.config, &self.folder.account_id, f)
    }
}
impl Remote for Provider<'_> {
    fn configured(&mut self) -> Result<(), EngineError> {
        super::local::check(self.cancel)?;
        self.engine.verify_root(self.folder, self.config)?;
        if self.engine.saved_folder(&self.folder.account_id)?.as_ref() != Some(self.folder)
            || !self.uploads.enabled()?
        {
            return Err(EngineError::InvalidPlan);
        }
        Ok(())
    }
    fn generate(&mut self) -> Result<String, EngineError> {
        self.access(transport::generate)
    }
    fn children(&mut self, parent: &str, name: &str) -> Result<Vec<InventoryEntry>, EngineError> {
        self.access(|t| transport::children(t, parent, name))
    }
    fn stat(&mut self, op: &Intent) -> Result<Option<InventoryEntry>, EngineError> {
        self.access(|t| transport::stat(t, op))
    }
    fn folder(&mut self, op: &Intent) -> Result<(), EngineError> {
        self.configured()?;
        self.access(|t| transport::create(t, op))
    }
    fn begin(&mut self, op: &Intent) -> Result<String, EngineError> {
        self.configured()?;
        self.access(|t| transport::session(t, op))
    }
    fn send(
        &mut self,
        op: &Intent,
        offset: Option<u64>,
        bytes: Vec<u8>,
    ) -> Result<transport::Position, EngineError> {
        self.configured()?;
        self.access(|t| {
            transport::send(
                t,
                op.session.as_deref().ok_or(EngineError::InvalidPlan)?,
                op.size,
                offset,
                bytes,
            )
        })
    }
    fn remember(&mut self, item: &InventoryEntry) {
        if item.kind == InventoryKind::Folder {
            self.parents.insert(item.id.clone(), item.parent_id.clone());
        }
    }
    fn validate_parent(&mut self, parent: &str) -> Result<(), EngineError> {
        self.configured()?;
        self.access(|t| crate::remote::writable_root(t, parent))
            .map_err(|error| {
                if parent == self.folder.remote.id {
                    crate::coordinator::remote(error)
                } else {
                    error
                }
            })?;
        let mut current = parent.to_string();
        let mut seen = std::collections::HashSet::new();
        while current != self.folder.remote.id {
            if !seen.insert(current.clone()) || seen.len() > 10000 {
                return Err(EngineError::InvalidPlan);
            }
            let expected = self
                .parents
                .get(&current)
                .ok_or(EngineError::RemoteChanged)?
                .clone();
            let op = Intent {
                path: "parent".into(),
                root: crate::download::Identity {
                    device: 0,
                    inode: 0,
                },
                source: None,
                id: current.clone(),
                parent: expected.clone(),
                directory: true,
                size: 0,
                md5: String::new(),
                sha: String::new(),
                snapshot: String::new(),
                session: None,
                confirmed: false,
                retry: super::Retry::default(),
            };
            let item = self.stat(&op)?.ok_or(EngineError::RemoteChanged)?;
            if item.kind != InventoryKind::Folder {
                return Err(EngineError::RemoteChanged);
            }
            current = expected;
        }
        self.access(|t| crate::remote::inventory_root(t, &self.folder.remote.id))
            .map_err(crate::coordinator::remote)
            .map(|_| ())
    }
}
