use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, DriveSyncEngine, EngineError, RestoredAccount};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

/// GTK owns completion; disk, network and vault work stay off the main thread.
pub fn work<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, EngineError> + Send + 'static,
    done: impl FnOnce(Result<T, EngineError>) + 'static,
) {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(job());
    });
    let mut done = Some(done);
    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
        let result = match receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => Err(EngineError::Authentication(
                drivesync_engine::AuthError::Internal,
            )),
        };
        if let Some(done) = done.take() {
            done(result);
        }
        gtk::glib::ControlFlow::Break
    });
}
#[derive(Debug, PartialEq, Eq)]
pub enum InitialDestination {
    Accounts,
    FolderSetup,
    Dashboard,
}
pub fn initial_destination(accounts: &[RestoredAccount]) -> InitialDestination {
    if accounts.iter().any(|a| a.folder.is_some()) {
        InitialDestination::Dashboard
    } else if accounts.len() == 1 && accounts[0].error.is_none() {
        InitialDestination::FolderSetup
    } else {
        InitialDestination::Accounts
    }
}
pub fn account_page(
    engine: Arc<DriveSyncEngine>,
    saved: RestoredAccount,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    if saved.folder.is_some() {
        crate::dashboard::build(engine, vec![saved], nav, toasts)
    } else {
        crate::setup::build(engine, saved.account, nav, toasts)
    }
}
pub fn open_account(
    engine: Arc<DriveSyncEngine>,
    account: AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) {
    let worker = engine.clone();
    let id = account.id.clone();
    let nav = nav.downgrade();
    let toasts = toasts.downgrade();
    work(
        move || {
            if worker.saved_folder(&id)?.is_some() {
                worker.restore_accounts().map(Some)
            } else {
                Ok(None)
            }
        },
        move |result| {
            if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                match result {
                    Ok(Some(accounts)) => {
                        nav.push(&crate::dashboard::build(engine, accounts, &nav, &toasts))
                    }
                    Ok(None) => nav.push(&crate::setup::build(engine, account, &nav, &toasts)),
                    Err(e) => toasts.add_toast(adw::Toast::new(&e.to_string())),
                }
            }
        },
    );
}
pub fn row(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    row
}
pub fn scroll(content: &impl IsA<gtk::Widget>, maximum: i32) -> gtk::ScrolledWindow {
    let clamp = adw::Clamp::builder()
        .maximum_size(maximum)
        .child(content)
        .build();
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build()
}
pub fn content() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{AccountId, AuthError, DriveFolder, FolderConfiguration};
    fn saved(configured: bool, error: Option<AuthError>) -> RestoredAccount {
        let id = AccountId("google:test".into());
        RestoredAccount {
            account: AccountSummary {
                id: id.clone(),
                provider_subject: "test".into(),
                email: None,
            },
            error,
            folder: configured.then(|| FolderConfiguration {
                account_id: id,
                remote: DriveFolder {
                    id: "folder-A".into(),
                    name: "Projects".into(),
                },
                local: "/tmp/local".into(),
                local_display: "/tmp/local".into(),
            }),
        }
    }
    #[test]
    fn initial_route_depends_on_saved_configuration_not_authentication_success() {
        assert_eq!(initial_destination(&[]), InitialDestination::Accounts);
        assert_eq!(
            initial_destination(&[saved(false, None)]),
            InitialDestination::FolderSetup
        );
        assert_eq!(
            initial_destination(&[saved(false, Some(AuthError::Network))]),
            InitialDestination::Accounts
        );
        for error in [
            None,
            Some(AuthError::Network),
            Some(AuthError::ReauthorizationRequired),
        ] {
            assert_eq!(
                initial_destination(&[saved(true, error)]),
                InitialDestination::Dashboard
            );
        }
        assert_eq!(
            initial_destination(&[
                saved(false, None),
                saved(true, Some(AuthError::SecureStorage))
            ]),
            InitialDestination::Dashboard
        );
        assert_eq!(
            initial_destination(&[saved(false, None), saved(false, None)]),
            InitialDestination::Accounts
        );
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    #[ignore = "requires a desktop display and an isolated XDG_DATA_HOME"]
    fn folder_setup_and_dashboard_layouts() {
        adw::init().unwrap();
        let output =
            std::env::var("DRIVESYNC_UI_PREVIEW_DIR").expect("explicit preview output directory");
        let data = std::env::var("XDG_DATA_HOME").expect("isolated profile directory");
        assert!(data.starts_with("/tmp/drivesync-ui-"));
        let engine = Arc::new(DriveSyncEngine::new());
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_icon_theme_name(Some("Adwaita"));
        }
        for width in [1000, 360] {
            for dashboard in [false, true] {
                let nav = adw::NavigationView::new();
                let toasts = adw::ToastOverlay::new();
                toasts.set_child(Some(&nav));
                let root = adw::NavigationPage::builder()
                    .title("Layout fixture")
                    .child(&gtk::Box::new(gtk::Orientation::Vertical, 0))
                    .tag("welcome")
                    .build();
                nav.add(&root);
                let account = AccountSummary {
                    id: drivesync_engine::AccountId("google:layout-fixture".into()),
                    provider_subject: "layout-fixture".into(),
                    email: Some("preview@example.com".into()),
                };
                let page = if dashboard {
                    crate::dashboard::build(
                        engine.clone(),
                        vec![RestoredAccount {
                            account,
                            error: None,
                            folder: Some(drivesync_engine::FolderConfiguration {
                                account_id: drivesync_engine::AccountId(
                                    "google:layout-fixture".into(),
                                ),
                                remote: drivesync_engine::DriveFolder {
                                    id: "layout-folder".into(),
                                    name: "Projects".into(),
                                },
                                local: "/home/preview/DriveSync/Projects".into(),
                                local_display: "/home/preview/DriveSync/Projects".into(),
                            }),
                        }],
                        &nav,
                        &toasts,
                    )
                } else {
                    crate::setup::build(engine.clone(), account, &nav, &toasts)
                };
                nav.push(&page);
                let window = adw::Window::builder()
                    .default_width(width)
                    .default_height(800)
                    .content(&toasts)
                    .build();
                window.present();
                let wait = gtk::glib::MainLoop::new(None, false);
                let finish = wait.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    finish.quit()
                });
                wait.run();
                assert!(
                    window.width() <= width + 10,
                    "layout overflow: {} > {}",
                    window.width(),
                    width
                );
                let snapshot = gtk::Snapshot::new();
                gtk::WidgetPaintable::new(Some(&window)).snapshot(
                    &snapshot,
                    window.width() as f64,
                    window.height() as f64,
                );
                let node = snapshot.to_node().expect("rendered layout");
                let texture = window.renderer().unwrap().render_texture(&node, None);
                texture
                    .save_to_png(format!(
                        "{output}/{}-{width}.png",
                        if dashboard { "dashboard" } else { "setup" }
                    ))
                    .unwrap();
                window.close();
            }
        }
    }
}
