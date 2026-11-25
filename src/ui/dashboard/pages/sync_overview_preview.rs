//! Dashboard-hosted visual preview; simulated values never describe the account.
use crate::ui::components::sync_overview::{SyncOverview, SyncOverviewState};
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::time::SystemTime;

pub(super) fn build() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.set_widget_name("dashboard-sync-overview-preview");
    root.set_visible(false);
    let heading = gtk::Label::builder()
        .label(gettext("Sync overview preview"))
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let notice = gtk::Label::builder()
        .label(gettext("Simulated data for visual review"))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let states = gtk::DropDown::from_strings(&[
        &gettext("Checking for changes"),
        &gettext("Syncing"),
        &gettext("Everything is synced"),
        &gettext("Sync needs attention"),
    ]);
    states.set_widget_name("dashboard-sync-overview-state");
    let overview = SyncOverview::new();
    overview.widget().set_widget_name("dashboard-sync-overview");
    overview.set_state(SyncOverviewState::Checking {
        checked_items: Some(2480),
    });
    overview.connect_attention_requested({
        let notice = notice.downgrade();
        move || {
            if let Some(notice) = notice.upgrade() {
                notice.set_label(&gettext(
                    "Preview action received; no account changes were made",
                ));
            }
        }
    });
    let preview = overview.clone();
    states.connect_selected_notify(move |states| {
        preview.set_state(match states.selected() {
            1 => SyncOverviewState::Syncing {
                completed_items: 1248,
                total_items: Some(1560),
                remaining_items: Some(312),
                remaining_bytes: Some(840_000_000),
                progress: Some(0.8),
            },
            2 => SyncOverviewState::Completed {
                last_sync: Some(SystemTime::now()),
            },
            3 => SyncOverviewState::Attention {
                pending_items: Some(3),
            },
            _ => SyncOverviewState::Checking {
                checked_items: Some(2480),
            },
        });
    });
    root.append(&heading);
    root.append(&notice);
    root.append(&states);
    root.append(overview.widget());
    root
}
