//! Account editors shared with their dashboard adapters. Editors have one owner
//! and stay mounted in Preferences so navigation preserves incomplete drafts.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, RestoredAccount};
use gettextrs::gettext;
use std::{cell::Cell, rc::Rc};

/// One account's editor, retained while navigating the dashboard.
pub(crate) struct AccountSettings {
    saved: RestoredAccount,
    editor: gtk::Box,
}
impl AccountSettings {
    pub(crate) fn new(saved: RestoredAccount, editor: gtk::Box) -> Self {
        Self { saved, editor }
    }
}

/// Read-only account membership and weak navigation controls for one dashboard.
pub(crate) struct Accounts {
    records: Vec<AccountSettings>,
    selector: gtk::glib::WeakRef<adw::ComboRow>,
    sections: gtk::glib::WeakRef<gtk::ListBox>,
    split: gtk::glib::WeakRef<adw::NavigationSplitView>,
    search: gtk::glib::WeakRef<gtk::SearchEntry>,
    selected: Cell<usize>,
}
impl Accounts {
    pub(crate) fn new(records: Vec<AccountSettings>) -> Rc<Self> {
        Rc::new(Self {
            records,
            selector: Default::default(),
            sections: Default::default(),
            split: Default::default(),
            search: Default::default(),
            selected: Cell::new(0),
        })
    }
    pub(crate) fn id_at(&self, index: usize) -> Option<&AccountId> {
        self.records.get(index).map(|r| &r.saved.account.id)
    }
    pub(super) fn select(&self, id: Option<&AccountId>, show_accounts: bool) {
        if let Some(index) =
            id.and_then(|id| self.records.iter().position(|r| &r.saved.account.id == id))
        {
            self.selected.set(index);
            if let Some(selector) = self.selector.upgrade() {
                selector.set_selected(index as u32);
            }
        }
        if show_accounts {
            if let Some(search) = self.search.upgrade() {
                search.set_text("");
                search.emit_by_name::<()>("search-changed", &[]);
            }
            if let Some(sections) = self.sections.upgrade() {
                sections.select_row(sections.row_at_index(1).as_ref());
            }
            if let Some(split) = self.split.upgrade() {
                split.set_show_content(true);
            }
        }
    }
    pub(super) fn bind_navigation(
        &self,
        sections: &gtk::ListBox,
        split: &adw::NavigationSplitView,
        search: &gtk::SearchEntry,
    ) {
        self.search.set(Some(search));
        self.sections.set(Some(sections));
        self.split.set(Some(split));
    }
    pub(super) fn page(self: &Rc<Self>) -> gtk::ScrolledWindow {
        let page = crate::ui::content();
        page.set_widget_name("preferences-accounts");
        let identity = adw::PreferencesGroup::builder()
            .title(gettext("Accounts"))
            .description(gettext("Settings apply only to the account shown below."))
            .build();
        if self.records.is_empty() {
            identity.add(
                &adw::ActionRow::builder()
                    .title(gettext("No connected accounts"))
                    .subtitle(gettext(
                        "Connect an account from the main window to configure it here.",
                    ))
                    .build(),
            );
            page.append(&identity);
            return crate::ui::scroll(&page, 760);
        }
        let labels: Vec<_> = self
            .records
            .iter()
            .map(|r| {
                r.saved
                    .account
                    .email
                    .as_deref()
                    .unwrap_or(&r.saved.account.id.0)
            })
            .collect();
        let expression = gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        );
        let selector = adw::ComboRow::builder()
            .title(gettext("Account"))
            .expression(expression)
            .use_subtitle(true)
            .build();
        selector.set_widget_name("preferences-account-selector");
        selector.set_use_markup(false);
        selector.set_model(Some(&gtk::StringList::new(&labels)));
        selector.set_selected(self.selected.get() as u32);
        if labels.len() > 1 {
            identity.add(&selector);
        } else {
            identity.add(
                &adw::ActionRow::builder()
                    .title(gettext("Account"))
                    .subtitle(labels[0])
                    .use_markup(false)
                    .build(),
            );
        }
        self.selector.set(Some(&selector));
        let editors = gtk::Stack::new();
        editors.set_hhomogeneous(false);
        editors.set_vhomogeneous(false);
        editors.set_widget_name("preferences-account-editors");
        for record in &self.records {
            editors.add_named(&record.editor, Some(&record.saved.account.id.0));
        }
        if let Some(record) = self.records.get(self.selected.get()) {
            editors.set_visible_child_name(&record.saved.account.id.0);
        }
        let weak_editors = editors.downgrade();
        let context = Rc::downgrade(self);
        selector.connect_selected_notify(move |selector| {
            let Some(context) = context.upgrade() else {
                return;
            };
            let index = selector.selected() as usize;
            if let (Some(editors), Some(id)) = (weak_editors.upgrade(), context.id_at(index)) {
                context.selected.set(index);
                editors.set_visible_child_name(&id.0);
            }
        });
        page.append(&identity);
        page.append(&editors);
        crate::ui::scroll(&page, 760)
    }
}
