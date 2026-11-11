//! Setup navigation and operation state, independent of widget structure.
use crate::services::setup::{RegisteredProvider, SetupDraft};
use crate::ui::components::remote_folder_picker::{RemoteFolderLocation, RemoteFolderPickerResult};
use drivesync_engine::{Account, AccountState, GoogleSignInId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Step {
    #[default]
    Provider,
    Connection,
    Synchronization,
    Review,
}
impl Step {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Connection => "connection",
            Self::Synchronization => "synchronization",
            Self::Review => "review",
        }
    }
    pub(super) fn title(self) -> String {
        match self {
            Self::Provider => gettextrs::gettext("1 of 4 · Choose provider"),
            Self::Connection => gettextrs::gettext("2 of 4 · Connect account"),
            Self::Synchronization => gettextrs::gettext("3 of 4 · Configure synchronization"),
            Self::Review => gettextrs::gettext("4 of 4 · Start synchronization"),
        }
    }
}
/// One account's setup state. Provider changes cannot reuse another identity.
#[derive(Default)]
pub(super) struct SetupState {
    pub step: Step,
    pub provider: Option<RegisteredProvider>,
    pub account: Option<Account>,
    pub draft: SetupDraft,
    /// Current setup only; mock selections never establish remote validity.
    pub remote_folder: Option<RemoteFolderLocation>,
    pub busy: bool,
    pub signing_in: bool,
    pub closed: bool,
    pub closing: bool,
    pub attempt: Option<GoogleSignInId>,
    pub authorization_url: Option<String>,
    pub feedback: String,
    /// Invalidates asynchronous results after cancellation or a newer operation.
    pub generation: u64,
}
impl SetupState {
    pub fn select_provider(&mut self, provider: RegisteredProvider) {
        if self
            .provider
            .as_ref()
            .is_some_and(|old| old.id != provider.id)
            || self
                .account
                .as_ref()
                .is_some_and(|account| account.provider != provider.id)
        {
            self.account = None;
            self.draft = SetupDraft::default();
            self.remote_folder = None;
            self.generation += 1;
        }
        self.provider = Some(provider);
    }
    pub fn can_continue(&self) -> bool {
        !self.busy
            && !self.closed
            && !self.closing
            && self.provider.is_some()
            && match self.step {
                Step::Provider => self.provider.is_some(),
                Step::Connection => self.authorized(),
                Step::Synchronization => {
                    self.authorized()
                        && self.draft.local_uri.is_some()
                        && self.remote_folder.is_some()
                }
                Step::Review => false,
            }
    }
    /// Applies only a response for the still-active account and operation.
    /// Cancellation keeps the previous selection; closing invalidates the result.
    pub fn finish_remote_choice(
        &mut self,
        generation: u64,
        account_id: &drivesync_engine::AccountId,
        result: RemoteFolderPickerResult,
    ) {
        if self.closed
            || self.closing
            || self.generation != generation
            || self.account.as_ref().map(|account| &account.id) != Some(account_id)
        {
            return;
        }
        self.busy = false;
        if let RemoteFolderPickerResult::Selected(location) = result {
            self.remote_folder = Some(location);
            self.feedback = gettextrs::gettext(
                "Simulated remote folder selected. No remote files were accessed.",
            );
        }
    }
    pub fn remote_path(&self) -> Option<String> {
        self.remote_folder.as_ref().map(|location| {
            location
                .path
                .iter()
                .map(|folder| folder.name.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        })
    }
    pub fn authorized(&self) -> bool {
        self.account.as_ref().is_some_and(|account| {
            account.state == AccountState::Connected
                && self
                    .provider
                    .as_ref()
                    .is_some_and(|provider| provider.id == account.provider)
        })
    }
    pub fn back(&mut self) {
        if self.busy {
            return;
        }
        self.step = match self.step {
            Step::Provider => Step::Provider,
            Step::Connection => Step::Provider,
            Step::Synchronization => Step::Connection,
            Step::Review => Step::Synchronization,
        };
        self.feedback.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::ProviderId;
    #[test]
    fn changing_provider_discards_dependent_choices() {
        let mut state = SetupState::default();
        let provider = crate::services::setup::list_providers().remove(0);
        state.select_provider(provider.clone());
        state.draft.local_uri = Some("file:///tmp/old".into());
        let root = crate::ui::components::remote_folder_picker::MockFolderSource::default().root();
        state.remote_folder = Some(RemoteFolderLocation {
            folder: root.clone(),
            path: vec![root],
        });
        state.select_provider(provider.clone());
        assert!(state.draft.local_uri.is_some());
        assert!(state.remote_folder.is_some());
        let mut other = provider;
        other.id = ProviderId("fixture-other".into());
        state.select_provider(other);
        assert!(state.draft.local_uri.is_none());
        assert!(state.remote_folder.is_none());
        assert_eq!(state.generation, 1);
    }
    #[test]
    fn authorization_and_matching_provider_are_required_before_configuration() {
        let provider = crate::services::setup::list_providers().remove(0);
        let mut state = SetupState {
            provider: Some(provider.clone()),
            step: Step::Connection,
            ..Default::default()
        };
        assert!(!state.can_continue());
        state.account = Some(Account {
            id: drivesync_engine::AccountId("fixture".into()),
            provider: provider.id,
            provider_user_id: "subject".into(),
            email: None,
            display_name: None,
            state: AccountState::ReauthorizationRequired,
            created_at: drivesync_engine::Timestamp { unix_millis: 0 },
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        });
        assert!(!state.can_continue());
        state.account.as_mut().unwrap().state = AccountState::Connected;
        assert!(state.can_continue());
        state.step = Step::Synchronization;
        assert!(!state.can_continue());
        state.draft.local_uri = Some("file:///tmp/fixture".into());
        assert!(!state.can_continue());
        let root = crate::ui::components::remote_folder_picker::MockFolderSource::default().root();
        state.remote_folder = Some(RemoteFolderLocation {
            folder: root.clone(),
            path: vec![root],
        });
        assert!(state.can_continue());
        state.closed = true;
        assert!(!state.can_continue());
    }
    #[test]
    fn busy_operations_block_navigation_and_back_keeps_choices() {
        let mut state = SetupState {
            step: Step::Review,
            busy: true,
            ..Default::default()
        };
        state.back();
        assert_eq!(state.step, Step::Review);
        assert!(!state.can_continue());
        state.busy = false;
        state.draft.local_uri = Some("file:///tmp/choice".into());
        state.back();
        assert_eq!(state.step, Step::Synchronization);
        assert!(state.draft.local_uri.is_some());
    }
}

#[cfg(test)]
mod remote_choice_tests {
    use super::*;
    #[test]
    fn cancellation_and_stale_responses_preserve_the_current_selection() {
        let provider = crate::services::setup::list_providers().remove(0);
        let account = Account {
            id: drivesync_engine::AccountId("fixture".into()),
            provider: provider.id.clone(),
            provider_user_id: "subject".into(),
            email: None,
            display_name: None,
            state: AccountState::Connected,
            created_at: drivesync_engine::Timestamp { unix_millis: 0 },
            updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
        };
        let root = crate::ui::components::remote_folder_picker::MockFolderSource::default().root();
        let selected = RemoteFolderLocation {
            folder: root.clone(),
            path: vec![root],
        };
        let mut state = SetupState {
            provider: Some(provider),
            account: Some(account.clone()),
            generation: 3,
            busy: true,
            ..Default::default()
        };
        state.finish_remote_choice(
            3,
            &account.id,
            RemoteFolderPickerResult::Selected(selected.clone()),
        );
        assert!(!state.busy);
        assert_eq!(state.remote_folder, Some(selected.clone()));
        state.busy = true;
        state.finish_remote_choice(3, &account.id, RemoteFolderPickerResult::Cancelled);
        assert!(!state.busy);
        assert_eq!(state.remote_folder, Some(selected.clone()));
        state.busy = true;
        state.finish_remote_choice(2, &account.id, RemoteFolderPickerResult::Cancelled);
        assert!(state.busy);
        state.finish_remote_choice(
            3,
            &drivesync_engine::AccountId("other-account".into()),
            RemoteFolderPickerResult::Cancelled,
        );
        assert!(state.busy);
        state.closing = true;
        state.finish_remote_choice(3, &account.id, RemoteFolderPickerResult::Cancelled);
        assert!(state.busy);
        assert_eq!(state.remote_folder, Some(selected));
    }
}
