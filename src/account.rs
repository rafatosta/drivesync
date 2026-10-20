//! Sanitized account information; the UI never receives OAuth tokens.
use adw::{gtk, prelude::*};
use drivesync_engine::AccountSummary;

pub fn build(account: &AccountSummary, toasts: &adw::ToastOverlay) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let status = adw::StatusPage::builder()
        .icon_name("emblem-ok-symbolic")
        .title("Google Account Connected")
        .description("Your account is authenticated. Synchronization has not been configured yet.")
        .build();
    content.append(&status);
    let group = adw::PreferencesGroup::new();
    for (title, value) in [
        (
            "Google account identifier",
            account.provider_subject.as_str(),
        ),
        (
            "Email",
            account.email.as_deref().unwrap_or("Not provided by Google"),
        ),
        ("Authentication", "Authenticated"),
    ] {
        let row = adw::ActionRow::builder()
            .use_markup(false)
            .title(title)
            .subtitle(value)
            .subtitle_selectable(true)
            .build();
        group.add(&row);
    }
    content.append(&group);
    let next = gtk::Button::with_label("Continue Sync Setup");
    next.set_halign(gtk::Align::Center);
    next.add_css_class("suggested-action");
    next.add_css_class("pill");
    let toasts = toasts.downgrade();
    next.connect_clicked(move |_| {
        if let Some(toasts) = toasts.upgrade() {
            toasts.add_toast(adw::Toast::new(
                "Folder selection is not available yet. No synchronization has been started.",
            ));
        }
    });
    content.append(&next);
    let clamp = adw::Clamp::builder()
        .maximum_size(640)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroll));
    adw::NavigationPage::builder()
        .title("Account Information")
        .child(&toolbar)
        .build()
}
