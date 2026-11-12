//! Coordinates setup state and services without reaching inside page widgets.
use super::{
    dialog::{Event, SetupDialog},
    models::{SetupState, Step},
};
use crate::{
    application::Context,
    services::{
        accounts,
        setup::{self, ConnectionMethod, RegisteredProvider},
    },
};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId, Engine};
use gettextrs::gettext;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};

pub(super) struct Controller {
    context: Context,
    separate_window: bool,
    // Retain routing infrastructure while its view is detached from the overlay.
    navigation: adw::NavigationView,
    toasts: adw::ToastOverlay,
    finishing: Cell<bool>,
    providers: Vec<RegisteredProvider>,
    state: RefCell<SetupState>,
    provisional_accounts: RefCell<BTreeSet<AccountId>>,
    cleanup_running: Cell<bool>,
    hold: RefCell<Option<gtk::gio::ApplicationHoldGuard>>,
    view: SetupDialog,
    remote_open_task: RefCell<Option<gtk::glib::JoinHandle<()>>>,
    remote_picker: RefCell<Option<crate::ui::components::remote_folder_picker::RemoteFolderPicker>>,
    #[cfg(test)]
    remote_fixture: Cell<bool>,
}
impl Controller {
    pub fn attach(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Option<Account>,
    ) -> gtk::Window {
        Self::build(context, navigation, toasts, account).0
    }
    fn build(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Option<Account>,
    ) -> (gtk::Window, Rc<Self>) {
        Self::build_mode(context, navigation, toasts, account, false)
    }
    /// Adds another account without replacing or hiding the primary window.
    pub fn attach_additional(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Option<Account>,
    ) -> gtk::Window {
        Self::build_mode(context, navigation, toasts, account, true).0
    }
    fn build_mode(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Option<Account>,
        separate_window: bool,
    ) -> (gtk::Window, Rc<Self>) {
        let providers = setup::list_providers();
        let parent = navigation
            .root()
            .and_downcast::<gtk::Window>()
            .expect("setup mounts in the application window");
        let window = if separate_window {
            let dialog = adw::Window::builder()
                .transient_for(&parent)
                .modal(false)
                .build();
            dialog.set_application(parent.application().as_ref());
            dialog.upcast::<gtk::Window>()
        } else {
            parent
        };
        let view = SetupDialog::new(&window, &providers);
        let provisional_accounts = RefCell::new(
            account
                .as_ref()
                .map(|account| account.id.clone())
                .into_iter()
                .collect(),
        );
        let mut state = SetupState::default();
        if let Some(account) = account {
            state.provider = providers.iter().find(|v| v.id == account.provider).cloned();
            state.account = Some(account);
            state.step = if state.authorized() {
                Step::Synchronization
            } else {
                Step::Connection
            };
        }
        let controller = Rc::new(Self {
            context: context.clone(),
            separate_window,
            navigation: navigation.clone(),
            toasts: toasts.clone(),
            finishing: Cell::new(false),
            providers,
            state: RefCell::new(state),
            provisional_accounts,
            cleanup_running: Cell::new(false),
            hold: RefCell::new(None),
            view,
            remote_open_task: RefCell::new(None),
            remote_picker: RefCell::new(None),
            #[cfg(test)]
            remote_fixture: Cell::new(false),
        });
        let weak = Rc::downgrade(&controller);
        controller.view.on_event(Rc::new(move |event| {
            if let Some(controller) = weak.upgrade() {
                controller.handle(event);
            }
        }));
        let closing = controller.clone();
        window.connect_close_request(move |_| {
            if closing.state.borrow().closed {
                return gtk::glib::Propagation::Proceed;
            }
            closing.request_close();
            gtk::glib::Propagation::Stop
        });
        if separate_window {
            window
                .downcast_ref::<adw::Window>()
                .unwrap()
                .set_content(Some(controller.view.widget()));
        } else {
            toasts.set_child(Some(controller.view.widget()));
        }
        controller.render();
        (window, controller)
    }
    fn render(&self) {
        // Selection callbacks ignore notifications for the already selected provider.
        let state = self.state.borrow();
        self.view.render(&state);
    }
    fn engine(&self) -> Option<Engine> {
        self.context.engine.borrow().clone()
    }
    fn handle(self: &Rc<Self>, event: Event) {
        if self.state.borrow().closed || self.state.borrow().closing {
            return;
        }
        if matches!(event, Event::Connect) && self.state.borrow().signing_in {
            self.cancel_sign_in();
            return;
        }
        if self.state.borrow().busy && !matches!(event, Event::Copy) {
            return;
        }
        match event {
            Event::Provider(id, selected) => {
                if !selected {
                    let mut state = self.state.borrow_mut();
                    if state
                        .provider
                        .as_ref()
                        .is_some_and(|provider| provider.id == id)
                    {
                        state.provider = None;
                    }
                    drop(state);
                    self.render();
                    return;
                }
                // Ignore a selection notification caused by rendering the same value.
                if self
                    .state
                    .borrow()
                    .provider
                    .as_ref()
                    .is_some_and(|p| p.id == id)
                {
                    return;
                }
                if let Some(provider) = self.providers.iter().find(|v| v.id == id) {
                    self.state.borrow_mut().select_provider(provider.clone());
                }
            }
            Event::Nickname(text) => self.state.borrow_mut().account_nickname = text,
            Event::Back => self.state.borrow_mut().back(),
            Event::Continue => {
                if !self.state.borrow().can_continue() {
                    return;
                }
                let step = self.state.borrow().step;
                match step {
                    Step::Provider => self.state.borrow_mut().step = Step::Connection,
                    Step::Connection => self.state.borrow_mut().step = Step::Synchronization,
                    Step::Synchronization => {
                        self.review();
                        return;
                    }
                    Step::Review => (),
                }
            }
            Event::Finish => {
                self.finish_setup();
                return;
            }
            Event::Connect => {
                self.connect();
                return;
            }
            Event::Copy => {
                if let Some(url) = &self.state.borrow().authorization_url {
                    self.view.copy_authorization_url(url);
                }
            }
            Event::ChooseRemote => {
                self.choose_remote();
                return;
            }
            Event::ChooseLocal => {
                self.choose_local();
                return;
            }
            Event::Disconnect => {
                self.disconnect();
                return;
            }
        }
        self.render();
    }
    fn disconnect(self: &Rc<Self>) {
        let Some(account) = self.state.borrow().account.clone() else {
            return;
        };
        let Some(window) = self.view.window() else {
            return;
        };
        self.state.borrow_mut().busy = true;
        self.render();
        let owner = self.clone();
        crate::application::accounts::disconnect(
            &self.context,
            window.upcast_ref(),
            &account,
            Rc::new(move |event| {
                use crate::application::accounts::DisconnectEvent;
                let mut state = owner.state.borrow_mut();
                state.busy = false;
                match event {
                    DisconnectEvent::Cancelled => (),
                    DisconnectEvent::Failed(message) => state.feedback = message,
                    DisconnectEvent::Disconnected { draft_removed } => {
                        if let Some(account) = state.account.as_ref() {
                            if draft_removed {
                                owner.provisional_accounts.borrow_mut().remove(&account.id);
                            }
                        }
                        if draft_removed {
                            let closing = state.closing;
                            *state = SetupState {
                                closing,
                                ..Default::default()
                            };
                        }
                        state.feedback = if draft_removed {
                            gettext("Account disconnected.")
                        } else {
                            gettext(
                                "Account disconnected. The saved setup draft could not be removed.",
                            )
                        };
                    }
                }
                let closing = state.closing;
                drop(state);
                if closing {
                    owner.maybe_cleanup();
                } else if !owner.state.borrow().closed {
                    owner.render();
                }
            }),
        );
    }
    fn cancel_sign_in(&self) {
        let Some(engine) = self.engine() else {
            return;
        };
        let attempt = self.state.borrow().attempt.clone();
        if let Some(id) = attempt {
            if !accounts::cancel(&engine, id) {
                self.state.borrow_mut().feedback =
                    gettext("Your account is being saved. Please wait.");
                self.render();
                return;
            }
        }
        let mut state = self.state.borrow_mut();
        state.generation += 1;
        state.authorization_url = None;
        state.feedback = gettext("Cancelling sign-in…");
        drop(state);
        self.render();
    }
    /// Normal close and app Quit wait for cancellation and secure local logout.
    fn request_close(self: &Rc<Self>) {
        if self.finishing.get() {
            self.state.borrow_mut().closing = true;
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            if state.closed || state.closing {
                return;
            }
            state.closing = true;
            state.busy = true;
            state.generation += 1;
            state.authorization_url = None;
            state.feedback = gettext("Signing out before closing…");
            if let (Some(engine), Some(id)) = (self.engine(), state.attempt.clone()) {
                accounts::cancel(&engine, id);
            }
        }
        if let Some(task) = self.remote_open_task.borrow_mut().take() {
            task.abort();
        }
        let picker = self.remote_picker.borrow_mut().take();
        if let Some(picker) = picker {
            picker.cancel();
        }
        if self.hold.borrow().is_none() {
            if let Some(app) = self.view.window().and_then(|window| window.application()) {
                *self.hold.borrow_mut() = Some(app.hold());
            }
        }
        self.render();
        self.maybe_cleanup();
    }
    fn cleanup_failed(&self, message: String) {
        self.cleanup_running.set(false);
        let mut state = self.state.borrow_mut();
        state.closing = false;
        state.busy = false;
        state.feedback = message;
        drop(state);
        self.render();
    }
    fn maybe_cleanup(self: &Rc<Self>) {
        if !self.state.borrow().closing
            || self.finishing.get()
            || self.state.borrow().signing_in
            || self.cleanup_running.replace(true)
        {
            return;
        }
        let owner = self.clone();
        gtk::glib::spawn_future_local(async move {
            // A failed credential commit can leave a recoverable registration
            // without returning its ID. Discover it before declaring logout done.
            if let Some(engine) = owner.engine() {
                match engine
                    .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
                    .await
                {
                    Ok(result) => {
                        for account in result.accounts {
                            match crate::services::configuration::load(&account.id).await {
                                Ok(None) => {
                                    owner.provisional_accounts.borrow_mut().insert(account.id);
                                }
                                Ok(Some(_)) => (),
                                Err(_) => {
                                    owner.cleanup_failed(gettext("Saved configuration could not be read. Your account has been preserved."));
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        owner.cleanup_failed(crate::core::messages::engine_error(&error));
                        return;
                    }
                }
            }
            let ids: Vec<_> = owner
                .provisional_accounts
                .borrow()
                .iter()
                .cloned()
                .collect();
            for id in ids {
                let result = match owner.engine() {
                    Some(engine) => match accounts::disconnect(&engine, id.clone()).await {
                        Ok(true) => Ok(()),
                        Ok(false) => Err(gettext(
                            "The saved setup could not be discarded. Try closing again.",
                        )),
                        Err(error) => Err(crate::core::messages::engine_error(&error)),
                    },
                    None => Err(gettext("The account service is unavailable. Try again.")),
                };
                if let Err(message) = result {
                    owner.cleanup_failed(message);
                    return;
                }
                owner.provisional_accounts.borrow_mut().remove(&id);
            }
            *owner.state.borrow_mut() = SetupState {
                closed: true,
                ..Default::default()
            };
            if let Some(window) = owner.view.window() {
                let app = window.application();
                window.destroy();
                if !owner.separate_window {
                    if let Some(app) = app {
                        app.quit();
                    }
                }
            }
            owner.hold.borrow_mut().take();
        });
    }
    fn connect(self: &Rc<Self>) {
        let Some(engine) = self.engine() else {
            return;
        };
        let method = self
            .state
            .borrow()
            .provider
            .as_ref()
            .map(|v| v.connection_method);
        if method != Some(ConnectionMethod::Google) {
            return;
        }
        let generation = {
            let mut state = self.state.borrow_mut();
            state.busy = true;
            state.signing_in = true;
            state.feedback.clear();
            state.generation += 1;
            state.generation
        };
        self.render();
        let owner = self.clone();
        let account_id = self
            .state
            .borrow()
            .account
            .as_ref()
            .map(|account| account.id.clone());
        gtk::glib::spawn_future_local(async move {
            let result = match accounts::begin(&engine, account_id).await {
                Ok(begin) => {
                    let id = begin.connection_id;
                    let cancelled = {
                        let mut state = owner.state.borrow_mut();
                        state.attempt = Some(id.clone());
                        state.closed || state.closing || state.generation != generation
                    };
                    if cancelled {
                        accounts::cancel(&engine, id.clone());
                    } else {
                        *owner.context.pending.borrow_mut() = Some(id.clone());
                        owner.state.borrow_mut().authorization_url =
                            Some(begin.authorization_url.clone());
                        owner.state.borrow_mut().feedback.clear();
                        owner.render();
                        let launching = Rc::downgrade(&owner);
                        gtk::glib::spawn_future_local(async move {
                            if accounts::open_authorization_url(&begin.authorization_url)
                                .await
                                .is_err()
                            {
                                if let Some(owner) = launching.upgrade() {
                                    if !owner.state.borrow().closed && !owner.state.borrow().closing
                                    {
                                        owner.state.borrow_mut().feedback = gettext("Could not open the browser. Copy the link to continue.");
                                        owner.render();
                                    }
                                }
                            }
                        });
                    }
                    let result = accounts::finish(&engine, id.clone()).await;
                    if owner.context.pending.borrow().as_ref() == Some(&id) {
                        owner.context.pending.borrow_mut().take();
                    }
                    result
                }
                Err(error) => Err(error),
            };
            if owner.preserve_completed_account(&result).await {
                return;
            }
            owner.finish_connection(result, generation);
            if owner.state.borrow().closing {
                owner.maybe_cleanup();
            } else if !owner.state.borrow().closed {
                owner.render();
            }
        });
    }
    /// Records even a late durable account before the close path performs logout.
    fn finish_connection(
        &self,
        result: drivesync_engine::EngineResult<drivesync_engine::FinishGoogleSignInResult>,
        generation: u64,
    ) {
        let mut state = self.state.borrow_mut();
        state.busy = false;
        state.signing_in = false;
        state.attempt = None;
        state.authorization_url = None;
        match result {
            Ok(result) => {
                // A commit can win cancellation. Track it before close cleanup.
                self.provisional_accounts
                    .borrow_mut()
                    .insert(result.account.id.clone());
                if state
                    .account
                    .as_ref()
                    .is_some_and(|account| account.id != result.account.id)
                {
                    state.draft = setup::SetupDraft::default();
                    state.remote_folder = None;
                }
                state.remote_authorization_required = false;
                state.account = Some(result.account);
                state.step = Step::Connection;
                state.feedback =
                    gettext("Account connected. Continue to configure synchronization.");
            }
            Err(error) if state.generation == generation => {
                state.feedback = crate::core::messages::engine_error(&error)
            }
            Err(_) => state.feedback = gettext("Sign-in cancelled."),
        }
    }
    #[cfg(test)]
    pub(super) fn with_local_choice(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Account,
        uri: String,
    ) -> gtk::Window {
        let separate = navigation
            .root()
            .is_some_and(|root| root.widget_name() == "drivesync-dashboard");
        let (window, owner) =
            Self::build_mode(context, navigation, toasts, Some(account), separate);
        owner.state.borrow_mut().draft.local_uri = Some(uri);
        owner.remote_fixture.set(true);
        owner.render();
        window.present();
        window
    }
    #[cfg(test)]
    pub(super) fn with_completed_account(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Account,
    ) -> gtk::Window {
        let (window, owner) = Self::build(context, navigation, toasts, None);
        window.present();
        gtk::glib::spawn_future_local(async move {
            let result = Ok(drivesync_engine::FinishGoogleSignInResult {
                account,
                created: false,
            });
            assert!(owner.preserve_completed_account(&result).await);
        });
        window
    }
    #[cfg(test)]
    pub(super) fn closing_with_late_account(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Account,
    ) -> gtk::Window {
        let (window, owner) = Self::build(context, navigation, toasts, None);
        window.present();
        owner.state.borrow_mut().signing_in = true;
        owner.request_close();
        assert!(
            window.is_visible(),
            "close must wait for the authentication result"
        );
        assert!(!owner.cleanup_running.get());
        owner.finish_connection(
            Ok(drivesync_engine::FinishGoogleSignInResult {
                account,
                created: true,
            }),
            0,
        );
        owner.maybe_cleanup();
        window
    }
    /// Resolves the selected account's real root without blocking GTK.
    fn choose_remote(self: &Rc<Self>) {
        let (provider, account, generation) = {
            let mut state = self.state.borrow_mut();
            if state.step != Step::Synchronization || !state.authorized() {
                return;
            }
            let provider = state.provider.clone().unwrap();
            let account = state.account.clone().unwrap();
            state.busy = true;
            state.feedback = gettext("Loading remote folders…");
            state.generation += 1;
            (provider, account, state.generation)
        };
        self.render();
        #[cfg(test)]
        if self.remote_fixture.get() {
            let source = crate::ui::components::remote_folder_picker::MockFolderSource::default();
            self.present_remote_picker(
                provider,
                account,
                generation,
                source.root(),
                source,
                Rc::default(),
            );
            return;
        }
        let Some(engine) = self.engine() else {
            self.state.borrow_mut().busy = false;
            self.state.borrow_mut().feedback =
                gettext("The account service is unavailable. Try again.");
            self.render();
            return;
        };
        let owner = Rc::downgrade(self);
        let task = gtk::glib::spawn_future_local(async move {
            let result =
                crate::ui::components::remote_folder_picker::engine_source::EngineFolderSource::open(engine, account.id.clone()).await;
            let Some(owner) = owner.upgrade() else {
                return;
            };
            if !owner
                .state
                .borrow()
                .remote_response_is_current(generation, &account.id)
            {
                return;
            }
            match result {
                Ok((root, source)) => {
                    let needs_authorization = source.authorization_required();
                    owner.present_remote_picker(
                        provider,
                        account,
                        generation,
                        root,
                        source,
                        needs_authorization,
                    );
                }
                Err(error) => {
                    let mut state = owner.state.borrow_mut();
                    state.busy = false;
                    if crate::ui::components::remote_folder_picker::engine_source::needs_authorization(&error) {
                        state.require_remote_authorization();
                    } else {
                        state.feedback = crate::core::messages::engine_error(&error);
                    }
                    drop(state);
                    owner.render();
                }
            }
        });
        *self.remote_open_task.borrow_mut() = Some(task);
    }
    /// Only this caller knows how remote data is obtained; the picker sees a source.
    fn present_remote_picker(
        self: &Rc<Self>,
        provider: RegisteredProvider,
        account: Account,
        generation: u64,
        root: crate::ui::components::remote_folder_picker::RemoteFolder,
        source: impl crate::ui::components::remote_folder_picker::RemoteFolderSource + 'static,
        needs_authorization: Rc<Cell<bool>>,
    ) {
        use crate::ui::components::remote_folder_picker::{
            RemoteFolderPicker, RemoteFolderPickerConfig,
        };
        let Some(window) = self.view.window() else {
            return;
        };
        self.state.borrow_mut().feedback.clear();
        self.render();
        let picker = RemoteFolderPicker::new(
            RemoteFolderPickerConfig {
                provider_name: provider.name,
                provider_icon_svg: provider.icon_svg.into(),
                account_label: account
                    .email
                    .as_deref()
                    .or(account.display_name.as_deref())
                    .unwrap_or(&account.provider_user_id)
                    .into(),
                root,
            },
            source,
        );
        let owner = Rc::downgrade(self);
        picker.connect_response(move |result| {
            if let Some(owner) = owner.upgrade() {
                let mut state = owner.state.borrow_mut();
                if !state.remote_response_is_current(generation, &account.id) {
                    return;
                }
                if needs_authorization.get() {
                    state.busy = false;
                    state.require_remote_authorization();
                } else {
                    state.finish_remote_choice(generation, &account.id, result);
                }
                drop(state);
                owner.remote_picker.borrow_mut().take();
                owner.render();
            }
        });
        picker.present(&window);
        *self.remote_picker.borrow_mut() = Some(picker);
    }
    fn choose_local(self: &Rc<Self>) {
        let Some(window) = self.view.window() else {
            return;
        };
        self.state.borrow_mut().busy = true;
        self.render();
        let picker = gtk::FileDialog::builder()
            .title(gettext("Choose local folder"))
            .modal(true)
            .build();
        if let Some(uri) = self.state.borrow().draft.local_uri.as_deref() {
            picker.set_initial_folder(Some(&gtk::gio::File::for_uri(uri)));
        }
        let owner = self.clone();
        let weak_window = window.downgrade();
        gtk::glib::spawn_future_local(async move {
            let Some(window) = weak_window.upgrade() else {
                return;
            };
            let result = picker.select_folder_future(Some(&window)).await;
            drop(window);
            if owner.state.borrow().closed || owner.state.borrow().closing {
                return;
            }
            match result {
                Ok(file) => {
                    let uri = file.uri().to_string();
                    if setup::check_local_folder(&uri).await.is_err() {
                        owner.state.borrow_mut().feedback =
                            gettext("The selected folder is unavailable. Choose another folder.");
                    } else {
                        owner.state.borrow_mut().draft.local_uri = Some(uri);
                        owner.state.borrow_mut().feedback = gettext(
                            "Local folder selected. Nothing is saved until setup is completed.",
                        );
                    }
                }
                Err(error)
                    if error.matches(gtk::DialogError::Dismissed)
                        || error.matches(gtk::DialogError::Cancelled) => {}
                Err(_) => {
                    owner.state.borrow_mut().feedback =
                        gettext("The folder chooser could not be opened. Try again.")
                }
            }
            owner.state.borrow_mut().busy = false;
            if !owner.state.borrow().closed && !owner.state.borrow().closing {
                owner.render();
            }
        });
    }
    /// Existing completed identities must never become provisional through Add Account.
    async fn preserve_completed_account(
        self: &Rc<Self>,
        result: &drivesync_engine::EngineResult<drivesync_engine::FinishGoogleSignInResult>,
    ) -> bool {
        let Ok(result) = result else {
            return false;
        };
        let saved = crate::services::configuration::load(&result.account.id).await;
        if matches!(saved, Ok(None)) {
            return false;
        }
        let close = self.state.borrow().closing;
        self.provisional_accounts
            .borrow_mut()
            .remove(&result.account.id);
        self.state.borrow_mut().closed = true;
        self.state.borrow_mut().signing_in = false;
        if !self.separate_window {
            crate::application::navigation::show_dashboard(
                &self.context,
                &self.navigation,
                &self.toasts,
            );
        }
        if saved.is_err() {
            crate::application::navigation::reload_accounts(
                &self.context,
                &self.navigation,
                &self.toasts,
                None,
            );
        } else {
            self.toasts.add_toast(adw::Toast::new(&gettext(
                "This account is already configured. Change its settings in Preferences.",
            )));
        }
        if close || self.separate_window {
            if let Some(window) = self.view.window() {
                window.close();
            }
        }
        true
    }
    /// Clears abandoned registrations while protecting every completed account.
    async fn cleanup_other_accounts(
        &self,
        engine: &Engine,
        selected: &AccountId,
    ) -> Result<(), String> {
        let accounts = engine
            .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
            .await
            .map_err(|e| crate::core::messages::engine_error(&e))?;
        for account in accounts.accounts {
            if &account.id == selected {
                continue;
            }
            match crate::services::configuration::load(&account.id).await {
                Ok(Some(_)) => (),
                Ok(None) => {
                    if !accounts::disconnect(engine, account.id.clone())
                        .await
                        .map_err(|e| crate::core::messages::engine_error(&e))?
                    {
                        return Err(gettext(
                            "The saved setup could not be discarded. Try again.",
                        ));
                    }
                    self.provisional_accounts.borrow_mut().remove(&account.id);
                }
                Err(_) => {
                    return Err(gettext(
                        "Saved configuration could not be read. Your account has been preserved.",
                    ))
                }
            }
        }
        Ok(())
    }
    fn finish_setup(self: &Rc<Self>) {
        if !self.state.borrow().can_finish() {
            return;
        }
        let Some(engine) = self.engine() else {
            self.state.borrow_mut().feedback =
                gettext("The account service is unavailable. Try again.");
            self.render();
            return;
        };
        let value = {
            let state = self.state.borrow();
            crate::services::configuration::Configuration {
                account_id: state.account.as_ref().unwrap().id.clone(),
                local_uri: state.draft.local_uri.clone().unwrap(),
                remote_id: state.remote_folder.as_ref().unwrap().folder.id.0.clone(),
                remote_path: state.remote_path().unwrap(),
            }
        };
        let nickname = self.state.borrow().account_nickname.clone();
        self.finishing.set(true);
        self.state.borrow_mut().busy = true;
        self.state.borrow_mut().feedback = gettext("Checking and saving configuration…");
        self.render();
        let owner = self.clone();
        gtk::glib::spawn_future_local(async move {
            let cleanup = owner
                .cleanup_other_accounts(&engine, &value.account_id)
                .await;
            if let Err(message) = cleanup {
                owner.finishing.set(false);
                owner.state.borrow_mut().busy = false;
                owner.state.borrow_mut().feedback = message;
                if owner.state.borrow().closing {
                    owner.maybe_cleanup();
                } else {
                    owner.render();
                }
                return;
            }

            #[cfg(test)]
            let result = if owner.remote_fixture.get() {
                match setup::check_local_folder(&value.local_uri).await {
                    Ok(()) => {
                        crate::services::configuration::seed_for_test(&value).await;
                        Ok(value)
                    }
                    Err(_) => Err(gettext(
                        "The selected folder is unavailable. Choose another folder.",
                    )),
                }
            } else {
                crate::services::configuration::save_checked_with_nickname(
                    &engine, value, &nickname,
                )
                .await
            };
            #[cfg(not(test))]
            let result = crate::services::configuration::save_checked_with_nickname(
                &engine, value, &nickname,
            )
            .await;
            owner.finishing.set(false);
            match result {
                Ok(value) => {
                    let close = owner.state.borrow().closing;
                    owner
                        .provisional_accounts
                        .borrow_mut()
                        .remove(&value.account_id);
                    owner.state.borrow_mut().closed = true;
                    owner
                        .context
                        .navigation_generation
                        .set(owner.context.navigation_generation.get().wrapping_add(1));
                    if owner.separate_window {
                        let _ = owner.navigation.activate_action("dashboard.refresh", None);
                    } else {
                        crate::application::navigation::show_dashboard(
                            &owner.context,
                            &owner.navigation,
                            &owner.toasts,
                        );
                    }
                    if close || owner.separate_window {
                        if let Some(window) = owner.view.window() {
                            window.close();
                        }
                    }
                }
                Err(message) => {
                    owner.state.borrow_mut().busy = false;
                    owner.state.borrow_mut().feedback = message;
                    if owner.state.borrow().closing {
                        owner.maybe_cleanup();
                    } else {
                        owner.render();
                    }
                }
            }
        });
    }
    fn review(self: &Rc<Self>) {
        let Some(uri) = self.state.borrow().draft.local_uri.clone() else {
            return;
        };
        self.state.borrow_mut().busy = true;
        self.render();
        let owner = self.clone();
        gtk::glib::spawn_future_local(async move {
            if setup::check_local_folder(&uri).await.is_err() {
                owner.state.borrow_mut().feedback =
                    gettext("The selected folder is unavailable. Choose another folder.");
            } else if !owner.state.borrow().closed && !owner.state.borrow().closing {
                owner.state.borrow_mut().step = Step::Review;
            }
            owner.state.borrow_mut().busy = false;
            if !owner.state.borrow().closed && !owner.state.borrow().closing {
                owner.render();
            }
        });
    }
}
