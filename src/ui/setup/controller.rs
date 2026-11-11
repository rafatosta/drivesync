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
use std::{cell::RefCell, rc::Rc};

pub(super) struct Controller {
    context: Context,
    navigation: gtk::glib::WeakRef<adw::NavigationView>,
    toasts: gtk::glib::WeakRef<adw::ToastOverlay>,
    providers: Vec<RegisteredProvider>,
    state: RefCell<SetupState>,
    persisted_account: RefCell<Option<AccountId>>,
    view: SetupDialog,
}
impl Controller {
    pub fn attach(
        context: &Context,
        navigation: &adw::NavigationView,
        toasts: &adw::ToastOverlay,
        account: Option<Account>,
    ) -> adw::Window {
        let providers = setup::list_providers();
        let parent = navigation.root().and_downcast::<gtk::Window>();
        let (window, view) = SetupDialog::new(parent.as_ref(), &providers);
        let persisted_account = RefCell::new(account.as_ref().map(|account| account.id.clone()));
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
            navigation: navigation.downgrade(),
            toasts: toasts.downgrade(),
            providers,
            state: RefCell::new(state),
            persisted_account,
            view,
        });
        let weak = Rc::downgrade(&controller);
        controller.view.on_event(Rc::new(move |event| {
            if let Some(controller) = weak.upgrade() {
                controller.handle(event);
            }
        }));
        let closing = controller.clone();
        window.connect_close_request(move |_| {
            closing.close();
            gtk::glib::Propagation::Proceed
        });
        let destroyed = controller.clone();
        window.connect_destroy(move |_| destroyed.close());
        controller.render();
        if controller.state.borrow().account.is_some() {
            controller.load_draft();
        }
        window
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
        if self.state.borrow().closed {
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
            Event::Provider(id) => {
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
            Event::Connect => {
                self.connect();
                return;
            }
            Event::Copy => {
                if let Some(url) = &self.state.borrow().authorization_url {
                    self.view.copy_authorization_url(url);
                }
            }
            Event::ChooseLocal => {
                self.choose_local();
                return;
            }
            Event::Dashboard => {
                if let Some(window) = self.view.window() {
                    window.close();
                }
                return;
            }
        }
        self.render();
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
    fn close(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return;
            }
            state.closed = true;
            state.generation += 1;
            state.authorization_url = None;
            if let (Some(engine), Some(id)) = (self.engine(), state.attempt.clone()) {
                accounts::cancel(&engine, id);
            }
        }
        self.refresh_dashboard();
    }
    fn refresh_dashboard(&self) {
        let account = self.persisted_account.borrow().clone();
        if let (Some(account), Some(nav), Some(toasts)) =
            (account, self.navigation.upgrade(), self.toasts.upgrade())
        {
            crate::application::navigation::connected(&self.context, account, false, &nav, &toasts);
        }
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
                        state.closed || state.generation != generation
                    };
                    if cancelled {
                        accounts::cancel(&engine, id.clone());
                    } else {
                        *owner.context.pending.borrow_mut() = Some(id.clone());
                        owner.state.borrow_mut().authorization_url =
                            Some(begin.authorization_url.clone());
                        owner.state.borrow_mut().feedback =
                            gettext("Complete authorization in your browser, then return here.");
                        owner.render();
                        if accounts::open_authorization_url(&begin.authorization_url)
                            .await
                            .is_err()
                            && !owner.state.borrow().closed
                        {
                            owner.state.borrow_mut().feedback =
                                gettext("Could not open the browser. Copy the link to continue.");
                            owner.render();
                        }
                    }
                    let result = accounts::finish(&engine, id.clone()).await;
                    if owner.context.pending.borrow().as_ref() == Some(&id) {
                        owner.context.pending.borrow_mut().take();
                    }
                    result
                }
                Err(error) => Err(error),
            };
            {
                let mut state = owner.state.borrow_mut();
                state.busy = false;
                state.signing_in = false;
                state.attempt = None;
                state.authorization_url = None;
                match result {
                    Ok(result) => {
                        // Persistence can win a cancellation race; retain the saved identity.
                        *owner.persisted_account.borrow_mut() = Some(result.account.id.clone());
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
            if owner.state.borrow().closed {
                owner.refresh_dashboard();
            } else if owner.state.borrow().account.is_some() {
                owner.load_draft();
            } else {
                owner.render();
            }
        });
    }
    fn load_draft(self: &Rc<Self>) {
        let Some(account) = self.state.borrow().account.clone() else {
            return;
        };
        self.state.borrow_mut().busy = true;
        self.render();
        let owner = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = setup::load_draft(&account.id).await;
            if owner.state.borrow().closed {
                return;
            }
            let mut state = owner.state.borrow_mut();
            state.busy = false;
            match result {
                Ok(draft) => state.draft = draft,
                Err(_) => state.feedback = gettext(
                    "The saved setup draft could not be read. Choose a folder again to replace it.",
                ),
            }
            drop(state);
            owner.render();
        });
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
            if owner.state.borrow().closed {
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
                        owner.save_choices().await;
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
            if !owner.state.borrow().closed {
                owner.render();
            }
        });
    }
    async fn save_choices(&self) -> bool {
        let (account, draft) = {
            let state = self.state.borrow();
            (state.account.clone(), state.draft.clone())
        };
        let Some(account) = account else {
            return false;
        };
        let saved = setup::save_draft(&account.id, &draft).await.is_ok();
        self.state.borrow_mut().feedback = if saved {
            gettext("Folder choice saved as a draft.")
        } else {
            gettext("The setup draft could not be saved. Try again.")
        };
        saved
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
            } else if !owner.state.borrow().closed && owner.save_choices().await {
                owner.state.borrow_mut().step = Step::Review;
            }
            owner.state.borrow_mut().busy = false;
            if !owner.state.borrow().closed {
                owner.render();
            }
        });
    }
}
