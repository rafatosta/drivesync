//! Modal shell compatible with the project's libadwaita 1.4 baseline.
use super::{browser::Browser, model::*, source::RemoteFolderSource};
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};
type ResponseCallback = Box<dyn Fn(RemoteFolderPickerResult)>;
/// Single-use picker. Retains no external settings and emits exactly one response.
pub struct RemoteFolderPicker {
    window: adw::Window,
    callbacks: Rc<RefCell<Vec<ResponseCallback>>>,
}
impl RemoteFolderPicker {
    pub fn new(
        config: RemoteFolderPickerConfig,
        source: impl RemoteFolderSource + 'static,
    ) -> Self {
        let window = adw::Window::builder()
            .title(gettext("Select remote folder"))
            .modal(true)
            .default_width(900)
            .default_height(600)
            .build();
        window.add_css_class("remote-folder-picker");
        let css = gtk::CssProvider::new();
        css.load_from_data(include_str!("style.css"));
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(&window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        window.connect_destroy({
            let display = gtk::prelude::WidgetExt::display(&window);
            move |_| gtk::style_context_remove_provider_for_display(&display, &css)
        });
        let toolbar = adw::ToolbarView::new();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        margins(&header, 24);
        let icon = super::folder_item::folder_icon(48);
        header.append(&icon);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.set_hexpand(true);
        titles.append(
            &gtk::Label::builder()
                .label(gettext("Select remote folder"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        titles.append(
            &gtk::Label::builder()
                .label(gettext("Choose a remote folder to synchronize"))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        header.append(&titles);
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("circular");
        close.set_valign(gtk::Align::Center);
        close.set_tooltip_text(Some(&gettext("Close")));
        header.append(&close);
        let top = gtk::Box::new(gtk::Orientation::Vertical, 0);
        top.append(&header);
        top.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        toolbar.add_top_bar(&top);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        margins(&content, 24);
        content.set_margin_top(0);
        let account = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let provider = gtk::Image::from_icon_name(&config.provider_icon);
        provider.set_pixel_size(40);
        account.append(&provider);
        let identity = gtk::Box::new(gtk::Orientation::Vertical, 2);
        identity.append(
            &gtk::Label::builder()
                .label(&config.provider_name)
                .xalign(0.0)
                .css_classes(["title-3"])
                .build(),
        );
        identity.append(
            &gtk::Label::builder()
                .label(&config.account_label)
                .xalign(0.0)
                .selectable(true)
                .build(),
        );
        account.append(&identity);
        content.append(&account);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let browser = Browser::new(config.root, Rc::new(source));
        content.append(&browser.root);
        toolbar.set_content(Some(&content));
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        margins(&footer, 24);
        let selected = gtk::Box::new(gtk::Orientation::Vertical, 6);
        selected.set_hexpand(true);
        selected.append(
            &gtk::Label::builder()
                .label(gettext("Selected folder"))
                .xalign(0.0)
                .build(),
        );
        let path_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        path_row.append(&gtk::Image::from_icon_name("folder-symbolic"));
        path_row.append(&browser.selected_path);
        selected.append(&path_row);
        footer.append(&selected);
        let cancel = gtk::Button::with_label(&gettext("Cancel"));
        cancel.set_valign(gtk::Align::Center);
        cancel.set_height_request(44);
        footer.append(&cancel);
        let confirm = gtk::Button::with_label(&gettext("Select folder"));
        confirm.add_css_class("suggested-action");
        confirm.set_valign(gtk::Align::Center);
        confirm.set_height_request(44);
        footer.append(&confirm);
        let bottom = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bottom.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        bottom.append(&footer);
        toolbar.add_bottom_bar(&bottom);
        window.set_content(Some(&toolbar));
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let window = window.downgrade();
            move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            }
        });
        window.add_controller(keys);
        let callbacks: Rc<RefCell<Vec<ResponseCallback>>> = Rc::default();
        let done = Rc::new(std::cell::Cell::new(false));
        let emit: Rc<dyn Fn(RemoteFolderPickerResult)> = Rc::new({
            let callbacks = callbacks.clone();
            move |response| {
                if !done.replace(true) {
                    for callback in callbacks.borrow().iter() {
                        callback(response.clone());
                    }
                }
            }
        });
        confirm.connect_clicked({
            let window = window.downgrade();
            let emit = emit.clone();
            move |_| {
                emit(RemoteFolderPickerResult::Selected(browser.location()));
                if let Some(window) = window.upgrade() {
                    window.close();
                }
            }
        });
        for button in [close, cancel] {
            button.connect_clicked({
                let window = window.downgrade();
                move |_| {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                }
            });
        }
        window.connect_close_request(move |_| {
            emit(RemoteFolderPickerResult::Cancelled);
            gtk::glib::Propagation::Proceed
        });
        Self { window, callbacks }
    }
    /// Presents a single-use modal picker for the calling application window.
    pub fn present(&self, parent: &adw::ApplicationWindow) {
        self.window.set_transient_for(Some(parent));
        self.window.present();
    }
    pub fn connect_response<F: Fn(RemoteFolderPickerResult) + 'static>(&self, callback: F) {
        self.callbacks.borrow_mut().push(Box::new(callback));
    }
}
pub(super) fn margins(widget: &impl IsA<gtk::Widget>, value: i32) {
    widget.set_margin_top(value);
    widget.set_margin_bottom(value);
    widget.set_margin_start(value);
    widget.set_margin_end(value);
}
