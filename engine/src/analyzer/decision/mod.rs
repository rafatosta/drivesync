//! Pure, deterministic semantic decisions from an AnalysisContext.
mod models;
pub use models::*;

use crate::analyzer::{AnalysisContext, AnalysisError};

/// Pure decision boundary. Configuration is immutable for the lifetime of a
/// decision engine; identical context and policy produce identical output.
pub trait DecisionService: Send + Sync {
    /// Return the original context unchanged and exactly one decision per target.
    /// Matched -> its item; Unmatched/Ambiguous -> incoming observation;
    /// Scope -> each distinct item sorted by stable ID, or Synchronization if empty.
    /// Invalid File, association/scope mismatch, duplicate item IDs or inconsistent
    /// evidence return InvalidContext, not a synchronization outcome.
    ///
    /// Older matched observations are Obsolete. Equal order is not proof of
    /// duplicate delivery; revision-bound historical facts may be evaluated if
    /// the observed payload agrees with them. Incomparable targeted observations
    /// remain insufficient if Context could not establish freshness. Scope uses verified facts.
    /// A newer observation is evaluated against the retained baseline, without
    /// treating old measurements as measurements of the incoming payload.
    ///
    /// Evaluate presence before content, name and location. Unknown is never
    /// absence/equivalence. Event labels are observations, not commands.
    /// An unmatched one-sided observation may select Create with proved free destination;
    /// a missing row is not proof.
    /// A new one-sided subject may create on the other side
    /// only with verified absence at the intended destination and no baseline.
    /// Two present linked sides without a baseline require proven equivalence
    /// to establish it; known incompatibility is a conflict.
    ///
    /// One-sided changes select corresponding intents. Equal concurrent changes
    /// establish a baseline if no action is needed. Different concurrent changes
    /// or deletion versus surviving changes conflict. A confirmed deletion may
    /// propagate only if the surviving state is verified unchanged and the policy
    /// permits it; select Trash, never permanent Delete.
    /// A confirmed conflict blocks every intent for that item; otherwise missing
    /// relevant evidence blocks every intent for this delivery. Unsupported known
    /// kinds are Blocked rather than requesting the same evidence forever.
    /// Directories have no content-update intent. Known name/parent differences
    /// are independent of content; timestamps, sizes and remote versions alone
    /// do not establish cross-side byte equality or changed content.
    ///
    /// Keep findings in dimension order (Presence, Content, Name, Location);
    /// deduplicate requirements and intents, with intent order Presence/Create,
    /// Content/UpdateContent, Name/Rename, Location/Move.
    /// No I/O, clock, inventory mutation, conflict resolution or operator commands.
    /// Never return to Context; produce terminal InsufficientEvidence when facts are unavailable.
    fn decide(&self, context: AnalysisContext) -> Result<AnalysisDecision, AnalysisError>;
}

/// Fixed deletion policy, injected explicitly rather than read from persistence.
/// AuthorizePropagation means permission has already been granted externally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletionPolicy {
    RequireAuthorization,
    AuthorizePropagation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionPolicy {
    pub local_deletion: DeletionPolicy,
    pub remote_deletion: DeletionPolicy,
}

impl Default for DecisionPolicy {
    fn default() -> Self {
        Self {
            local_deletion: DeletionPolicy::RequireAuthorization,
            remote_deletion: DeletionPolicy::RequireAuthorization,
        }
    }
}

/// Concrete entry point used by behavior-first tests. Business rules are
/// intentionally unimplemented until the contract has been reviewed.
pub struct DecisionEngine {
    policy: DecisionPolicy,
}

impl DecisionEngine {
    pub fn new(policy: DecisionPolicy) -> Self {
        Self { policy }
    }
}

impl DecisionService for DecisionEngine {
    fn decide(&self, context: AnalysisContext) -> Result<AnalysisDecision, AnalysisError> {
        let _ = (self.policy, context);
        todo!("implement Decision after its behavioral contract is approved")
    }
}
