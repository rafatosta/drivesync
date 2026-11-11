//! Grid controller with generation-guarded asynchronous pages and known paths.
use super::{folder_item, model::*, navigation::Navigation, source::*};
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};
/// Widgets own the controller through signals; the controller keeps weak widgets.
pub(super) struct Browser {
    pub root: gtk::Box,
    pub selected_path: gtk::Label,
    state: Rc<State>,
}
struct State {
    source: Rc<dyn RemoteFolderSource>,
    navigation: RefCell<Navigation>,
    selected: RefCell<RemoteFolderLocation>,
    generation: Cell<u64>,
    store: gtk::gio::ListStore,
    selection: gtk::glib::WeakRef<gtk::SingleSelection>,
    breadcrumbs: gtk::glib::WeakRef<gtk::Box>,
    status: gtk::glib::WeakRef<gtk::Label>,
    spinner: gtk::glib::WeakRef<gtk::Spinner>,
    back: gtk::glib::WeakRef<gtk::Button>,
    forward: gtk::glib::WeakRef<gtk::Button>,
    selected_path: gtk::glib::WeakRef<gtk::Label>,
    retry: gtk::glib::WeakRef<gtk::Button>,
    feedback: gtk::glib::WeakRef<gtk::Box>,
}
impl Browser {
    pub fn location(&self) -> RemoteFolderLocation {
        self.state.selected.borrow().clone()
    }
    pub fn new(root_folder: RemoteFolder, source: Rc<dyn RemoteFolderSource>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_vexpand(true);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.set_tooltip_text(Some(&gettext("Back")));
        let forward = gtk::Button::from_icon_name("go-next-symbolic");
        forward.set_tooltip_text(Some(&gettext("Forward")));
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some(&gettext("Refresh")));
        let breadcrumbs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        breadcrumbs.add_css_class("remote-path");
        let path_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&breadcrumbs)
            .build();
        bar.append(&back);
        bar.append(&forward);
        bar.append(&path_scroll);
        bar.append(&refresh);
        root.append(&bar);
        let feedback = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let spinner = gtk::Spinner::new();
        let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
        let retry = gtk::Button::with_label(&gettext("Try again"));
        retry.set_visible(false);
        feedback.append(&spinner);
        feedback.append(&status);
        feedback.append(&retry);
        root.append(&feedback);
        let store = gtk::gio::ListStore::new::<gtk::glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let grid = gtk::GridView::new(Some(selection.clone()), Some(folder_item::factory()));
        grid.set_min_columns(1);
        grid.set_max_columns(4);
        grid.set_single_click_activate(false);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&grid)
            .build();
        root.append(&scroll);
        let selected_path = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .build();
        let location = RemoteFolderLocation {
            folder: root_folder.clone(),
            path: vec![root_folder.clone()],
        };
        let mut navigation = Navigation::default();
        navigation.visit(vec![root_folder]);
        let state = Rc::new(State {
            source,
            navigation: RefCell::new(navigation),
            selected: RefCell::new(location),
            generation: Cell::new(0),
            store,
            selection: selection.downgrade(),
            breadcrumbs: breadcrumbs.downgrade(),
            status: status.downgrade(),
            spinner: spinner.downgrade(),
            back: back.downgrade(),
            forward: forward.downgrade(),
            selected_path: selected_path.downgrade(),
            retry: retry.downgrade(),
            feedback: feedback.downgrade(),
        });
        selection.connect_selected_item_notify({
            let state = state.clone();
            move |selection| {
                if let Some(item) = selection.selected_item() {
                    let item = item.downcast::<gtk::glib::BoxedAnyObject>().unwrap();
                    let folder = item.borrow::<RemoteFolder>().clone();
                    let mut path = state.navigation.borrow().path();
                    path.push(folder.clone());
                    state.select(RemoteFolderLocation { folder, path });
                }
            }
        });
        grid.connect_activate({
            let state = state.clone();
            move |_, index| {
                if let Some(item) = state.store.item(index) {
                    let item = item.downcast::<gtk::glib::BoxedAnyObject>().unwrap();
                    let mut path = state.navigation.borrow().path();
                    path.push(item.borrow::<RemoteFolder>().clone());
                    state.visit(path);
                }
            }
        });
        for (button, ahead) in [(back, false), (forward, true)] {
            button.connect_clicked({
                let state = state.clone();
                move |_| {
                    state.navigation.borrow_mut().step(ahead);
                    state.load();
                }
            });
        }
        for button in [refresh, retry] {
            button.connect_clicked({
                let state = state.clone();
                move |_| state.load()
            });
        }
        // Closing/destroying the browser invalidates all outstanding page responses.
        root.connect_unrealize({
            let state = state.clone();
            move |_| {
                state.generation.set(state.generation.get() + 1);
            }
        });
        state.load();
        selected_path.set_tooltip_text(Some(&gettext(
            "Select current folder using the last breadcrumb",
        )));
        Self {
            root,
            selected_path,
            state,
        }
    }
}
impl State {
    fn select(&self, location: RemoteFolderLocation) {
        if let Some(label) = self.selected_path.upgrade() {
            let last = location.path.len().saturating_sub(1);
            let path = location
                .path
                .iter()
                .enumerate()
                .map(|(index, folder)| {
                    let name = gtk::glib::markup_escape_text(&folder.name);
                    if index == last {
                        format!("<b>{name}</b>")
                    } else {
                        name.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" / ");
            label.set_markup(&path);
        }
        *self.selected.borrow_mut() = location;
    }
    fn visit(self: &Rc<Self>, path: Vec<RemoteFolder>) {
        self.navigation.borrow_mut().visit(path);
        self.load();
    }
    fn load(self: &Rc<Self>) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.store.remove_all();
        let path = self.navigation.borrow().path();
        self.select(RemoteFolderLocation {
            folder: path.last().unwrap().clone(),
            path: path.clone(),
        });
        if let Some(bar) = self.breadcrumbs.upgrade() {
            while let Some(child) = bar.first_child() {
                bar.remove(&child);
            }
            for (index, folder) in path.iter().enumerate() {
                if index > 0 {
                    bar.append(&gtk::Image::from_icon_name("go-next-symbolic"));
                }
                let button = gtk::Button::with_label(&folder.name);
                button.add_css_class("flat");
                if index + 1 == path.len() {
                    button.add_css_class("heading");
                }
                button.connect_clicked({
                    let state = Rc::downgrade(self);
                    let path = path[..=index].to_vec();
                    move |_| {
                        if let Some(state) = state.upgrade() {
                            state.visit(path.clone());
                        }
                    }
                });
                bar.append(&button);
            }
        }
        let nav = self.navigation.borrow();
        if let Some(button) = self.back.upgrade() {
            button.set_sensitive(nav.cursor > 0);
        }
        if let Some(button) = self.forward.upgrade() {
            button.set_sensitive(nav.cursor + 1 < nav.history.len());
        }
        drop(nav);
        if let Some(feedback) = self.feedback.upgrade() {
            feedback.set_visible(true);
        }
        if let Some(spinner) = self.spinner.upgrade() {
            spinner.set_visible(true);
            spinner.start();
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label(&gettext("Loading folders…"));
        }
        if let Some(retry) = self.retry.upgrade() {
            retry.set_visible(false);
        }
        let weak = Rc::downgrade(self);
        let source = self.source.clone();
        let parent = path.last().unwrap().id.clone();
        gtk::glib::spawn_future_local(async move {
            let mut token: Option<String> = None;
            let mut seen = HashSet::new();
            let mut tokens = HashSet::new();
            loop {
                let result = source.list_folders(&parent, token.as_deref()).await;
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state.generation.get() != generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        for folder in page.folders {
                            if folder.parent_id.as_ref() == Some(&parent)
                                && seen.insert(folder.id.clone())
                            {
                                state.store.append(&gtk::glib::BoxedAnyObject::new(folder));
                            }
                        }
                        if let Some(next) = page.next_page_token {
                            if tokens.insert(next.clone()) {
                                token = Some(next);
                                drop(state);
                                gtk::glib::timeout_future(std::time::Duration::from_millis(1))
                                    .await;
                                continue;
                            }
                            state.finish(Some(FolderSourceError::Other(gettext(
                                "Repeated page token",
                            ))));
                            return;
                        }
                        state.finish(None);
                        return;
                    }
                    Err(error) => {
                        state.finish(Some(error));
                        return;
                    }
                }
            }
        });
    }
    fn finish(&self, error: Option<FolderSourceError>) {
        if let Some(feedback) = self.feedback.upgrade() {
            feedback.set_visible(error.is_some() || self.store.n_items() == 0);
        }
        if let Some(spinner) = self.spinner.upgrade() {
            spinner.stop();
            spinner.set_visible(false);
        }
        if let Some(retry) = self.retry.upgrade() {
            retry.set_visible(error.is_some());
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label(&match error {
                Some(FolderSourceError::Unauthorized) => {
                    gettext("Access to this folder is not authorized.")
                }
                Some(FolderSourceError::NotFound) => gettext("Folder not found."),
                Some(_) => gettext("Unable to load folders. Try again."),
                None if self.store.n_items() == 0 => gettext("This folder has no subfolders."),
                None => String::new(),
            });
        }
    }
}

/// Exercises navigation while earlier queries are still outstanding on GTK's thread.
#[cfg(test)]
pub(super) async fn exercise_browser() {
    let mut source = super::MockFolderSource::default();
    source.delay = std::time::Duration::from_millis(10);
    source.page_size = 7;
    let root = source.root();
    let projects = source.list_folders(&root.id, None).await.unwrap().folders[1].clone();
    let drive = source
        .list_folders(&projects.id, None)
        .await
        .unwrap()
        .folders[0]
        .clone();
    let browser = Browser::new(root.clone(), Rc::new(source));
    browser
        .state
        .visit(vec![root.clone(), projects.clone(), drive]);
    gtk::glib::timeout_future(std::time::Duration::from_millis(80)).await;
    assert_eq!(browser.state.store.n_items(), 7);
    assert_eq!(browser.location().folder.name, "DriveSync");
    let first = browser
        .state
        .store
        .item(0)
        .unwrap()
        .downcast::<gtk::glib::BoxedAnyObject>()
        .unwrap();
    let first = first.borrow::<RemoteFolder>().clone();
    browser.state.selection.upgrade().unwrap().set_selected(0);
    assert_eq!(browser.location().folder, first);
    let mut path = browser.state.navigation.borrow().path();
    path.push(first);
    browser.state.visit(path);
    gtk::glib::timeout_future(std::time::Duration::from_millis(30)).await;
    assert_eq!(browser.state.store.n_items(), 0);
    assert_eq!(
        browser.state.status.upgrade().unwrap().label(),
        gettext("This folder has no subfolders.")
    );
    browser.state.navigation.borrow_mut().step(false);
    browser.state.load();
    gtk::glib::timeout_future(std::time::Duration::from_millis(30)).await;
    assert_eq!(browser.state.store.n_items(), 7);
    for (id, expected) in [("Erro de consulta", 0), ("Muitas pastas", 80)] {
        browser.state.visit(vec![
            root.clone(),
            RemoteFolder {
                id: RemoteFolderId(id.into()),
                name: id.into(),
                parent_id: Some(root.id.clone()),
            },
        ]);
        gtk::glib::timeout_future(std::time::Duration::from_millis(300)).await;
        assert_eq!(browser.state.store.n_items(), expected);
        assert_eq!(
            browser.state.retry.upgrade().unwrap().is_visible(),
            expected == 0
        );
    }
    browser.state.visit(vec![root.clone()]);
    assert_eq!(browser.location().folder, root);
}
