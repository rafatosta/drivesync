//! Reproducible screenshots of production widgets with isolated fictional data.
use super::{find_button, find_named, rendered};
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, AccountSummary, DriveSyncEngine, RestoredAccount};
use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc, time::Duration};

fn settle() {
    let frame = gtk::glib::MainLoop::new(None, false);
    let finish = frame.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(300), move || finish.quit());
    frame.run();
}

fn capture(window: &impl IsA<gtk::Window>, output: &Path, name: &str) {
    let window = window.as_ref();
    let node = rendered(window);
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(output.join(format!("{name}.png")))
        .unwrap();
}

/// Popovers use a separate native surface and must be captured independently.
fn capture_popover(window: &adw::Window, button: &gtk::MenuButton, output: &Path, name: &str) {
    settle();
    let popover = button.popover().unwrap();
    assert!(popover.is_mapped());
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(&popover)).snapshot(
        &snapshot,
        popover.width() as f64,
        popover.height() as f64,
    );
    let node = snapshot.to_node().expect("popover rendered");
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(output.join(format!("{name}.png")))
        .unwrap();
}

fn menu(widget: &gtk::Widget, tooltip: &str) -> Option<gtk::MenuButton> {
    if let Some(button) = widget.downcast_ref::<gtk::MenuButton>() {
        if button.is_mapped() && button.tooltip_text().as_deref() == Some(tooltip) {
            return Some(button.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = menu(&widget, tooltip) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

/// Renders real application widgets; never installs automatic jobs or begins OAuth.
#[test]
#[ignore = "requires a desktop display and an isolated XDG_DATA_HOME"]
fn demonstration_screenshots() {
    let data = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(data.starts_with("/tmp/drivesync-ui-demo-"));
    let output = std::path::PathBuf::from(std::env::var("DRIVESYNC_UI_PREVIEW_DIR").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    std::env::set_var(
        "DRIVESYNC_GOOGLE_CLIENT_ID",
        "screenshots.apps.googleusercontent.com",
    );
    let metadata = Path::new(&data).join("drivesync");
    std::fs::create_dir_all(&metadata).unwrap();
    std::fs::write(metadata.join("accounts.json"), r#"{"version":2,"clients":{"screenshots.apps.googleusercontent.com":[{"id":"google:demo-personal","provider_subject":"demo-personal","email":"marina.pessoal@example.com"},{"id":"google:demo-work","provider_subject":"demo-work","email":"marina.trabalho@example.com"},{"id":"google:demo-study","provider_subject":"demo-study","email":"marina.estudos@example.com"}]},"folders":{}}"#).unwrap();
    adw::init().unwrap();
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
        settings.set_gtk_enable_animations(false);
    }
    let app = adw::Application::builder()
        .application_id("com.rtosta.DriveSync.Screenshots")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let about_parent = adw::ApplicationWindow::builder().application(&app).build();
    crate::main_menu::install(&app, &about_parent);
    let quit = gtk::gio::SimpleAction::new("quit", None);
    let weak_app = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = weak_app.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    let engine = Arc::new(DriveSyncEngine::new());
    for (id, name) in [
        ("personal", "Marina · Pessoal"),
        ("work", "Marina · Trabalho"),
        ("study", "Marina · Estudos"),
    ] {
        engine
            .set_account_description(&AccountId(format!("google:demo-{id}")), name)
            .unwrap();
    }
    for width in [1200, 390] {
        let nav = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&nav));
        let cancellation = Rc::new(RefCell::new(None));
        crate::connection::install_action(engine.clone(), &nav, &toasts, cancellation.clone());
        let welcome = crate::welcome::build(engine.clone(), &nav, &toasts, cancellation.clone());
        let root = adw::NavigationPage::new(&welcome, "Welcome");
        nav.add(&root);
        let window = adw::Window::builder()
            .default_width(width)
            .default_height(900)
            .content(&toasts)
            .build();
        window.set_application(Some(&app));
        window.present();
        settle();
        capture(&window, &output, &format!("01-boas-vindas-{width}"));
        let connection =
            crate::connection::show(engine.clone(), &nav, &toasts, cancellation.clone());
        connection.present();
        capture(&connection, &output, &format!("02-conectar-conta-{width}"));
        connection.close();
        let waiting = crate::connection::waiting_preview(Some(window.upcast_ref()));
        waiting.present();
        capture(
            &waiting,
            &output,
            &format!("03-autorizacao-navegador-{width}"),
        );
        waiting.close();
        let accounts = [
            ("personal", "Fotos e documentos", "Pessoal"),
            ("work", "Projetos da equipe", "Trabalho"),
            ("study", "Materiais de estudo", "Estudos"),
        ]
        .into_iter()
        .map(|(id, remote, local)| {
            let account_id = AccountId(format!("google:demo-{id}"));
            let path = Path::new(&data).join(local);
            if id != "study" {
                std::fs::create_dir_all(&path).unwrap();
            }
            RestoredAccount {
                account: AccountSummary {
                    id: account_id.clone(),
                    provider_subject: format!("demo-{id}"),
                    email: Some(format!(
                        "marina.{}@example.com",
                        match id {
                            "personal" => "pessoal",
                            "work" => "trabalho",
                            _ => "estudos",
                        }
                    )),
                },
                error: if id == "study" {
                    Some(drivesync_engine::AuthError::ReauthorizationRequired)
                } else {
                    None
                },
                folder: Some(drivesync_engine::FolderConfiguration {
                    account_id,
                    remote: drivesync_engine::DriveFolder {
                        id: format!("demo-folder-{id}"),
                        name: remote.into(),
                    },
                    local: path,
                    local_display: format!("/home/marina/DriveSync/{local}").into(),
                }),
            }
        })
        .collect();
        let dashboard = crate::dashboard::build(engine.clone(), accounts, &nav, &toasts);
        nav.push(&dashboard);
        settle();
        let list = find_named(window.upcast_ref(), "account-list")
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let split = find_named(window.upcast_ref(), "dashboard-split")
            .unwrap()
            .downcast::<adw::NavigationSplitView>()
            .unwrap();
        capture(&window, &output, &format!("04-dashboard-pessoal-{width}"));
        if width < 660 {
            split.set_show_content(false);
            capture(
                &window,
                &output,
                &format!("05-lista-multiplas-contas-{width}"),
            );
        }
        list.select_row(list.row_at_index(1).as_ref());
        capture(&window, &output, &format!("06-dashboard-trabalho-{width}"));
        let actions = menu(window.upcast_ref(), "Account actions").unwrap();
        actions.popup();
        capture_popover(
            &window,
            &actions,
            &output,
            &format!("07b-menu-acoes-conta-{width}"),
        );
        capture(&window, &output, &format!("07-acoes-conta-{width}"));
        find_button(window.upcast_ref(), "Sign Out")
            .unwrap()
            .emit_clicked();
        settle();
        let dialog = gtk::Window::list_toplevels()
            .into_iter()
            .find_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .unwrap();
        capture(&dialog, &output, &format!("08-confirmar-sair-{width}"));
        dialog.response("cancel");
        actions.popdown();
        list.select_row(list.row_at_index(2).as_ref());
        capture(
            &window,
            &output,
            &format!("09-conta-reconectar-pasta-indisponivel-{width}"),
        );
        let picker = crate::folder_picker::preview(Some(window.upcast_ref()));
        let folders = find_named(picker.upcast_ref(), "drive-folder-list")
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        for (i, title) in [
            "Documentos pessoais",
            "Fotos de viagens",
            "Projetos da equipe",
            "Apresentações",
            "Planilhas",
            "Materiais de estudo",
            "Relatórios",
            "Arquivos compartilhados",
        ]
        .iter()
        .enumerate()
        {
            folders
                .row_at_index(i as i32)
                .unwrap()
                .downcast::<adw::ActionRow>()
                .unwrap()
                .set_title(title);
        }
        picker.set_default_size(if width > 660 { 640 } else { 390 }, 540);
        picker.present();
        capture(
            &picker,
            &output,
            &format!("10-selecionar-pasta-drive-{width}"),
        );
        picker.close();
        list.select_row(list.row_at_index(0).as_ref());
        let main_menu = menu(window.upcast_ref(), "Main Menu").unwrap();
        main_menu.popup();
        capture_popover(
            &window,
            &main_menu,
            &output,
            &format!("11b-opcoes-menu-principal-{width}"),
        );
        capture(&window, &output, &format!("11-menu-principal-{width}"));
        main_menu.popdown();
        let unconfigured = RestoredAccount {
            account: AccountSummary {
                id: AccountId("google:demo-work".into()),
                provider_subject: "demo-work".into(),
                email: Some("marina.trabalho@example.com".into()),
            },
            error: None,
            folder: None,
        };
        let setup = crate::dashboard::build(engine.clone(), vec![unconfigured], &nav, &toasts);
        nav.push(&setup);
        settle();
        capture(
            &window,
            &output,
            &format!("13-configuracao-inicial-pastas-{width}"),
        );
        window.close();
    }
    crate::dashboard::verify_download_progress(|window, name| capture(window, &output, name));
    app.activate_action("about", None);
    settle();
    let about = gtk::Window::list_toplevels()
        .into_iter()
        .find_map(|w| w.downcast::<adw::AboutWindow>().ok())
        .unwrap();
    capture(&about, &output, "12-sobre-drivesync");
    about.close();
    about_parent.close();
}
