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
    Provider(ProviderId),
    Back,
    Continue,
    Connect,
    Copy,
    ChooseLocal,
    Dashboard,
}
/// Owns presentation widgets privately; the window is weak to permit dismissal
/// while engine operations are still completing.
pub(super) struct SetupDialog {
    window: gtk::glib::WeakRef<adw::Window>,
    stack: gtk::Stack,
    title: adw::WindowTitle,
    back: gtk::Button,
    next: gtk::Button,
    feedback: gtk::Label,
    provider: ProviderPage,
    connection: ConnectionPage,
    synchronization: SynchronizationPage,
    review: ReviewPage,
}
impl SetupDialog {
    pub fn new(
        parent: Option<&gtk::Window>,
        providers: &[RegisteredProvider],
    ) -> (adw::Window, Self) {
        let window = adw::Window::builder()
            .title(gettextrs::gettext("Set up account"))
            .modal(true)
            .default_width(680)
            .default_height(620)
            .width_request(320)
            .height_request(360)
            .destroy_with_parent(true)
            .build();
        window.set_widget_name("drivesync-account-setup");
        window.set_transient_for(parent);
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
        toolbar.set_content(Some(&body));
        window.set_content(Some(&toolbar));
        let view = Self {
            window: window.downgrade(),
            stack,
            title,
            back,
            next,
            feedback,
            provider,
            connection,
            synchronization,
            review,
        };
        (window, view)
    }
    pub fn window(&self) -> Option<adw::Window> {
        self.window.upgrade()
    }
    pub fn on_event(&self, callback: Rc<dyn Fn(Event)>) {
        let cb = callback.clone();
        self.provider
            .on_selected(Rc::new(move |id| cb(Event::Provider(id))));
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
        self.review.on_dashboard(move || callback(Event::Dashboard));
    }
    pub fn render(&self, state: &SetupState) {
        self.stack.set_visible_child_name(state.step.name());
        self.title.set_subtitle(&state.step.title());
        self.back.set_visible(state.step != Step::Provider);
        self.back.set_sensitive(!state.busy);
        self.next.set_visible(state.step != Step::Review);
        self.next.set_sensitive(state.can_continue());
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
            account.filter(|_| state.authorized()),
            state.busy,
            state.signing_in,
            state.authorization_url.is_some(),
        );
        let path = state.draft.local_uri.as_deref().map(setup::display_path);
        self.synchronization.update(path.as_deref(), !state.busy);
        self.review.update(
            provider,
            account.unwrap_or(""),
            path.as_deref().unwrap_or(""),
        );
    }
    pub fn copy_authorization_url(&self, url: &str) {
        if let Some(window) = self.window() {
            window.clipboard().set_text(url);
        }
    }
}
