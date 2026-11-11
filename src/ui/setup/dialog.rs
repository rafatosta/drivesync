//! Dialog composition. Controllers see semantic events and render state only.
use super::{
    models::{SetupState, Step},
    pages::*,
};
use crate::services::setup::{self, RegisteredProvider};
use adw::{gtk, prelude::*};
use drivesync_engine::ProviderId;
use std::rc::Rc;

pub(super) enum Event {
    Provider(ProviderId, bool),
    Back,
    Continue,
    Connect,
    Copy,
    ChooseLocal,
    ChooseRemote,
    Disconnect,
}
/// Owns presentation widgets privately; the window is weak to permit dismissal
/// while engine operations are still completing.
pub(super) struct SetupDialog {
    window: gtk::glib::WeakRef<gtk::Window>,
    root: adw::ToolbarView,
    stack: gtk::Stack,
    title: adw::WindowTitle,
    back: gtk::Button,
    next: gtk::Button,
    disconnect: gtk::Button,
    feedback: gtk::Label,
    provider: ProviderPage,
    connection: ConnectionPage,
    synchronization: SynchronizationPage,
    review: ReviewPage,
}
impl SetupDialog {
    pub fn new(window: &gtk::Window, providers: &[RegisteredProvider]) -> Self {
        window.set_title(Some(&gettextrs::gettext("Set up account")));
        window.set_default_size(680, 620);
        window.set_widget_name("drivesync-account-setup");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("DriveSync", "");
        header.set_title_widget(Some(&title));
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.set_tooltip_text(Some(&gettextrs::gettext("Back")));
        header.pack_start(&back);
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let stack = gtk::Stack::builder().vexpand(true).build();
        let provider = ProviderPage::new(providers);
        let connection = ConnectionPage::new();
        let synchronization = SynchronizationPage::new();
        let review = ReviewPage::new();
        stack.add_named(&crate::ui::scroll(provider.widget(), 560), Some("provider"));
        stack.add_named(
            &crate::ui::scroll(connection.widget(), 560),
            Some("connection"),
        );
        stack.add_named(
            &crate::ui::scroll(synchronization.widget(), 560),
            Some("synchronization"),
        );
        stack.add_named(&crate::ui::scroll(review.widget(), 560), Some("review"));
        body.append(&stack);
        let feedback = gtk::Label::builder()
            .wrap(true)
            .selectable(true)
            .margin_start(24)
            .margin_end(24)
            .build();
        feedback.set_widget_name("setup-feedback");
        body.append(&feedback);
        let next = gtk::Button::builder()
            .label(gettextrs::gettext("Continue"))
            .halign(gtk::Align::End)
            .margin_start(24)
            .margin_end(24)
            .margin_bottom(24)
            .css_classes(["suggested-action"])
            .build();
        next.set_widget_name("setup-continue");
        body.append(&next);
        let disconnect = gtk::Button::builder()
            .label(gettextrs::gettext("Disconnect account…"))
            .halign(gtk::Align::Center)
            .margin_bottom(24)
            .css_classes(["destructive-action"])
            .build();
        disconnect.set_widget_name("setup-disconnect");
        body.append(&disconnect);
        toolbar.set_content(Some(&body));
        Self {
            root: toolbar,
            window: window.downgrade(),
            stack,
            title,
            back,
            next,
            disconnect,
            feedback,
            provider,
            connection,
            synchronization,
            review,
        }
    }
    pub fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }
    pub fn window(&self) -> Option<gtk::Window> {
        self.window.upgrade()
    }
    pub fn on_event(&self, callback: Rc<dyn Fn(Event)>) {
        let cb = callback.clone();
        self.provider.on_selected(Rc::new(move |id, selected| {
            cb(Event::Provider(id, selected))
        }));
        let cb = callback.clone();
        self.back.connect_clicked(move |_| cb(Event::Back));
        let cb = callback.clone();
        self.next.connect_clicked(move |_| cb(Event::Continue));
        let cb = callback.clone();
        self.connection.on_connect(move || cb(Event::Connect));
        let cb = callback.clone();
        self.connection.on_copy(move || cb(Event::Copy));
        let cb = callback.clone();
        self.synchronization
            .on_choose(move || cb(Event::ChooseLocal));
        let cb = callback.clone();
        self.synchronization
            .on_choose_remote(move || cb(Event::ChooseRemote));
        self.disconnect
            .connect_clicked(move |_| callback(Event::Disconnect));
    }
    pub fn render(&self, state: &SetupState) {
        self.stack.set_visible_child_name(state.step.name());
        self.title.set_subtitle(&state.step.title());
        self.back.set_visible(state.step != Step::Provider);
        self.back.set_sensitive(!state.busy);
        self.next.set_visible(state.step != Step::Review);
        self.next.set_sensitive(state.can_continue());
        self.disconnect.set_visible(state.account.is_some());
        self.disconnect.set_sensitive(!state.busy);
        self.feedback.set_label(&state.feedback);
        self.feedback.set_visible(!state.feedback.is_empty());
        self.provider
            .set_selected(state.provider.as_ref().map(|v| &v.id), !state.busy);
        let provider = state
            .provider
            .as_ref()
            .map(|v| v.name.as_str())
            .unwrap_or("");
        let account = state.account.as_ref().map(|v| {
            v.email
                .as_deref()
                .or(v.display_name.as_deref())
                .unwrap_or(&v.provider_user_id)
        });
        self.connection.update(
            provider,
            account.filter(|_| state.authorized() || state.remote_authorization_required),
            state.busy,
            state.signing_in && !state.closing,
            state.authorization_url.is_some(),
            state.remote_authorization_required,
        );
        let path = state.draft.local_uri.as_deref().map(setup::display_path);
        let remote = state.remote_path();
        self.synchronization.update(
            path.as_deref(),
            remote.as_deref(),
            !state.busy && state.authorized(),
        );
        self.review.update(
            provider,
            account.unwrap_or(""),
            path.as_deref().unwrap_or(""),
            remote.as_deref().unwrap_or(""),
        );
    }
    pub fn copy_authorization_url(&self, url: &str) {
        if let Some(window) = self.window() {
            window.clipboard().set_text(url);
        }
    }
}
