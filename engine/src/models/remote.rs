//! Selected remote observation metadata; no provider types, credentials or I/O.
use super::*;

/// Opaque, filtered provider metadata; its format is interpreted only by the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderFileData {
    pub provider: ProviderId,
    pub format: String,
    pub format_version: u32,
    pub payload: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteEntryKind {
    File,
    Directory,
    Shortcut,
}
/// Minimal reviewed profile. Unreceived fields never become empty/false/zero values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteFile {
    pub reference: RemoteFileRef,
    pub provider: ProviderId,
    pub name: MetadataValue<String>,
    pub kind: MetadataValue<RemoteEntryKind>,
    pub mime_type: MetadataValue<String>,
    pub parent_ids: MetadataValue<Vec<RemoteFileId>>,
    pub trashed: MetadataValue<bool>,
    pub version: MetadataValue<String>,
    pub created_time: MetadataValue<String>,
    pub modified_time: MetadataValue<String>,
    pub size: MetadataValue<u64>,
    pub md5_checksum: MetadataValue<String>,
    pub sha256_checksum: MetadataValue<String>,
    pub provider_data: MetadataValue<ProviderFileData>,
}
impl RemoteFile {
    /// Checks identity, payload bounds and provider association without interpreting JSON.
    pub fn validate(&self) -> ModelResult<()> {
        if self.reference.account_id.0.trim().is_empty()
            || self.reference.file_id.0.trim().is_empty()
            || self.provider.0.trim().is_empty()
        {
            return Err(ModelError::EmptyIdentity);
        }
        if let MetadataValue::Known(parents) = &self.parent_ids {
            if parents.iter().any(|id| id.0.trim().is_empty()) {
                return Err(ModelError::EmptyIdentity);
            }
        }
        if let MetadataValue::Known(data) = &self.provider_data {
            if data.provider != self.provider
                || data.format.trim().is_empty()
                || data.format_version == 0
                || data.payload.len() > 65536
            {
                return Err(ModelError::InconsistentMetadata);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn remote() -> RemoteFile {
        RemoteFile {
            reference: RemoteFileRef {
                account_id: AccountId("account".into()),
                file_id: RemoteFileId("file".into()),
            },
            provider: ProviderId("google".into()),
            name: MetadataValue::Known("Document".into()),
            kind: MetadataValue::Known(RemoteEntryKind::File),
            mime_type: MetadataValue::NotCollected,
            parent_ids: MetadataValue::Known(vec![RemoteFileId("parent".into())]),
            trashed: MetadataValue::NotCollected,
            version: MetadataValue::NotCollected,
            created_time: MetadataValue::NotCollected,
            modified_time: MetadataValue::NotCollected,
            size: MetadataValue::NotCollected,
            md5_checksum: MetadataValue::NotCollected,
            sha256_checksum: MetadataValue::NotCollected,
            provider_data: MetadataValue::NotCollected,
        }
    }
    #[test]
    fn absent_metadata_is_valid_but_identity_and_provider_payload_are_checked() {
        let mut file = remote();
        assert_eq!(file.validate(), Ok(()));
        file.reference.account_id.0.clear();
        assert_eq!(file.validate(), Err(ModelError::EmptyIdentity));
        file = remote();
        file.provider_data = MetadataValue::Known(ProviderFileData {
            provider: ProviderId("another".into()),
            format: "filtered".into(),
            format_version: 1,
            payload: vec![],
        });
        assert_eq!(file.validate(), Err(ModelError::InconsistentMetadata));
        let MetadataValue::Known(data) = &mut file.provider_data else {
            panic!()
        };
        data.provider = file.provider.clone();
        data.payload = vec![0; 65537];
        assert_eq!(file.validate(), Err(ModelError::InconsistentMetadata));
    }
    #[test]
    fn envelope_checks_account_resource_location_and_payload_together() {
        let remote = remote();
        let mut file = File {
            id: FileId("occurrence".into()),
            sync_id: SyncId("sync".into()),
            event: FileEvent {
                remote: None,
                kind: FileEventKind::Created,
                uncertainty: None,
                previous: None,
                current: Some(FileLocation::Remote {
                    reference: remote.reference.clone(),
                    parent_id: Some(RemoteFileId("parent".into())),
                    name: Some("Document".into()),
                }),
            },
            observed_at: Timestamp { unix_millis: 1 },
            coverage: ObservationCoverage::Partial,
            scope: FileScope::Remote(remote.reference.clone()),
            data: Some(FileData::Remote(remote)),
        };
        assert_eq!(file.validate(), Ok(()));
        let encoded = serde_json::to_vec(&file).unwrap();
        assert_eq!(serde_json::from_slice::<File>(&encoded).unwrap(), file);
        if let Some(FileLocation::Remote { name, .. }) = &mut file.event.current {
            *name = Some("Wrong".into());
        }
        assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    }
}
