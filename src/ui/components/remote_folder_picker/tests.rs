//! Behavioral checks use no network, accounts or persistent application state.
use super::*;
use adw::{gtk, prelude::*};
use std::{cell::RefCell, rc::Rc, time::Duration};
#[test]
fn mock_pages_empty_errors_and_opaque_tokens() {
    let context = gtk::glib::MainContext::new();
    context.block_on(async {
        let mut source = MockFolderSource::default();
        source.delay = Duration::ZERO;
        source.page_size = 7;
        let root = source.root();
        assert_eq!(
            source
                .list_folders(&root.id, None)
                .await
                .unwrap()
                .folders
                .len(),
            4
        );
        assert!(source
            .list_folders(&RemoteFolderId("Fotos".into()), None)
            .await
            .unwrap()
            .folders
            .is_empty());
        assert_eq!(
            source
                .list_folders(&RemoteFolderId("Erro de consulta".into()), None)
                .await
                .unwrap_err(),
            FolderSourceError::Unavailable
        );
        assert_eq!(
            source
                .list_folders(&RemoteFolderId("missing".into()), None)
                .await
                .unwrap_err(),
            FolderSourceError::NotFound
        );
        let mut token = None;
        let mut ids = std::collections::HashSet::new();
        loop {
            let page = source
                .list_folders(&RemoteFolderId("Muitas pastas".into()), token.as_deref())
                .await
                .unwrap();
            for folder in page.folders {
                assert!(ids.insert(folder.id));
            }
            token = page.next_page_token;
            if token.is_none() {
                break;
            }
        }
        assert_eq!(ids.len(), 80);
        assert!(source
            .list_folders(&root.id, Some("invalid"))
            .await
            .is_err());
    });
}
/// Opt-in because GTK requires a desktop display and its initialization thread.
#[test]
#[ignore = "requires a desktop display"]
fn native_picker_selection_cancel_and_preview() {
    if let Ok(directory) = std::env::var("DRIVESYNC_PICKER_LOCALE") {
        unsafe {
            gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
        }
        gettextrs::bindtextdomain("drivesync", directory).unwrap();
        gettextrs::textdomain("drivesync").unwrap();
    }
    adw::init().unwrap();
    adw::StyleManager::default().set_color_scheme(
        if std::env::var_os("DRIVESYNC_PICKER_DARK").is_some() {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        },
    );
    gtk::Settings::default()
        .unwrap()
        .set_gtk_icon_theme_name(Some("Adwaita"));
    gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap())
        .add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons"));
    gtk::glib::MainContext::default().block_on(super::browser::exercise_browser());
    let source = MockFolderSource::default();
    let root = source.root();
    let picker = RemoteFolderPicker::new(
        RemoteFolderPickerConfig {
            provider_name: "Google Drive".into(),
            provider_icon: "google-drive".into(),
            account_label: "conta@example.com".into(),
            root: root.clone(),
        },
        source,
    );
    let responses = Rc::new(RefCell::new(Vec::new()));
    picker.connect_response({
        let responses = responses.clone();
        move |value| responses.borrow_mut().push(value)
    });
    let app = adw::Application::builder()
        .application_id("com.rtosta.PickerTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let parent = adw::ApplicationWindow::new(&app);
    // The isolated application never activates the real account-management app.
    picker.present(&parent);
    let context = gtk::glib::MainContext::default();
    context.block_on(gtk::glib::timeout_future(Duration::from_millis(500)));
    let window = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<adw::Window>().ok())
        .find(|w| w.has_css_class("remote-folder-picker"))
        .unwrap();
    fn find_grid(widget: &gtk::Widget) -> Option<gtk::GridView> {
        if let Ok(grid) = widget.clone().downcast::<gtk::GridView>() {
            return Some(grid);
        }
        let mut child = widget.first_child();
        while let Some(value) = child {
            if let Some(grid) = find_grid(&value) {
                return Some(grid);
            }
            child = value.next_sibling();
        }
        None
    }
    let grid = find_grid(window.upcast_ref()).unwrap();
    grid.emit_by_name::<()>("activate", &[&1u32]);
    context.block_on(gtk::glib::timeout_future(Duration::from_millis(250)));
    grid.emit_by_name::<()>("activate", &[&0u32]);
    context.block_on(gtk::glib::timeout_future(Duration::from_millis(250)));
    let selection = grid
        .model()
        .unwrap()
        .downcast::<gtk::SingleSelection>()
        .unwrap();
    assert_eq!(selection.n_items(), 7);
    selection.set_selected(0);
    context.block_on(gtk::glib::timeout_future(Duration::from_millis(100)));
    if let Some(path) = std::env::var_os("DRIVESYNC_PICKER_PREVIEW") {
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
        let node = snapshot.to_node().unwrap();
        let renderer = window.renderer().unwrap();
        renderer
            .render_texture(&node, None)
            .save_to_png(path)
            .unwrap();
    }
    fn find_button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
        if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
            if button.label().as_deref() == Some(label) {
                return Some(button);
            }
        }
        let mut child = widget.first_child();
        while let Some(value) = child {
            if let Some(button) = find_button(&value, label) {
                return Some(button);
            }
            child = value.next_sibling();
        }
        None
    }
    find_button(window.upcast_ref(), &gettextrs::gettext("Select folder"))
        .unwrap()
        .emit_clicked();
    let values = responses.borrow();
    let [RemoteFolderPickerResult::Selected(location)] = values.as_slice() else {
        panic!("expected exactly one selection");
    };
    assert_eq!(location.folder.name, "Código-fonte");
    assert_eq!(
        location
            .path
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["Meu Drive", "Projetos", "DriveSync", "Código-fonte"]
    );
    drop(values);
    let source = MockFolderSource::default();
    let cancelled = RemoteFolderPicker::new(
        RemoteFolderPickerConfig {
            provider_name: "Mock".into(),
            provider_icon: "folder".into(),
            account_label: "mock".into(),
            root: source.root(),
        },
        source,
    );
    let cancellations = Rc::new(RefCell::new(Vec::new()));
    cancelled.connect_response({
        let values = cancellations.clone();
        move |value| values.borrow_mut().push(value)
    });
    cancelled.present(&parent);
    let window = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<adw::Window>().ok())
        .find(|w| w.has_css_class("remote-folder-picker") && w.is_visible())
        .unwrap();
    window.close();
    assert_eq!(
        *cancellations.borrow(),
        vec![RemoteFolderPickerResult::Cancelled]
    );
}
