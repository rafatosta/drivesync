//! Deterministic in-memory source including empty, delayed, failing and paged folders.
use super::{model::*, source::*};
use std::time::Duration;
#[derive(Clone)]
pub struct MockFolderSource {
    folders: Vec<RemoteFolder>,
    pub delay: Duration,
    pub page_size: usize,
}
impl Default for MockFolderSource {
    fn default() -> Self {
        let mut folders = vec![RemoteFolder {
            id: RemoteFolderId("root".into()),
            name: "Meu Drive".into(),
            parent_id: None,
        }];
        for (parent, children) in [
            ("root", vec!["Documentos", "Projetos", "Fotos", "Backup"]),
            ("Documentos", vec!["Trabalho", "Pessoal"]),
            ("Projetos", vec!["DriveSync", "Zapzap", "GTasks"]),
            (
                "DriveSync",
                vec![
                    "Código-fonte",
                    "Documentação",
                    "Recursos",
                    "Releases",
                    "Testes",
                    "Ferramentas",
                    "scripts",
                ],
            ),
            (
                "Testes",
                vec!["Pasta vazia", "Erro de consulta", "Muitas pastas"],
            ),
        ] {
            for name in children {
                folders.push(RemoteFolder {
                    id: RemoteFolderId(name.into()),
                    name: name.into(),
                    parent_id: Some(RemoteFolderId(parent.into())),
                });
            }
        }
        for i in 1..=80 {
            folders.push(RemoteFolder {
                id: RemoteFolderId(format!("mock-page-{i}")),
                name: format!("Pasta {i:02}"),
                parent_id: Some(RemoteFolderId("Muitas pastas".into())),
            });
        }
        Self {
            folders,
            delay: Duration::from_millis(180),
            page_size: 20,
        }
    }
}
impl MockFolderSource {
    pub fn root(&self) -> RemoteFolder {
        self.folders[0].clone()
    }
}
impl RemoteFolderSource for MockFolderSource {
    fn list_folders(
        &self,
        parent: &RemoteFolderId,
        token: Option<&str>,
    ) -> FolderFuture<'_, Result<FolderPage, FolderSourceError>> {
        let parent = parent.clone();
        let offset = token.map(str::parse::<usize>).transpose();
        Box::pin(async move {
            gtk::glib::timeout_future(self.delay).await;
            if parent.0 == "Erro de consulta" {
                return Err(FolderSourceError::Unavailable);
            }
            if !self.folders.iter().any(|f| f.id == parent) {
                return Err(FolderSourceError::NotFound);
            }
            let offset = offset
                .map_err(|_| FolderSourceError::Other("Invalid page token".into()))?
                .unwrap_or(0);
            let children: Vec<_> = self
                .folders
                .iter()
                .filter(|f| f.parent_id.as_ref() == Some(&parent))
                .cloned()
                .collect();
            if offset > children.len() {
                return Err(FolderSourceError::NotFound);
            }
            let end = (offset + self.page_size.max(1)).min(children.len());
            Ok(FolderPage {
                folders: children[offset..end].to_vec(),
                next_page_token: (end < children.len()).then(|| end.to_string()),
            })
        })
    }
}
