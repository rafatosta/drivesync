//! Persistent activity timeline and account filter.
mod details;
mod rows;

use adw::{gtk, prelude::*};
use drivesync_engine::{
    AccountId, ActivityPage, DriveSyncEngine, LocalActivity, LocalBatch, RestoredAccount,
};
use gettextrs::gettext;
use std::{cell::Cell, rc::Rc, sync::Arc};

pub(in crate::features::dashboard) const PAGE: &str = "activities";

pub(in crate::features::dashboard) struct Activities {
    pub root: adw::BreakpointBin,
    controller: Rc<Timeline>,
}

impl Activities {
    pub fn new(engine: Arc<DriveSyncEngine>, accounts: &[RestoredAccount]) -> Self {
        let root = adw::BreakpointBin::new();
        root.set_widget_name("activities-page");
        let split = adw::NavigationSplitView::builder()
            .min_sidebar_width(280.0)
            .max_sidebar_width(540.0)
            .sidebar_width_fraction(0.56)
            .build();
        let body = crate::ui::content();
        let (detail_body, details) = details::Details::new(&split);
        split.set_sidebar(Some(&adw::NavigationPage::new(
            &crate::ui::scroll(&body, 760),
            &gettext("Activities"),
        )));
        split.set_content(Some(&adw::NavigationPage::new(
            &crate::ui::scroll(&detail_body, 520),
            &gettext("Activity details"),
        )));
        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            700.0,
            adw::LengthUnit::Sp,
        ));
        breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
        root.set_child(Some(&split));
        root.add_breakpoint(breakpoint);
        let accounts: Vec<_> = accounts
            .iter()
            .map(|account| {
                let id = account.account.id.clone();
                let label = account
                    .account
                    .email
                    .clone()
                    .unwrap_or_else(|| id.0.clone());
                (id, label)
            })
            .collect();
        let filter = account_filter(&accounts);
        let group = adw::PreferencesGroup::new();
        group.add(&filter);
        body.append(&group);

        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.add_css_class("dim-label");
        body.append(&status);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        list.set_widget_name("activities-list");
        body.append(&list);
        let latest = gtk::Button::with_label(&gettext("Back to latest"));
        let older = gtk::Button::with_label(&gettext("Older activities"));
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&latest);
        buttons.append(&older);
        body.append(&buttons);

        let controller = Rc::new(Timeline {
            engine,
            accounts,
            split: split.downgrade(),
            details,
            selected: Cell::new(None),
            selected_row: Default::default(),
            filter: filter.downgrade(),
            status: status.downgrade(),
            list: list.downgrade(),
            latest: latest.downgrade(),
            older: older.downgrade(),
            before: Cell::new(None),
            next: Cell::new(None),
            generation: Cell::new(0),
            loading: Cell::new(false),
            dirty: Cell::new(false),
        });
        for button in [&latest, &older] {
            button.set_visible(false);
        }
        let state = controller.clone();
        filter.connect_selected_notify(move |_| state.show(None));
        let state = controller.clone();
        latest.connect_clicked(move |_| state.show(None));
        let state = controller.clone();
        older.connect_clicked(move |_| {
            if let Some(next) = state.next.get() {
                state.show(Some(next));
            }
        });
        let state = controller.clone();
        list.connect_map(move |_| state.refresh());
        Self { root, controller }
    }

    pub fn receiver(&self) -> impl Fn(&LocalBatch) + 'static {
        let state = Rc::downgrade(&self.controller);
        move |batch| {
            if batch
                .notices
                .iter()
                .any(|notice| !matches!(notice.activity, LocalActivity::Recheck(_)))
            {
                if let Some(state) = state.upgrade() {
                    state.refresh();
                }
            }
        }
    }
}

fn account_filter(accounts: &[(AccountId, String)]) -> adw::ComboRow {
    let choices = gtk::StringList::new(&[]);
    choices.append(&gettext("All accounts"));
    for (_, label) in accounts {
        choices.append(label);
    }
    let filter = adw::ComboRow::builder()
        .title(gettext("Account"))
        .model(&choices)
        .build();
    filter.set_widget_name("activities-account-filter");
    filter
}

struct Timeline {
    engine: Arc<DriveSyncEngine>,
    split: gtk::glib::WeakRef<adw::NavigationSplitView>,
    details: details::Details,
    selected: Cell<Option<i64>>,
    selected_row: gtk::glib::WeakRef<gtk::ListBoxRow>,
    accounts: Vec<(AccountId, String)>,
    filter: gtk::glib::WeakRef<adw::ComboRow>,
    status: gtk::glib::WeakRef<gtk::Label>,
    list: gtk::glib::WeakRef<gtk::Box>,
    latest: gtk::glib::WeakRef<gtk::Button>,
    older: gtk::glib::WeakRef<gtk::Button>,
    before: Cell<Option<i64>>,
    next: Cell<Option<i64>>,
    generation: Cell<u64>,
    loading: Cell<bool>,
    dirty: Cell<bool>,
}

impl Timeline {
    fn refresh(self: &Rc<Self>) {
        if self.before.get().is_some() || !self.list.upgrade().is_some_and(|list| list.is_mapped())
        {
            return;
        }
        if self.loading.get() {
            self.dirty.set(true);
        } else {
            self.load();
        }
    }

    fn show(self: &Rc<Self>, before: Option<i64>) {
        self.before.set(before);
        self.clear_selection();
        self.next.set(None);
        self.generation.set(self.generation.get().wrapping_add(1));
        self.dirty.set(true);
        if let Some(list) = self.list.upgrade() {
            clear(&list);
        }
        if let Some(older) = self.older.upgrade() {
            older.set_visible(false);
        }
        if let Some(latest) = self.latest.upgrade() {
            latest.set_visible(before.is_some());
        }
        if !self.loading.get() {
            self.load();
        }
    }

    fn load(self: &Rc<Self>) {
        let (Some(filter), Some(status)) = (self.filter.upgrade(), self.status.upgrade()) else {
            return;
        };
        if self
            .list
            .upgrade()
            .is_some_and(|list| list.first_child().is_none())
        {
            status.set_label(&gettext("Loading activities…"));
            status.set_visible(true);
        }
        self.loading.set(true);
        self.dirty.set(false);
        let selection = filter.selected() as usize;
        let accounts: Vec<_> = if selection == 0 {
            self.accounts.iter().map(|(id, _)| id.clone()).collect()
        } else {
            self.accounts
                .get(selection - 1)
                .map(|(id, _)| id.clone())
                .into_iter()
                .collect()
        };
        let generation = self.generation.get();
        let before = self.before.get();
        let engine = self.engine.clone();
        let weak = Rc::downgrade(self);
        crate::core::worker::work(
            move || engine.activity_history(&accounts, before),
            move |result| {
                let Some(state) = weak.upgrade() else { return };
                state.loading.set(false);
                if generation == state.generation.get() {
                    state.render(result, selection == 0);
                }
                if state.dirty.get() {
                    state.load();
                }
            },
        );
    }

    fn render(
        self: &Rc<Self>,
        result: Result<ActivityPage, drivesync_engine::EngineError>,
        show_account: bool,
    ) {
        let (Some(list), Some(status), Some(older)) = (
            self.list.upgrade(),
            self.status.upgrade(),
            self.older.upgrade(),
        ) else {
            return;
        };
        match result {
            Ok(page) => {
                clear(&list);
                status.set_label(&gettext("No activities recorded."));
                status.set_visible(page.records.is_empty());
                self.next.set(page.older);
                older.set_visible(page.older.is_some());
                if self
                    .selected
                    .get()
                    .is_some_and(|id| !page.records.iter().any(|record| record.id == id))
                {
                    self.clear_selection();
                }
                let weak = Rc::downgrade(self);
                let selected_row = rows::render_days(
                    &list,
                    page.records,
                    |id| {
                        show_account
                            .then(|| {
                                self.accounts
                                    .iter()
                                    .find(|(account, _)| account == id)
                                    .map(|(_, label)| label.as_str())
                            })
                            .flatten()
                    },
                    self.selected.get(),
                    move |record, row| {
                        if let Some(state) = weak.upgrade() {
                            state.select(record, row);
                        }
                    },
                );
                self.selected_row
                    .set(selected_row.as_ref().map(gtk::prelude::Cast::upcast_ref));
            }
            Err(_) => {
                status.set_label(&gettext("Could not load activity history."));
                status.set_visible(true);
            }
        }
    }
    fn account_label<'a>(&'a self, id: &'a AccountId) -> &'a str {
        self.accounts
            .iter()
            .find(|(account, _)| account == id)
            .map_or(id.0.as_str(), |(_, label)| label.as_str())
    }

    fn select(&self, record: &drivesync_engine::ActivityRecord, row: &gtk::ListBoxRow) {
        if let Some(previous) = self
            .selected_row
            .upgrade()
            .filter(|previous| previous != row)
        {
            if let Some(list) = previous.parent().and_downcast::<gtk::ListBox>() {
                list.unselect_row(&previous);
            }
        }
        self.selected.set(Some(record.id));
        self.selected_row.set(Some(row));
        self.details
            .show(record, self.account_label(&record.account));
        if let Some(split) = self.split.upgrade() {
            split.set_show_content(true);
        }
    }

    fn clear_selection(&self) {
        self.selected.set(None);
        self.selected_row.set(None);
        self.details.clear();
        if let Some(split) = self.split.upgrade() {
            split.set_show_content(false);
        }
    }
}

fn clear(list: &gtk::Box) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}
