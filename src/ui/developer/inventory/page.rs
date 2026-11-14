//! Read-only table browser with bounded asynchronous reads and stale-response protection.
use crate::application::{
    inventory::{self, ConfiguredInventory},
    Context,
};
use adw::{gtk, prelude::*};
use drivesync_engine::inventory::{
    inspection::{self, CellKind, InspectionCell, InspectionPage, InventoryTable},
    InventoryError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct State {
    scopes: Vec<ConfiguredInventory>,
    columns: Vec<String>,
    /// Previous exclusive row keys; a page is a live diagnostic read, not a historical snapshot.
    history: Vec<i64>,
    after: i64,
    next: Option<i64>,
    loading: bool,
}
struct View {
    root: gtk::glib::WeakRef<gtk::Box>,
    context: Context,
    accounts: gtk::DropDown,
    tables: gtk::DropDown,
    refresh: gtk::Button,
    previous: gtk::Button,
    next: gtk::Button,
    status: gtk::Label,
    location: gtk::Label,
    range: gtk::Label,
    spinner: gtk::Spinner,
    grid: gtk::ColumnView,
    store: gtk::gio::ListStore,
    selection: gtk::SingleSelection,
    detail: gtk::TextView,
    state: RefCell<State>,
    generation: Cell<u64>,
    loading_accounts: Cell<bool>,
}
fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build()
}
fn kind(kind: CellKind) -> &'static str {
    match kind {
        CellKind::Null => "NULL",
        CellKind::Integer => "INTEGER",
        CellKind::Real => "REAL",
        CellKind::Text => "TEXT",
        CellKind::BlobText => "BLOB / UTF-8",
        CellKind::BlobHex => "BLOB / hex",
    }
}
/// GTK strings are NUL-terminated; preserve embedded NULs as an explicit visible escape.
fn gtk_text(value: &str) -> String {
    value.replace('\0', "\\0")
}
fn detail_text(columns: &[String], row: &[InspectionCell]) -> String {
    columns
        .iter()
        .zip(row)
        .map(|(name, cell)| {
            format!(
                "{name} [{}{}]\n{}\n",
                kind(cell.kind),
                if cell.truncated {
                    format!(" · preview truncated · {} bytes total", cell.byte_length)
                } else if matches!(
                    cell.kind,
                    CellKind::Text | CellKind::BlobText | CellKind::BlobHex
                ) {
                    format!(" · {} bytes", cell.byte_length)
                } else {
                    String::new()
                },
                gtk_text(&cell.display)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn error_message(error: InventoryError) -> &'static str {
    match error {
        InventoryError::NotFound=>"Inventory database has not been created for this folder. Inspection does not create it.",
        InventoryError::UnsupportedSchema=>"This inventory uses an unsupported schema. It has been preserved without conversion.",
        InventoryError::InvalidIdentity=>"The inventory does not belong to the selected account and configured folder.",
        InventoryError::CorruptDatabase=>"The inventory could not be read because its data is invalid. The database has been preserved.",
        InventoryError::PermissionDenied=>"Access to this inventory is not permitted.",
        _=>"The inventory is unavailable or busy. Check access to the configured folder and try Refresh.",
    }
}
impl View {
    fn controls(&self) {
        let state = self.state.borrow();
        let busy = state.loading || self.loading_accounts.get();
        self.refresh.set_sensitive(!busy);
        self.accounts
            .set_sensitive(!self.loading_accounts.get() && !state.scopes.is_empty());
        self.tables
            .set_sensitive(!self.loading_accounts.get() && !state.scopes.is_empty());
        self.previous
            .set_sensitive(!busy && !state.history.is_empty());
        self.next.set_sensitive(!busy && state.next.is_some());
        self.spinner.set_spinning(busy);
    }
    fn clear(&self) {
        self.store.remove_all();
        self.detail
            .buffer()
            .set_text("Select a record to inspect its column values.");
        self.range.set_label("");
        self.state.borrow_mut().columns.clear();
        while let Some(column) = self
            .grid
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
        {
            self.grid.remove_column(&column);
        }
    }
    fn reset(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.history.clear();
        state.after = 0;
        state.next = None;
        drop(state);
        self.load();
    }
    fn render(&self, page: InspectionPage) {
        self.clear();
        for (index, name) in page.columns.iter().enumerate() {
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, object| {
                let item = object
                    .downcast_ref::<gtk::ListItem>()
                    .expect("column list item");
                let text = gtk::Label::builder()
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .width_chars(16)
                    .max_width_chars(36)
                    .margin_start(8)
                    .margin_end(8)
                    .margin_top(8)
                    .margin_bottom(8)
                    .css_classes(["monospace"])
                    .build();
                item.set_child(Some(&text));
            });
            factory.connect_bind(move |_, object| {
                let item = object
                    .downcast_ref::<gtk::ListItem>()
                    .expect("column list item");
                let text = item
                    .child()
                    .and_downcast::<gtk::Label>()
                    .expect("cell label");
                let object = item
                    .item()
                    .and_downcast::<gtk::glib::BoxedAnyObject>()
                    .expect("row values");
                let row = object.borrow::<Vec<InspectionCell>>();
                let cell = &row[index];
                let shown = gtk_text(&cell.display)
                    .replace('\n', " ↵ ")
                    .replace('\t', " ⇥ ");
                text.set_label(&shown);
                text.set_tooltip_text(Some(&format!(
                    "{}{}\n{}",
                    kind(cell.kind),
                    if cell.truncated {
                        " · truncated preview"
                    } else {
                        ""
                    },
                    gtk_text(&cell.display)
                )));
                if cell.kind == CellKind::Null {
                    text.add_css_class("dim-label");
                } else {
                    text.remove_css_class("dim-label");
                }
            });
            let column = gtk::ColumnViewColumn::new(Some(name), Some(factory));
            column.set_resizable(true);
            self.grid.append_column(&column);
        }
        let count = page.rows.len();
        {
            let mut state = self.state.borrow_mut();
            state.columns = page.columns;
            state.next = page.next_after;
        }
        for row in page.rows {
            self.store.append(&gtk::glib::BoxedAnyObject::new(row));
        }
        self.selection.set_selected(if count == 0 {
            gtk::INVALID_LIST_POSITION
        } else {
            0
        });
        self.status.set_label(if count == 0 {
            "This table is empty."
        } else {
            "Select a record to read its values below. Scroll horizontally for more columns."
        });
        let number = self.state.borrow().history.len() + 1;
        self.range
            .set_label(&format!("Page {number} · {count} records"));
    }
    fn load(self: &Rc<Self>) {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.clear();
        self.state.borrow_mut().next = None;
        let scope = self
            .state
            .borrow()
            .scopes
            .get(self.accounts.selected() as usize)
            .cloned();
        let Some(scope) = scope else {
            self.location.set_label("");
            self.status.set_label(
                "No configured accounts. Complete account setup to inspect an inventory.",
            );
            self.state.borrow_mut().loading = false;
            self.controls();
            return;
        };
        let mut request = match scope.request {
            Ok(request) => request,
            Err(error) => {
                self.status.set_label(&error);
                self.location.set_label("");
                self.state.borrow_mut().loading = false;
                self.controls();
                return;
            }
        };
        request.table = InventoryTable::ALL
            .get(self.tables.selected() as usize)
            .copied()
            .unwrap_or(InventoryTable::Items);
        request.after_rowid = self.state.borrow().after;
        self.location.set_label(
            &request
                .local_root
                .join(".drivesync/inventory.sqlite3")
                .display()
                .to_string(),
        );
        self.status.set_label("Reading inventory…");
        self.state.borrow_mut().loading = true;
        self.controls();
        let weak = Rc::downgrade(self);
        gtk::glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || inspection::inspect(request)).await;
            let Some(view) = weak
                .upgrade()
                .filter(|v| v.root.upgrade().is_some() && v.generation.get() == generation)
            else {
                return;
            };
            view.state.borrow_mut().loading = false;
            match result {
                Ok(Ok(page)) => view.render(page),
                Ok(Err(error)) => view.status.set_label(error_message(error.kind)),
                Err(_) => view
                    .status
                    .set_label("The inventory could not be read. Try Refresh."),
            }
            view.controls();
        });
    }
    fn reload(self: &Rc<Self>) {
        if self.loading_accounts.replace(true) {
            return;
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        self.clear();
        self.location.set_label("");
        {
            let mut state = self.state.borrow_mut();
            state.loading = false;
            state.history.clear();
            state.after = 0;
            state.next = None;
        }
        self.controls();
        self.status.set_label("Loading configured accounts…");
        let weak = Rc::downgrade(self);
        let context = self.context.clone();
        gtk::glib::spawn_future_local(async move {
            let result = inventory::configured(&context).await;
            let Some(view) = weak.upgrade().filter(|v| v.root.upgrade().is_some()) else {
                return;
            };
            let selected = view
                .state
                .borrow()
                .scopes
                .get(view.accounts.selected() as usize)
                .map(|s| s.account_id.clone());
            // Ignore selection notifications until both model and scope mapping are replaced.
            match result {
                Ok(scopes) => {
                    let strings = scopes.iter().map(|s| s.label.as_str()).collect::<Vec<_>>();
                    let model = gtk::StringList::new(&strings);
                    let index = selected
                        .and_then(|id| scopes.iter().position(|s| s.account_id == id))
                        .unwrap_or(0);
                    let empty = scopes.is_empty();
                    view.state.borrow_mut().scopes = scopes;
                    view.accounts.set_model(Some(&model));
                    view.accounts.set_selected(if empty {
                        gtk::INVALID_LIST_POSITION
                    } else {
                        index as u32
                    });
                    view.loading_accounts.set(false);
                    view.reset();
                }
                Err(error) => {
                    view.state.borrow_mut().scopes.clear();
                    view.accounts.set_model(Some(&gtk::StringList::new(&[])));
                    view.loading_accounts.set(false);
                    view.status.set_label(&error);
                    view.controls();
                }
            }
        });
    }
}
/// Builds the inventory destination; account discovery begins only when the page is mapped.
pub(in crate::ui::developer) fn build(context: &Context) -> gtk::Box {
    let (root, view) = create(context);
    // The root owns the view. Child handlers and background reads hold only weak references.
    root.connect_map(move |_| view.reload());
    root
}
fn create(context: &Context) -> (gtk::Box, Rc<View>) {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_hexpand(true);
    let account_factory = gtk::SignalListItemFactory::new();
    account_factory.connect_setup(|_, item| {
        let item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("account list item");
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .hexpand(true)
                .build(),
        ));
    });
    account_factory.connect_bind(|_, item| {
        let item = item
            .downcast_ref::<gtk::ListItem>()
            .expect("account list item");
        let text = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .expect("account name");
        item.child()
            .and_downcast::<gtk::Label>()
            .expect("account label")
            .set_label(&text.string());
    });
    accounts.set_factory(Some(&account_factory));
    accounts.set_widget_name("inventory-account");
    accounts.set_tooltip_text(Some("Account whose inventory is being inspected"));
    accounts.update_property(&[gtk::accessible::Property::Label("Account")]);
    let refresh = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Reload accounts and current table")
        .build();
    refresh.set_widget_name("inventory-refresh");
    let account_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    account_row.append(&accounts);
    account_row.append(&refresh);
    root.append(&account_row);
    let names = InventoryTable::ALL
        .iter()
        .map(|t| t.name())
        .collect::<Vec<_>>();
    let tables = gtk::DropDown::from_strings(&names);
    tables.set_selected(1);
    tables.set_widget_name("inventory-table");
    tables.set_hexpand(true);
    tables.update_property(&[gtk::accessible::Property::Label("Inventory table")]);
    let spinner = gtk::Spinner::new();
    let table_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    table_row.append(&label("Table"));
    table_row.append(&tables);
    table_row.append(&spinner);
    root.append(&table_row);
    let location = label("");
    location.add_css_class("dim-label");
    location.set_selectable(true);
    root.append(&location);
    let status = label("Select a configured account to inspect its inventory.");
    status.set_widget_name("inventory-status");
    root.append(&status);
    let store = gtk::gio::ListStore::new::<gtk::glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(true);
    let grid = gtk::ColumnView::new(Some(selection.clone()));
    grid.set_show_row_separators(true);
    grid.set_show_column_separators(true);
    grid.set_widget_name("inventory-grid");
    let scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(180)
        .child(&grid)
        .has_frame(true)
        .build();
    root.append(&scroll);
    let previous = gtk::Button::with_label("Previous");
    previous.set_widget_name("inventory-previous");
    let next = gtk::Button::with_label("Next");
    next.set_widget_name("inventory-next");
    let range = label("");
    range.set_hexpand(true);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.append(&range);
    footer.append(&previous);
    footer.append(&next);
    root.append(&footer);
    let detail = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(12)
        .right_margin(12)
        .top_margin(12)
        .bottom_margin(12)
        .build();
    detail.set_widget_name("inventory-record");
    detail
        .buffer()
        .set_text("Select a record to inspect its column values.");
    let detail_scroll = gtk::ScrolledWindow::builder()
        .min_content_height(160)
        .max_content_height(220)
        .propagate_natural_height(true)
        .child(&detail)
        .has_frame(true)
        .build();
    let expander = gtk::Expander::builder()
        .label("Selected record")
        .child(&detail_scroll)
        .build();
    root.append(&expander);
    let note=label("Read-only inspection · Up to 100 records per page · Long values show a bounded preview · Data is populated by inventory consumers.");
    note.add_css_class("dim-label");
    root.append(&note);
    let view = Rc::new(View {
        root: root.downgrade(),
        context: context.clone(),
        accounts: accounts.clone(),
        tables: tables.clone(),
        refresh: refresh.clone(),
        previous: previous.clone(),
        next: next.clone(),
        status,
        location,
        range,
        spinner,
        grid: grid.clone(),
        store,
        selection: selection.clone(),
        detail,
        state: RefCell::new(State::default()),
        generation: Cell::new(0),
        loading_accounts: Cell::new(false),
    });
    view.controls();
    accounts.connect_selected_notify({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                if !view.loading_accounts.get() {
                    view.reset();
                }
            }
        }
    });
    tables.connect_selected_notify({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                if !view.loading_accounts.get() {
                    view.reset();
                }
            }
        }
    });
    refresh.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                view.reload();
            }
        }
    });
    previous.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let after = view.state.borrow_mut().history.pop();
                if let Some(after) = after {
                    view.state.borrow_mut().after = after;
                    view.load();
                }
            }
        }
    });
    next.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let mut state = view.state.borrow_mut();
                if let Some(next) = state.next {
                    let old = state.after;
                    state.history.push(old);
                    state.after = next;
                    drop(state);
                    view.load();
                }
            }
        }
    });
    selection.connect_selected_notify({
        let weak = Rc::downgrade(&view);
        move |selection| {
            if let Some(view) = weak.upgrade() {
                if let Some(object) = selection
                    .selected_item()
                    .and_downcast::<gtk::glib::BoxedAnyObject>()
                {
                    view.detail.buffer().set_text(&detail_text(
                        &view.state.borrow().columns,
                        &object.borrow::<Vec<InspectionCell>>(),
                    ));
                }
            }
        }
    });
    grid.connect_activate(move |_, _| expander.set_expanded(true));
    (root, view)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
