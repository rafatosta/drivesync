//! GTK4/Libadwaita first-run presentation.
//!
//! Constructs the welcome page and its reusable presentation widgets, while
//! Browser authorization is presented by the shared connection module.
use adw::{gtk, prelude::*};
use gettextrs::gettext;

/// First-run content and the entry point to the shared connection window.
struct WelcomeView {
    toolbar: adw::ToolbarView,
    connect: gtk::Button,
    content: gtk::Box,
}

/// Creates a centered, wrapping text label with the requested CSS class.
fn text(value: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(value)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .halign(gtk::Align::Center)
        .css_classes([class])
        .build()
}

/// Builds the welcome view without starting or completing Google OAuth.
///
/// Account restoration and browser authorization have separate screens.
fn build_view() -> WelcomeView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_end(&crate::ui::components::main_menu::button());
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(32);
    content.set_margin_bottom(32);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // Embed a raster export of the existing SVG to avoid an external SVG loader
    // when the app runs in an isolated SDK session or without installed icons.
    let bytes = gtk::glib::Bytes::from_static(include_bytes!("../../../data/icons/welcome.png"));
    let image = match gtk::gdk::Texture::from_bytes(&bytes) {
        Ok(texture) => gtk::Image::from_paintable(Some(&texture)),
        Err(_) => gtk::Image::from_icon_name("folder-download-symbolic"),
    };
    image.set_pixel_size(112);
    image.set_halign(gtk::Align::Center);
    image.update_property(&[gtk::accessible::Property::Label(&gettext(
        "DriveSync application icon",
    ))]);
    content.append(&image);
    content.append(&text(&gettext("Welcome to DriveSync"), "title-1"));
    let description = text(&gettext("Connect an account to get started."), "body");
    description.set_max_width_chars(48);
    description.set_halign(gtk::Align::Center);
    content.append(&description);
    content.append(&text(
        &gettext("Account management · Technical preview"),
        "caption",
    ));

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.set_halign(gtk::Align::Center);
    let connect = gtk::Button::with_label(&gettext("Set up account"));
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    connect.set_tooltip_text(Some(&gettext(
        "Choose a provider and configure your account",
    )));
    actions.append(&connect);
    content.append(&actions);

    let footnote = text(
        &gettext("A calmer place for the files that matter to you."),
        "dim-label",
    );
    footnote.set_max_width_chars(80);
    footnote.set_halign(gtk::Align::Center);
    content.append(&footnote);
    let version = text(crate::core::build_info::VERSION, "caption");
    version.add_css_class("dim-label");
    version.set_selectable(true);
    version.set_tooltip_text(Some(crate::core::build_info::REVISION));
    content.append(&version);

    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .vexpand(true)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroll));

    WelcomeView {
        toolbar,
        connect,
        content,
    }
}

/// Creates the empty dashboard with the add-account action.
#[cfg(test)]
pub(crate) fn build() -> adw::ToolbarView {
    let view = build_view();
    view.connect.set_action_name(Some("connection.open"));
    view.toolbar
}

/// Landing presentation for incomplete saved accounts; no Dashboard is built.
pub(crate) struct PendingAccounts {
    toolbar: adw::ToolbarView,
    actions: Vec<(drivesync_engine::Account, gtk::Button, gtk::Button)>,
}
impl PendingAccounts {
    pub fn new(accounts: &[drivesync_engine::Account]) -> Self {
        let view = build_view();
        view.connect.set_action_name(Some("connection.open"));
        view.content.append(&text(
            &gettext(
                "Complete account setup and validate both folders before opening the dashboard.",
            ),
            "dim-label",
        ));
        let group = adw::PreferencesGroup::builder()
            .title(gettext("Setup pending"))
            .build();
        let mut actions = Vec::new();
        for account in accounts {
            let row = adw::ActionRow::builder()
                .title(
                    account
                        .display_name
                        .as_deref()
                        .or(account.email.as_deref())
                        .unwrap_or("Account"),
                )
                .subtitle(account.email.as_deref().unwrap_or(""))
                .use_markup(false)
                .build();
            let resume = gtk::Button::builder()
                .label(gettext("Continue setup"))
                .valign(gtk::Align::Center)
                .build();
            resume.set_widget_name("pending-account-resume");
            let disconnect = gtk::Button::builder()
                .icon_name("network-offline-symbolic")
                .tooltip_text(gettext("Disconnect account…"))
                .valign(gtk::Align::Center)
                .build();
            disconnect.set_widget_name("pending-account-disconnect");
            row.add_suffix(&resume);
            row.add_suffix(&disconnect);
            group.add(&row);
            actions.push((account.clone(), resume, disconnect));
        }
        group.set_visible(!accounts.is_empty());
        view.content.append(&group);
        view.toolbar.set_widget_name("setup-landing");
        Self {
            toolbar: view.toolbar,
            actions,
        }
    }
    pub fn widget(&self) -> &adw::ToolbarView {
        &self.toolbar
    }
    pub fn on_resume(&self, callback: std::rc::Rc<dyn Fn(drivesync_engine::Account)>) {
        for (account, resume, _) in &self.actions {
            let account = account.clone();
            let callback = callback.clone();
            resume.connect_clicked(move |_| callback(account.clone()));
        }
    }
    pub fn on_disconnect(&self, callback: std::rc::Rc<dyn Fn(drivesync_engine::Account)>) {
        for (account, _, disconnect) in &self.actions {
            let account = account.clone();
            let callback = callback.clone();
            disconnect.connect_clicked(move |_| callback(account.clone()));
        }
    }
    pub fn handle(&self) -> PendingAccountsHandle {
        PendingAccountsHandle {
            actions: self
                .actions
                .iter()
                .map(|(account, resume, disconnect)| {
                    (
                        account.id.clone(),
                        resume.downgrade(),
                        disconnect.downgrade(),
                    )
                })
                .collect(),
        }
    }
}
/// Weak controls for presentation changes without exposing layout details.
#[derive(Clone)]
pub(crate) struct PendingAccountsHandle {
    actions: Vec<(
        drivesync_engine::AccountId,
        gtk::glib::WeakRef<gtk::Button>,
        gtk::glib::WeakRef<gtk::Button>,
    )>,
}
impl PendingAccountsHandle {
    pub fn set_busy(&self, id: &drivesync_engine::AccountId, busy: bool) {
        for (account, resume, disconnect) in &self.actions {
            if account == id {
                if let Some(button) = resume.upgrade() {
                    button.set_sensitive(!busy);
                }
                if let Some(button) = disconnect.upgrade() {
                    button.set_sensitive(!busy);
                }
            }
        }
    }
}
