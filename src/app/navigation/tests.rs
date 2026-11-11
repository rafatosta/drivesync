//! First-run decisions use local records rather than remote authentication.
use super::*;
use drivesync_engine::{AccountState, ProviderId, Timestamp};
#[test]
fn first_run_and_saved_accounts_choose_the_correct_page() {
    assert_eq!(initial_destination(&[]), InitialDestination::Welcome);
    for state in [
        AccountState::Connected,
        AccountState::ReauthorizationRequired,
        AccountState::Unavailable,
    ] {
        let account = Account {
            id: AccountId("fixture".into()),
            provider: ProviderId("google".into()),
            provider_user_id: "subject".into(),
            email: None,
            display_name: None,
            state,
            created_at: Timestamp { unix_millis: 0 },
            updated_at: Timestamp { unix_millis: 0 },
        };
        assert_eq!(
            initial_destination(&[account]),
            InitialDestination::Dashboard
        );
    }
}
