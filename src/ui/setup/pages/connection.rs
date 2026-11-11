//! Browser sign-in presentation, independent of authorization implementation.
use crate::services::setup::RegisteredProvider;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct ConnectionPage {
    root: gtk::Box,
    icon: gtk::Image,
    provider: gtk::Label,
    identity: gtk::Label,
    connect: gtk::Button,
    connect_label: gtk::Label,
    sign_in_icon: gtk::Image,
    copy: gtk::Button,
    spinner: gtk::Spinner,
}
impl ConnectionPage {
    pub fn new() -> Self {
        let root = crate::ui::content();
        root.set_spacing(12);
        root.set_valign(gtk::Align::Center);
        let icon = gtk::Image::builder()
            .pixel_size(96)
            .halign(gtk::Align::Center)
            .build();
        let provider = gtk::Label::builder()
            .css_classes(["title-1"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        let description = gtk::Label::builder()
            .label(gettext("Connect your Google account to allow DriveSync to access your files on Google Drive."))
            .wrap(true).justify(gtk::Justification::Center).max_width_chars(52).halign(gtk::Align::Center).build();
        let identity = gtk::Label::builder().wrap(true).selectable(true).build();
        let spinner = gtk::Spinner::new();
        let connect = gtk::Button::builder()
            .halign(gtk::Align::Center)
            .width_request(260)
            .height_request(44)
            .build();
        connect.set_widget_name("setup-connect");
        connect.add_css_class("setup-google-sign-in");
        let button_content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        button_content.set_halign(gtk::Align::Center);
        let sign_in_icon = crate::ui::components::svg_icon(
            include_str!("../../../../data/icons/google-sign-in.svg"),
            24,
        );
        let connect_label = gtk::Label::new(Some(&gettext("Sign in with Google")));
        connect_label.set_widget_name("setup-connect-label");
        connect_label.add_css_class("heading");
        button_content.append(&sign_in_icon);
        button_content.append(&connect_label);
        connect.set_child(Some(&button_content));
        let copy = gtk::Button::with_label(&gettext("Copy Link"));
        copy.set_widget_name("setup-copy");
        copy.set_halign(gtk::Align::Center);
        copy.add_css_class("flat");
        let security = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        security.set_halign(gtk::Align::Center);
        security.set_margin_top(8);
        let shield = gtk::Image::from_icon_name("security-high-symbolic");
        shield.set_pixel_size(20);
        shield.add_css_class("dim-label");
        security.append(&shield);
        security.append(&gtk::Label::builder()
            .label(gettext("You will be redirected to Google to authorize access.\nDriveSync does not store your password."))
            .wrap(true).xalign(0.0).max_width_chars(48).css_classes(["caption"]).build());
        root.append(&icon);
        root.append(&provider);
        root.append(&description);
        root.append(&identity);
        root.append(&spinner);
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        actions.set_margin_top(12);
        actions.set_margin_bottom(12);
        actions.append(&connect);
        actions.append(&copy);
        root.append(&actions);
        root.append(&security);
        Self {
            root,
            icon,
            provider,
            identity,
            connect,
            connect_label,
            sign_in_icon,
            copy,
            spinner,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(
        &self,
        provider: Option<&RegisteredProvider>,
        identity: Option<&str>,
        busy: bool,
        signing_in: bool,
        has_url: bool,
        reauthorize: bool,
    ) {
        let name = provider.map(|p| p.name.as_str()).unwrap_or("");
        if self.provider.text() != name {
            if let Some(provider) = provider {
                let bytes = gtk::glib::Bytes::from_static(provider.icon_svg.as_bytes());
                self.icon.set_from_gicon(&gtk::gio::BytesIcon::new(&bytes));
            } else {
                self.icon.clear();
            }
            self.provider.set_label(name);
        }
        self.identity.set_label(identity.unwrap_or(""));
        self.identity.set_visible(identity.is_some());
        self.connect.set_visible(identity.is_none() || reauthorize);
        self.connect.set_sensitive(!busy || signing_in);
        let label = if signing_in {
            gettext("Cancel sign-in")
        } else if reauthorize {
            gettext("Authorize Drive access…")
        } else {
            gettext("Sign in with Google")
        };
        self.connect_label.set_label(&label);
        self.connect
            .update_property(&[gtk::accessible::Property::Label(&label)]);
        self.sign_in_icon.set_visible(!signing_in);
        self.copy
            .set_visible(has_url && (identity.is_none() || reauthorize));
        self.copy.set_sensitive(has_url);
        self.spinner.set_spinning(busy);
        self.spinner.set_visible(busy);
    }
    pub fn on_connect(&self, callback: impl Fn() + 'static) {
        self.connect.connect_clicked(move |_| callback());
    }
    pub fn on_copy(&self, callback: impl Fn() + 'static) {
        self.copy.connect_clicked(move |_| callback());
    }
}
