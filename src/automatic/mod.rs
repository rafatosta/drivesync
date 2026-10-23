//! Application-owned automatic downloads. UI pages observe controllers rather
//! than own workers, so hiding/rebuilding a page never duplicates or stops a job.
mod policy;
mod runtime;
pub(crate) use policy::Phase;
pub(crate) use runtime::{configured, install, reconcile, Controller, Snapshot};

/// Temporary authorization transport failures remain eligible for safe retry;
/// revoked credentials, identity failures and storage errors require user action.
pub(crate) fn eligible(saved: &drivesync_engine::RestoredAccount) -> bool {
    use drivesync_engine::AuthError;
    matches!(
        saved.error,
        None | Some(
            AuthError::Network
                | AuthError::ProviderUnavailable
                | AuthError::TimedOut
                | AuthError::Busy
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{AccountId, AccountSummary, AuthError, RestoredAccount};
    #[test]
    fn temporary_startup_failures_retry_but_revocation_and_storage_require_action() {
        let mut saved = RestoredAccount {
            account: AccountSummary {
                id: AccountId("a".into()),
                provider_subject: "a".into(),
                email: None,
            },
            folder: None,
            error: None,
        };
        assert!(eligible(&saved));
        for error in [
            AuthError::Network,
            AuthError::ProviderUnavailable,
            AuthError::TimedOut,
            AuthError::Busy,
        ] {
            saved.error = Some(error);
            assert!(eligible(&saved));
        }
        for error in [
            AuthError::ReauthorizationRequired,
            AuthError::IdentityMismatch,
            AuthError::SecureStorage,
            AuthError::AccountStorage,
            AuthError::MissingPermissions,
        ] {
            saved.error = Some(error);
            assert!(!eligible(&saved));
        }
    }
}
