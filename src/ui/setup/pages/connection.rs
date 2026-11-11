//! Browser sign-in presentation, independent of authorization implementation.
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct ConnectionPage {
    root: gtk::Box,
    provider: gtk::Label,
    identity: gtk::Label,
    connect: gtk::Button,
    copy: gtk::Button,
    spinner: gtk::Spinner,
}
impl ConnectionPage {
    pub fn new() -> Self {
        let root = super::content(
            &gettextrs::gettext("Connect account"),
            &gettextrs::gettext(
                "Authorize DriveSync in your browser. DriveSync does not store your password.",
            ),
            "avatar-default-symbolic",
        );
        let provider = gtk::Label::builder()
            .css_classes(["heading"])
            .wrap(true)
            .build();
        let identity = gtk::Label::builder().wrap(true).selectable(true).build();
        let spinner = gtk::Spinner::new();
        let connect = gtk::Button::with_label(&gettext("Sign In…"));
        connect.set_widget_name("setup-connect");
        connect.add_css_class("suggested-action");
        let copy = gtk::Button::with_label(&gettext("Copy Link"));
        root.append(&provider);
        root.append(&identity);
        root.append(&spinner);
        root.append(&connect);
        root.append(&copy);
        Self {
            root,
            provider,
            identity,
            connect,
            copy,
            spinner,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(
        &self,
        provider: &str,
        identity: Option<&str>,
        busy: bool,
        signing_in: bool,
        has_url: bool,
        reauthorize: bool,
    ) {
        self.provider.set_label(provider);
        self.identity.set_label(identity.unwrap_or(""));
        self.connect.set_visible(identity.is_none() || reauthorize);
        self.connect.set_sensitive(!busy || signing_in);
        self.connect.set_label(&if signing_in {
            gettext("Cancel sign-in")
        } else if reauthorize {
            gettext("Authorize Drive access…")
        } else {
            gettext("Sign In…")
        });
        self.copy.set_visible(identity.is_none() || reauthorize);
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
