//! Observable facts required by analysis, independent of current producer support.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DigestAlgorithm { Sha256, Md5 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentDigest { pub algorithm: DigestAlgorithm, pub hex: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservedPresence { Present, AbsentConfirmed, Trashed, OutsideScope, Inaccessible }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservedParent {
    LocalRoot,
    Local { path: LocalRelativePath, identity: MetadataValue<LocalFileId> },
    Remote(RemoteFileRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCapabilities {
    pub read_content: MetadataValue<bool>,
    pub create_child: MetadataValue<bool>,
    pub replace_content: MetadataValue<bool>,
    pub rename: MetadataValue<bool>,
    pub move_item: MetadataValue<bool>,
    pub trash: MetadataValue<bool>,
    pub restore: MetadataValue<bool>,
}

/// Facts about the observed resource only. Missing is not false or absent.
/// Digests must be measured against the same stable payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEvidence {
    pub presence: MetadataValue<ObservedPresence>,
    pub digest: MetadataValue<ContentDigest>,
    pub parent: MetadataValue<ObservedParent>,
    pub revision: MetadataValue<String>,
    pub capabilities: MetadataValue<ResourceCapabilities>,
    pub previous_local_identity: MetadataValue<LocalFileId>,
    /// Completeness of a directory enumeration, not metadata coverage.
    pub namespace_complete: MetadataValue<bool>,
}
impl Default for FileEvidence {
    fn default() -> Self {
        Self { presence: MetadataValue::NotCollected, digest: MetadataValue::NotCollected,
            parent: MetadataValue::NotCollected, revision: MetadataValue::NotCollected,
            capabilities: MetadataValue::NotCollected, previous_local_identity: MetadataValue::NotCollected,
            namespace_complete: MetadataValue::NotCollected }
    }
}
impl FileEvidence {
    pub fn validate(&self, file: &File) -> ModelResult<()> {
        use MetadataValue::Known;
        let invalid = || Err(ModelError::InconsistentEvent);
        if let Known(digest) = &self.digest {
            let n = match digest.algorithm { DigestAlgorithm::Sha256 => 64, DigestAlgorithm::Md5 => 32 };
            if digest.hex.len() != n || !digest.hex.bytes().all(|b| b.is_ascii_hexdigit()) || file.data.is_none() { return invalid(); }
        }
        if let Known(p) = self.presence {
            if p == ObservedPresence::Present && file.data.is_none() || p == ObservedPresence::AbsentConfirmed && file.data.is_some() { return invalid(); }
            if matches!(file.event.local_presence, Some(LocalPresenceEvidence::Present)) && p != ObservedPresence::Present ||
                matches!(file.event.local_presence, Some(LocalPresenceEvidence::AbsentConfirmed)) && p != ObservedPresence::AbsentConfirmed { return invalid(); }
            if matches!(file.scope, FileScope::Local(_)) && p == ObservedPresence::Trashed { return invalid(); }
        }
        if let Known(parent) = &self.parent {
            match (&file.scope,parent) {
                (FileScope::Local(_), ObservedParent::LocalRoot | ObservedParent::Local { .. }) => {},
                (FileScope::Remote(r), ObservedParent::Remote(p)) if r.account_id == p.account_id && !p.file_id.0.trim().is_empty() => {},
                _ => return invalid(),
            }
        }
        if let Known(id) = &self.previous_local_identity {
            if id.0.trim().is_empty() || !matches!(file.scope,FileScope::Local(_)) || file.event.previous.is_none() { return invalid(); }
        }
        if let Known(rev) = &self.revision { if rev.trim().is_empty() { return invalid(); } }
        Ok(())
    }
}
