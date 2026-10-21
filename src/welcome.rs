//! First-run presentation only. Account operations go through the engine facade.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, AuthError, Cancellation, DriveSyncEngine, EngineError};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

enum AuthEvent {
    OpenBrowser(String, mpsc::Sender<bool>),
    Finished(Result<AccountSummary, EngineError>),
}

fn text(value: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(value)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .halign(gtk::Align::Center)
        .css_classes([class])
        .build()
}

pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> adw::ToolbarView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let menu = gtk::gio::Menu::new();
    menu.append(Some("Quit"), Some("app.quit"));
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main Menu")
            .menu_model(&menu)
            .build(),
    );
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(32);
    content.set_margin_bottom(32);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // Embed the existing project artwork so it also works from an uninstalled checkout.
    let bytes = gtk::glib::Bytes::from_static(include_bytes!(
        "../data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.Devel.svg"
    ));
    let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
    let image = match gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, None::<&gtk::gio::Cancellable>)
    {
        Ok(pixbuf) => gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf))),
        // Restricted environments may not provide an SVG loader/portal.
        Err(_) => gtk::Image::from_icon_name("folder-download-symbolic"),
    };
    image.set_pixel_size(144);
    image.set_halign(gtk::Align::Center);
    image.update_property(&[gtk::accessible::Property::Label(
        "DriveSync folder and cloud",
    )]);
    content.append(&image);
    content.append(&text("Welcome to DriveSync", "title-1"));
    let description = text(
        "Keep selected Google Drive folders available on your Linux computer. The first milestone brings safe, download-only sync, with support for multiple accounts.",
        "body",
    );
    description.set_max_width_chars(62);
    description.set_halign(gtk::Align::Center);
    content.append(&description);
    content.append(&text("Download only · Technical preview", "caption"));

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.set_halign(gtk::Align::Center);
    let connect = gtk::Button::with_label("Connect Google Account");
    connect.add_css_class("suggested-action");
    connect.add_css_class("pill");
    connect.set_tooltip_text(Some("Sign in securely in your web browser"));
    actions.append(&connect);

    let feedback = text("", "body");
    feedback.set_max_width_chars(60);
    feedback.set_selectable(true);
    feedback.set_visible(false);
    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    actions.append(&spinner);
    let cancel_button = gtk::Button::with_label("Cancel");
    cancel_button.set_visible(false);
    actions.append(&cancel_button);
    let cancel_slot = cancellation.clone();
    cancel_button.connect_clicked(move |button| {
        if let Some(cancel) = cancel_slot.borrow().as_ref() {
            cancel.cancel();
        }
        button.set_sensitive(false);
    });
    content.append(&crate::restoration::build(
        engine.clone(),
        navigation,
        toasts,
        &connect,
    ));
    let navigation = navigation.downgrade();
    let toasts = toasts.downgrade();
    let feedback_clone = feedback.clone();
    connect.connect_clicked(move |button| {
        if !button.is_sensitive() || cancellation.borrow().is_some() {
            return;
        }
        let cancel = Cancellation::new();
        *cancellation.borrow_mut() = Some(cancel.clone());
        button.set_sensitive(false);
        button.set_label("Waiting for Google…");
        spinner.set_visible(true);
        spinner.start();
        cancel_button.set_sensitive(true);
        cancel_button.set_visible(true);
        feedback_clone.set_label("Complete authorization in your web browser, then return here.");
        feedback_clone.set_visible(true);
        let (sender, receiver) = mpsc::channel();
        let worker_engine = engine.clone();
        std::thread::spawn(move || {
            let browser_sender = sender.clone();
            let browser_cancel = cancel.clone();
            let result = worker_engine.add_account(&cancel, move |url| {
                let (acknowledge, reply) = mpsc::channel();
                browser_sender
                    .send(AuthEvent::OpenBrowser(url.into(), acknowledge))
                    .map_err(|_| AuthError::BrowserUnavailable)?;
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    if browser_cancel.is_cancelled() {
                        return Err(AuthError::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        return Err(AuthError::BrowserUnavailable);
                    }
                    match reply.recv_timeout(Duration::from_millis(100)) {
                        Ok(true) => return Ok(()),
                        Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Err(AuthError::BrowserUnavailable)
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    }
                }
            });
            let _ = sender.send(AuthEvent::Finished(result));
        });
        let button = button.clone();
        let spinner = spinner.clone();
        let cancel_button = cancel_button.clone();
        let feedback = feedback_clone.clone();
        let navigation = navigation.clone();
        let toasts = toasts.clone();
        let cancellation = cancellation.clone();
        let page_engine = engine.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
            let result = match receiver.try_recv() {
                Ok(AuthEvent::OpenBrowser(url, acknowledge)) => {
                    if cancellation
                        .borrow()
                        .as_ref()
                        .is_some_and(Cancellation::is_cancelled)
                    {
                        let _ = acknowledge.send(false);
                        return gtk::glib::ControlFlow::Continue;
                    }
                    gtk::glib::spawn_future_local(async move {
                        // GIO uses the desktop OpenURI portal inside Flatpak.
                        let launched = gtk::gio::AppInfo::launch_default_for_uri_future(
                            &url,
                            None::<&gtk::gio::AppLaunchContext>,
                        )
                        .await
                        .is_ok();
                        let _ = acknowledge.send(launched);
                    });
                    return gtk::glib::ControlFlow::Continue;
                }
                Ok(AuthEvent::Finished(result)) => result,
                Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err(EngineError::Authentication(AuthError::Internal))
                }
            };
            cancellation.borrow_mut().take();
            button.set_sensitive(true);
            button.set_label("Connect Google Account");
            spinner.stop();
            spinner.set_visible(false);
            cancel_button.set_visible(false);
            match result {
                Ok(account) => {
                    feedback.set_visible(false);
                    if let (Some(navigation), Some(toasts)) =
                        (navigation.upgrade(), toasts.upgrade())
                    {
                        crate::flow::open_account(
                            page_engine.clone(),
                            account,
                            &navigation,
                            &toasts,
                        );
                    }
                }
                Err(error) => {
                    feedback.set_label(&error.to_string());
                    if let Some(toasts) = toasts.upgrade() {
                        toasts.add_toast(adw::Toast::new(&error.to_string()));
                    }
                }
            }
            gtk::glib::ControlFlow::Break
        });
    });
    let learn = gtk::ToggleButton::with_label("Learn More");
    learn.add_css_class("flat");
    actions.append(&learn);
    content.append(&actions);
    content.append(&feedback);

    let details = gtk::Revealer::new();
    let explanation = text(
        "DriveSync is in early development. Google sign-in is available; file transfers are not functional yet. The MVP will download files from Google Drive to local folders. Local edits will not be uploaded, and remote deletions will not automatically delete local files. Google Workspace documents are outside this milestone.",
        "body",
    );
    explanation.set_max_width_chars(70);
    explanation.set_selectable(true);
    details.set_child(Some(&explanation));
    let details_clone = details.clone();
    learn.connect_toggled(move |button| details_clone.set_reveal_child(button.is_active()));
    content.append(&details);

    let note = text(
        "Browser OAuth · Sign-in opens securely in your web browser. DriveSync will never ask for your Google password here.",
        "dim-label",
    );
    note.set_max_width_chars(80);
    note.set_halign(gtk::Align::Center);
    content.append(&note);

    let clamp = adw::Clamp::builder()
        .maximum_size(1000)
        .vexpand(true)
        .child(&content)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();
    toolbar.set_content(Some(&scroll));
    toolbar
}
