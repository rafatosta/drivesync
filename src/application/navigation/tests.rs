//! Authorization and persisted drafts cannot grant dashboard access on their own.
use super::*;
use crate::services::setup::DirectoryCheck;
use drivesync_engine::{AccountState, ProviderId, Timestamp};
#[test]
fn dashboard_requires_authorization_and_both_validated_directories() {
    assert_eq!(initial_destination(&[]), InitialDestination::Setup);
    let account = Account {
        id: AccountId("fixture".into()),
        provider: ProviderId("google".into()),
        provider_user_id: "subject".into(),
        email: None,
        display_name: None,
        state: AccountState::Connected,
        created_at: Timestamp { unix_millis: 0 },
        updated_at: Timestamp { unix_millis: 0 },
    };
    for local in [
        DirectoryCheck::Missing,
        DirectoryCheck::Invalid,
        DirectoryCheck::Valid,
        DirectoryCheck::Unavailable,
    ] {
        for remote in [
            DirectoryCheck::Missing,
            DirectoryCheck::Invalid,
            DirectoryCheck::Valid,
            DirectoryCheck::Unavailable,
        ] {
            for authorized in [false, true] {
                let validation = ConfigurationValidation {
                    authorized,
                    local,
                    remote,
                };
                let expected = if authorized
                    && local == DirectoryCheck::Valid
                    && remote == DirectoryCheck::Valid
                {
                    InitialDestination::Dashboard
                } else {
                    InitialDestination::Setup
                };
                assert_eq!(
                    initial_destination(&[(account.clone(), validation)]),
                    expected
                );
            }
        }
    }
}
