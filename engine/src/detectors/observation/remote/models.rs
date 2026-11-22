//! Requests, results, scopes, and errors for provider-specific remote observers.

use crate::{
    detectors::observation::models::{ObservationCoverage, RemoteObservation},
    models::{AccountId, ProviderId, RemoteId, SyncRootId},
};

/// Fixed account, provider, and root scope for remote observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteScope {
    /// Account whose authorized session will be used.
    pub account_id: AccountId,
    /// Provider selected from the account configuration.
    pub provider_id: ProviderId,
    /// Local synchronization root associated with the remote root.
    pub sync_root_id: SyncRootId,
    /// Provider identity of the selected remote root.
    pub remote_root_id: RemoteId,
}

/// Provider-bound configuration used to construct one remote observer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteObserverConfig {
    /// Account whose authorization and remote inventory are in scope.
    pub account_id: AccountId,
    /// Provider selected from the account configuration.
    pub provider_id: ProviderId,
    /// Local synchronization root associated with the remote root.
    pub sync_root_id: SyncRootId,
    /// Provider identity of the selected remote root.
    pub remote_root_id: RemoteId,
}

/// Remote scope and observation behavior requested for one observer round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteObserveRequest {
    /// Account, provider, and root isolation boundary.
    pub scope: RemoteScope,
    /// Whether to capture an initial, incremental, or recovery view.
    pub mode: RemoteObserveMode,
    /// Optional upper bound on provider pages to read in this round.
    pub page_budget: Option<u64>,
    /// Persisted identities to recheck when missing, including after restart.
    pub known_remote_ids: Vec<RemoteId>,
}

/// Remote observation round strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteObserveMode {
    /// Capture an initial view of the configured root.
    Initial,
    /// Read changes since a provider-specific continuation point.
    Incremental,
    /// Rebuild a view after an interrupted or invalid capture.
    Recovery,
}

/// Opaque continuation value interpretable only by its originating provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderCursor {
    /// Provider that created and understands this cursor.
    pub provider_id: ProviderId,
    /// Opaque provider-defined value.
    pub value: String,
}

/// Observations and coverage returned by one remote observer round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteObserveResult {
    /// Normalized remote item evidence collected during this round.
    pub observations: Vec<RemoteObservation>,
    /// Whether the declared scope was examined completely enough to reason about absence.
    pub coverage: ObservationCoverage,
    /// Optional provider-owned continuation for a later round.
    pub continuation: Option<ProviderCursor>,
    /// Whether the provider reports additional pages beyond this round.
    pub has_more: bool,
}

/// Sanitized failure categories for remote observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteObserverError {
    /// No observer was registered for the requested provider.
    UnsupportedProvider,
    /// The account requires a valid authorized session.
    AuthorizationRequired,
    /// The provider denied access to the configured scope or metadata.
    PermissionDenied,
    /// The provider or network is temporarily unavailable.
    Unavailable,
    /// The requested scope could not be examined with sufficient coverage.
    IncompleteCoverage,
    /// The supplied provider continuation cannot be used.
    InvalidCursor,
    /// The caller cancelled this observation round.
    Cancelled,
}

impl std::fmt::Display for RemoteObserverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Remote observation failed: {self:?}")
    }
}

impl std::error::Error for RemoteObserverError {}
