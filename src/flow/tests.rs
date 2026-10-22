//! Unit tests for initial routing decisions.
use super::navigation::{initial_destination, InitialDestination};
use drivesync_engine::{
    AccountId, AccountSummary, AuthError, DriveFolder, FolderConfiguration, RestoredAccount,
};

fn saved(configured: bool, error: Option<AuthError>) -> RestoredAccount {
    let id = AccountId("google:test".into());
    RestoredAccount {
        account: AccountSummary {
            id: id.clone(),
            provider_subject: "test".into(),
            email: None,
        },
        error,
        folder: configured.then(|| FolderConfiguration {
            account_id: id,
            remote: DriveFolder {
                id: "folder-A".into(),
                name: "Projects".into(),
            },
            local: "/tmp/local".into(),
            local_display: "/tmp/local".into(),
        }),
    }
}
#[test]
fn initial_route_depends_on_saved_configuration_not_authentication_success() {
    assert_eq!(initial_destination(&[]), InitialDestination::Accounts);
    assert_eq!(
        initial_destination(&[saved(false, None)]),
        InitialDestination::FolderSetup
    );
    assert_eq!(
        initial_destination(&[saved(false, Some(AuthError::Network))]),
        InitialDestination::Dashboard
    );
    for error in [
        None,
        Some(AuthError::Network),
        Some(AuthError::ReauthorizationRequired),
    ] {
        assert_eq!(
            initial_destination(&[saved(true, error)]),
            InitialDestination::Dashboard
        );
    }
    assert_eq!(
        initial_destination(&[
            saved(false, None),
            saved(true, Some(AuthError::SecureStorage))
        ]),
        InitialDestination::Dashboard
    );
    assert_eq!(
        initial_destination(&[saved(false, None), saved(false, None)]),
        InitialDestination::Dashboard
    );
}
