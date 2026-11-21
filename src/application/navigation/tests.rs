//! Saved accounts remain reachable during account setup.
use super::*;
use drivesync_engine::{AccountState, ProviderId, Timestamp};
#[test]
fn saved_accounts_are_available_even_before_folder_configuration() {
    assert_eq!(initial_destination(&[]), InitialDestination::Setup);
    let mut account = Account {
        id: AccountId("fixture".into()),
        provider: ProviderId("google".into()),
        provider_user_id: "subject".into(),
        email: None,
        display_name: None,
        state: AccountState::Connected,
        created_at: Timestamp { unix_millis: 0 },
        updated_at: Timestamp { unix_millis: 0 },
    };
    assert_eq!(
        initial_destination(&[(account.clone(), false)]),
        InitialDestination::Dashboard
    );
    assert_eq!(
        initial_destination(&[(account.clone(), true)]),
        InitialDestination::Dashboard
    );
    account.state = AccountState::ReauthorizationRequired;
    assert_eq!(
        initial_destination(&[(account, true)]),
        InitialDestination::Dashboard
    );
}
