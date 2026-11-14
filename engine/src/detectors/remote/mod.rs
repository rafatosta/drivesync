//! Periodic remote observation; scheduling is independent of external event delivery.
mod models;
pub use models::*;
mod google;
mod interfaces;
pub(crate) use interfaces::{Cancellation, RemoteSessionContext};
use interfaces::{ObserverFactory, RemoteBackend, RemoteObserver};
mod runtime;
use runtime::now;
pub use runtime::RemoteDetector;

impl RemoteDetector {
    /// Resolves the persisted account provider before opening any producer storage.
    /// Unsupported providers fail explicitly; no implementation is inferred from paths.
    #[cfg(test)]
    pub fn new(request: RemoteDetectorCreateRequest) -> RemoteResult<Self> {
        runtime::validate(&request.scope, &request.settings)?;
        let session = request
            .connector
            .remote_session_context(&request.scope.account_id)?;
        let factory = implementation_for(&session.provider)?;
        let provider = session.provider.clone();
        let scope = request.scope.clone();
        Self::open(
            request.scope,
            request.settings,
            request.data_directory,
            provider,
            move |backend| factory(request.connector, session, scope, backend),
        )
    }
}

impl RemoteDetector {
    pub(crate) fn with_store(
        request: RemoteDetectorCreateRequest,
        store: std::sync::Arc<super::storage::Store>,
    ) -> RemoteResult<Self> {
        runtime::validate(&request.scope, &request.settings)?;
        let session = request
            .connector
            .remote_session_context(&request.scope.account_id)?;
        let factory = implementation_for(&session.provider)?;
        Self::from_store(request.settings, store, move |backend| {
            factory(request.connector, session, request.scope, backend)
        })
    }
}

/// The only common-module registry that knows the concrete provider implementations.
fn implementation_for(provider: &crate::ProviderId) -> RemoteResult<ObserverFactory> {
    match provider.0.as_str() {
        "google" => Ok(google::create),
        _ => Err(RemoteFailure::new(RemoteError::UnsupportedProvider)),
    }
}

#[cfg(test)]
mod dispatch_tests;

pub(crate) fn validate_configuration(
    request: &RemoteDetectorCreateRequest,
) -> RemoteResult<crate::ProviderId> {
    runtime::validate(&request.scope, &request.settings)?;
    let context = request
        .connector
        .remote_session_context(&request.scope.account_id)?;
    implementation_for(&context.provider)?;
    Ok(context.provider)
}
