//! Isolated SyncStatus scenarios; buttons emit presentation updates, never engine work.
use crate::ui::components::sync_status::{SyncActivity, SyncStatus, SyncStatusData};
use adw::{gtk, prelude::*};

pub(in crate::ui::developer) fn build() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(24);
    root.set_margin_end(24);
    root.append(
        &gtk::Label::builder()
            .label("SyncStatus · simulated events")
            .xalign(0.0)
            .css_classes(["title-1"])
            .build(),
    );
    root.append(&gtk::Label::builder().label("Preview only. These controls do not operate on your accounts. Unconfirmed inactivity hides the card.").wrap(true).xalign(0.0).build());
    let card = SyncStatus::new();
    root.append(card.widget());
    let controls = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(3)
        .row_spacing(6)
        .column_spacing(6)
        .build();
    for (name, snapshots) in scenarios() {
        let button = gtk::Button::with_label(name);
        let card = card.clone();
        button.connect_clicked(move |_| card.update(&SyncStatusData::aggregate(&snapshots)));
        controls.insert(&button, -1);
    }
    root.append(&controls);
    card.update(&SyncStatusData {
        synchronized: true,
        ..Default::default()
    });
    root
}

fn scenarios() -> Vec<(&'static str, Vec<SyncStatusData>)> {
    use SyncActivity::*;
    let confirmed = SyncStatusData {
        synchronized: true,
        ..Default::default()
    };
    let mut scenarios = vec![
        ("Confirmed synchronized", vec![confirmed.clone()]),
        ("Idle, unconfirmed", vec![SyncStatusData::default()]),
        (
            "Upload · 25%",
            vec![SyncStatusData {
                activity: Upload,
                completed: Some(3),
                total: Some(12),
                transferred_bytes: Some(24_600_000),
                ..Default::default()
            }],
        ),
        (
            "Download · bytes",
            vec![SyncStatusData {
                activity: Download,
                transferred_bytes: Some(50_000_000),
                total_bytes: Some(100_000_000),
                ..Default::default()
            }],
        ),
        (
            "Two accounts · one active",
            vec![
                confirmed.clone(),
                SyncStatusData {
                    activity: Scanning,
                    ..Default::default()
                },
            ],
        ),
        (
            "Two accounts · confirmed",
            vec![confirmed.clone(), confirmed],
        ),
        (
            "Two accounts · unknown",
            vec![
                SyncStatusData {
                    synchronized: true,
                    ..Default::default()
                },
                SyncStatusData::default(),
            ],
        ),
    ];
    for (label, activity) in [
        ("Upload · unknown total", Upload),
        ("Download · unknown total", Download),
        ("Scan", Scanning),
        ("Remote check", CheckingRemote),
        ("Rename", Renaming),
        ("Move", Moving),
        ("Delete", Deleting),
        ("Reconcile", Reconciling),
    ] {
        scenarios.push((
            label,
            vec![SyncStatusData {
                activity,
                ..Default::default()
            }],
        ));
    }
    scenarios
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated display"]
    fn sync_status_developer_scenarios() {
        adw::init().unwrap();
        let page = build();
        let controls = page
            .last_child()
            .unwrap()
            .downcast::<gtk::FlowBox>()
            .unwrap();
        for index in 0..scenarios().len() {
            let child = controls.child_at_index(index as i32).unwrap();
            child
                .child()
                .unwrap()
                .downcast::<gtk::Button>()
                .unwrap()
                .emit_clicked();
        }
    }
}
