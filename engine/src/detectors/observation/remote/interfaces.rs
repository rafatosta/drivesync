//! Provider observer interfaces, cancellation, and explicit factory dispatch.

use super::models::{
    RemoteObserveRequest, RemoteObserveResult, RemoteObserverConfig, RemoteObserverError,
};
use crate::models::{AccountId, ProviderId};
use std::{
    any::Any,
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Authorized provider session identity passed to a remote observer.
///
/// The provider-specific session remains opaque to the shared observation contract. A provider
/// adapter can recover its own concrete session through as_any; callers must ensure its account
/// and provider match the requested scope before invoking an observer.
pub(crate) trait AuthorizedSession: Any + Send + Sync {
    /// Account identity bound to this authorization.
    fn account_id(&self) -> &AccountId;

    /// Provider identity bound to this authorization.
    fn provider_id(&self) -> &ProviderId;

    /// Opaque provider session for the registered specialist.
    fn as_any(&self) -> &dyn Any;
}

/// Cloneable one-way cancellation signal for an observation round.
#[derive(Debug, Clone, Default)]
pub(crate) struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Creates a token in the active state.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Signals cancellation to all clones of this token.
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Returns whether cancellation has been signaled.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Future returned by one provider observer.
pub(crate) type RemoteObserverFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RemoteObserveResult, RemoteObserverError>> + Send + 'a>>;

/// Reads provider resources and converts them to normalized observations.
///
/// Implementations must not query Inventory to decide synchronization actions or mutate local or
/// remote files. Provider pagination and cursor meaning remain private to the specialist.
pub(crate) trait RemoteObserver: Send + Sync {
    /// Observes one bounded round using an already authorized session.
    ///
    /// Implementations must monitor cancellation during the round and return Cancelled promptly
    /// after the token is signaled. The dispatch wrapper rejects account, provider, or root
    /// mismatches before entering the provider implementation.
    fn observe<'a>(
        &'a self,
        request: RemoteObserveRequest,
        session: &'a dyn AuthorizedSession,
        cancellation: &'a CancellationToken,
    ) -> RemoteObserverFuture<'a>;
}

/// Constructs an observer for one account-derived provider configuration.
pub(crate) trait RemoteObserverProvider: Send + Sync {
    /// Creates the provider specialist for the supplied fixed scope.
    fn create(
        &self,
        config: RemoteObserverConfig,
    ) -> Result<Box<dyn RemoteObserver>, RemoteObserverError>;
}

/// Contract for resolving a remote observer from configured provider identity.
pub(crate) trait RemoteObserverFactory: Send + Sync {
    /// Resolves the registered specialist or reports an unsupported provider.
    fn create(
        &self,
        config: RemoteObserverConfig,
    ) -> Result<Box<dyn RemoteObserver>, RemoteObserverError>;
}

/// Explicit provider registry with no implicit default observer.
#[derive(Default)]
pub(crate) struct ProviderRemoteObserverFactory {
    providers: BTreeMap<ProviderId, Arc<dyn RemoteObserverProvider>>,
}

struct ScopedRemoteObserver {
    config: RemoteObserverConfig,
    observer: Box<dyn RemoteObserver>,
}

impl ScopedRemoteObserver {
    fn accepts(&self, request: &RemoteObserveRequest, session: &dyn AuthorizedSession) -> bool {
        request.scope.account_id == self.config.account_id
            && request.scope.provider_id == self.config.provider_id
            && request.scope.sync_root_id == self.config.sync_root_id
            && request.scope.remote_root_id == self.config.remote_root_id
            && session.account_id() == &self.config.account_id
            && session.provider_id() == &self.config.provider_id
    }
}

impl RemoteObserver for ScopedRemoteObserver {
    fn observe<'a>(
        &'a self,
        request: RemoteObserveRequest,
        session: &'a dyn AuthorizedSession,
        cancellation: &'a CancellationToken,
    ) -> RemoteObserverFuture<'a> {
        if cancellation.is_cancelled() {
            return Box::pin(async { Err(RemoteObserverError::Cancelled) });
        }
        if !self.accepts(&request, session) {
            return Box::pin(async { Err(RemoteObserverError::AuthorizationRequired) });
        }
        self.observer.observe(request, session, cancellation)
    }
}

impl ProviderRemoteObserverFactory {
    /// Creates an empty registry; providers must be registered explicitly.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Registers a constructor for one provider identity.
    ///
    /// Returns false and leaves the existing constructor intact when the identity is already
    /// registered. This prevents accidental provider replacement during setup.
    pub(crate) fn register(
        &mut self,
        provider_id: ProviderId,
        provider: Arc<dyn RemoteObserverProvider>,
    ) -> bool {
        if self.providers.contains_key(&provider_id) {
            return false;
        }
        self.providers.insert(provider_id, provider);
        true
    }
}

impl RemoteObserverFactory for ProviderRemoteObserverFactory {
    fn create(
        &self,
        config: RemoteObserverConfig,
    ) -> Result<Box<dyn RemoteObserver>, RemoteObserverError> {
        let provider = self
            .providers
            .get(&config.provider_id)
            .ok_or(RemoteObserverError::UnsupportedProvider)?;
        let observer = provider.create(config.clone())?;
        Ok(Box::new(ScopedRemoteObserver { config, observer }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        detectors::observation::models::ObservationCoverage,
        models::{RemoteId, SyncRootId},
    };
    use std::sync::Mutex;

    struct StubObserver;

    impl RemoteObserver for StubObserver {
        fn observe<'a>(
            &'a self,
            _: RemoteObserveRequest,
            _: &'a dyn AuthorizedSession,
            _: &'a CancellationToken,
        ) -> RemoteObserverFuture<'a> {
            Box::pin(async {
                Ok(RemoteObserveResult {
                    observations: Vec::new(),
                    coverage: ObservationCoverage::Complete,
                    continuation: None,
                    has_more: false,
                })
            })
        }
    }

    struct StubSession {
        account_id: AccountId,
        provider_id: ProviderId,
    }

    impl AuthorizedSession for StubSession {
        fn account_id(&self) -> &AccountId {
            &self.account_id
        }

        fn provider_id(&self) -> &ProviderId {
            &self.provider_id
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Default)]
    struct StubProvider {
        created_for: Mutex<Vec<RemoteObserverConfig>>,
    }

    impl RemoteObserverProvider for StubProvider {
        fn create(
            &self,
            config: RemoteObserverConfig,
        ) -> Result<Box<dyn RemoteObserver>, RemoteObserverError> {
            self.created_for.lock().unwrap().push(config);
            Ok(Box::new(StubObserver))
        }
    }

    #[test]
    fn factory_dispatches_only_to_the_registered_provider() {
        let provider_id = ProviderId("example".into());
        let provider = Arc::new(StubProvider::default());
        let mut factory = ProviderRemoteObserverFactory::new();
        assert!(factory.register(provider_id.clone(), provider.clone()));
        assert!(!factory.register(provider_id.clone(), Arc::new(StubProvider::default())));

        let config = RemoteObserverConfig {
            account_id: AccountId("account-1".into()),
            provider_id: provider_id.clone(),
            sync_root_id: SyncRootId("root-1".into()),
            remote_root_id: RemoteId("remote-root-1".into()),
        };
        factory.create(config.clone()).unwrap();

        assert_eq!(provider.created_for.lock().unwrap().as_slice(), &[config]);
        assert!(matches!(
            factory.create(RemoteObserverConfig {
                provider_id: ProviderId("google".into()),
                ..provider.created_for.lock().unwrap()[0].clone()
            }),
            Err(RemoteObserverError::UnsupportedProvider)
        ));
    }

    #[test]
    fn cancellation_is_shared_across_token_clones() {
        let token = CancellationToken::new();
        let observer_token = token.clone();
        assert!(!observer_token.is_cancelled());
        token.cancel();
        assert!(observer_token.is_cancelled());
    }

    #[test]
    fn scoped_observer_rejects_account_provider_and_root_mismatches() {
        let config = RemoteObserverConfig {
            account_id: AccountId("account-1".into()),
            provider_id: ProviderId("example".into()),
            sync_root_id: SyncRootId("root-1".into()),
            remote_root_id: RemoteId("remote-root-1".into()),
        };
        let observer = ScopedRemoteObserver {
            config: config.clone(),
            observer: Box::new(StubObserver),
        };
        let session = StubSession {
            account_id: config.account_id.clone(),
            provider_id: config.provider_id.clone(),
        };
        let request = RemoteObserveRequest {
            scope: super::super::models::RemoteScope {
                account_id: config.account_id.clone(),
                provider_id: config.provider_id.clone(),
                sync_root_id: config.sync_root_id.clone(),
                remote_root_id: config.remote_root_id.clone(),
            },
            mode: super::super::models::RemoteObserveMode::Initial,
            page_budget: None,
            known_remote_ids: Vec::new(),
        };
        assert!(observer.accepts(&request, &session));

        let mut mismatched_request = request;
        mismatched_request.scope.account_id = AccountId("another-account".into());
        assert!(!observer.accepts(&mismatched_request, &session));

        let mismatched_session = StubSession {
            account_id: config.account_id.clone(),
            provider_id: ProviderId("another-provider".into()),
        };
        assert!(!observer.accepts(
            &RemoteObserveRequest {
                scope: super::super::models::RemoteScope {
                    account_id: config.account_id.clone(),
                    provider_id: config.provider_id.clone(),
                    sync_root_id: config.sync_root_id.clone(),
                    remote_root_id: config.remote_root_id.clone(),
                },
                mode: super::super::models::RemoteObserveMode::Initial,
                page_budget: None,
                known_remote_ids: Vec::new(),
            },
            &mismatched_session
        ));

        let mut mismatched_provider_request = RemoteObserveRequest {
            scope: super::super::models::RemoteScope {
                account_id: config.account_id.clone(),
                provider_id: config.provider_id.clone(),
                sync_root_id: config.sync_root_id.clone(),
                remote_root_id: config.remote_root_id.clone(),
            },
            mode: super::super::models::RemoteObserveMode::Initial,
            page_budget: None,
            known_remote_ids: Vec::new(),
        };
        mismatched_provider_request.scope.provider_id = ProviderId("another-provider".into());
        assert!(!observer.accepts(&mismatched_provider_request, &session));

        let mut mismatched_root_request = mismatched_provider_request;
        mismatched_root_request.scope.provider_id = config.provider_id.clone();
        mismatched_root_request.scope.sync_root_id = SyncRootId("another-root".into());
        assert!(!observer.accepts(&mismatched_root_request, &session));

        let mut mismatched_remote_root_request = mismatched_root_request;
        mismatched_remote_root_request.scope.sync_root_id = config.sync_root_id;
        mismatched_remote_root_request.scope.remote_root_id =
            RemoteId("another-remote-root".into());
        assert!(!observer.accepts(&mismatched_remote_root_request, &session));
    }
}
