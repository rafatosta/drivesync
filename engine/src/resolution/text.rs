//! Reviewed text candidates bind immutable B/L/R bytes to exact scoped evidence.
use super::{RemoteVersion, ResolutionChoice, ResolutionRequest};
use crate::{ConfirmedBase, ConflictRecord, DownloadPlan, EngineError, TextMergeOutcome};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A verified preview. Reading or discarding it never accepts a resolution.
/// Only plain text is supported; positional alignment always requires review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMergeCandidate {
    pub(crate) request: ResolutionRequest,
}
impl TextMergeCandidate {
    /// Confirmed ancestor bytes, verified by the retained revision and SHA-256.
    pub fn base(&self) -> &[u8] {
        &self.evidence().base
    }
    /// Exact local bytes shown for review.
    pub fn local(&self) -> &[u8] {
        &self.evidence().local
    }
    /// Exact remote bytes shown for review.
    pub fn remote(&self) -> &[u8] {
        &self.evidence().remote
    }
    /// Proposed bytes; this is not a promise of semantic compatibility.
    pub fn result(&self) -> &[u8] {
        &self.evidence().result
    }
    /// Explicitly accepts this reviewed candidate for live revalidation.
    /// The executor still requires upload authorization and preserves all versions.
    pub fn accept_reviewed(&self) -> ResolutionRequest {
        self.request.clone()
    }
    fn evidence(&self) -> &MergeEvidence {
        self.request.merge.as_ref().expect("verified candidate")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MergeEvidence {
    pub client: String,
    pub scope: DownloadPlan,
    pub confirmed: ConfirmedBase,
    pub version: RemoteVersion,
    pub base: Vec<u8>,
    pub local: Vec<u8>,
    pub remote: Vec<u8>,
    pub result: Vec<u8>,
}
impl std::fmt::Debug for MergeEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MergeEvidence").finish_non_exhaustive()
    }
}
pub(crate) fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl MergeEvidence {
    /// Validates every binding on journal reopen; no arbitrary result is accepted.
    pub fn valid(&self, plan: &super::Plan) -> bool {
        plan.choice == ResolutionChoice::MergeText
            && plan.base.as_ref() == Some(&self.confirmed)
            && self.version == plan.remote
            && self.version.item.mime_type == "text/plain"
            && !self.client.is_empty()
            && self.scope.local_root.is_absolute()
            && self.scope.entries.is_empty()
            && crate::planner::fingerprint_parts(&self.confirmed.local_fingerprint)
                .is_some_and(|(h, _, _)| h == sha(&self.base))
            && crate::planner::fingerprint_parts(&plan.local_fingerprint)
                .is_some_and(|(h, _, _)| h == sha(&self.local))
            && self.version.item.size == Some(self.remote.len() as u64)
            && self.version.item.md5_checksum.as_deref() == Some(&md5(&self.remote))
            && crate::merge_plain_text("text/plain", Some(&self.base), &self.local, &self.remote)
                == TextMergeOutcome::Candidate(self.result.clone())
    }
}
pub(crate) fn md5(bytes: &[u8]) -> String {
    use md5::Md5;
    format!("{:x}", Md5::digest(bytes))
}

/// Produces a candidate only after comparison has authenticated L/R and the store B.
pub(crate) fn candidate(
    client: &str,
    scope: &DownloadPlan,
    record: &ConflictRecord,
    base: ConfirmedBase,
    version: RemoteVersion,
    bytes: Vec<u8>,
    comparison: crate::VersionComparison,
) -> Result<TextMergeCandidate, EngineError> {
    if record.diagnosis != crate::ChangeKind::ConcurrentChanges
        || comparison.mime_type != "text/plain"
    {
        return Err(EngineError::InvalidPlan);
    }
    let crate::ComparisonContent::Available { local, remote } = comparison.content else {
        return Err(EngineError::InventoryLimit);
    };
    let TextMergeOutcome::Candidate(result) =
        crate::merge_plain_text("text/plain", Some(&bytes), &local, &remote)
    else {
        return Err(EngineError::InvalidPlan);
    };
    let mut request = ResolutionRequest::from_record(record, ResolutionChoice::KeepLocal)?;
    request.choice = ResolutionChoice::MergeText;
    let mut scope = scope.clone();
    scope.entries.clear();
    request.merge = Some(MergeEvidence {
        client: client.into(),
        scope,
        confirmed: base,
        version,
        base: bytes,
        local,
        remote,
        result,
    });
    Ok(TextMergeCandidate { request })
}
