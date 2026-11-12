//! Completed configuration owns startup routing; authorization failure must not erase it.
use super::*;
use drivesync_engine::{AccountState, ProviderId, Timestamp};
#[test]
fn only_completed_records_grant_dashboard_access_and_survive_reauthorization() {
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
        InitialDestination::Setup
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
