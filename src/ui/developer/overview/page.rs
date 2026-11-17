//! Isolated SyncStatus scenarios; buttons emit presentation updates, never engine work.
use crate::ui::components::sync_status::{SyncActivity, SyncStatus, SyncStatusData};
use adw::{gtk, prelude::*};
use gettextrs::gettext;

pub(in crate::ui::developer) fn build() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(24);
    root.set_margin_end(24);
    root.append(
        &gtk::Label::builder()
            .label(gettext("Overview"))
            .xalign(0.0)
            .css_classes(["title-1"])
            .build(),
    );
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
    let simulations = gtk::Expander::builder()
        .label("Simular estados da interface")
        .child(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .max_content_height(240)
                .propagate_natural_height(true)
                .child(&controls)
                .build(),
        )
        .build();
    simulations.set_widget_name("overview-simulations");
    root.append(&simulations);
    card.update(&SyncStatusData {
        conflicts: 2,
        problems: 1,
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
        (
            "Atenção · 2 conflitos e 1 problema",
            vec![SyncStatusData {
                conflicts: 2,
                problems: 1,
                ..Default::default()
            }],
        ),
        (
            "Atenção · 1 conflito",
            vec![SyncStatusData {
                conflicts: 1,
                ..Default::default()
            }],
        ),
        (
            "Atenção · 2 problemas",
            vec![SyncStatusData {
                problems: 2,
                ..Default::default()
            }],
        ),
        (
            "Duas contas · atenção",
            vec![
                SyncStatusData {
                    conflicts: 1,
                    ..Default::default()
                },
                SyncStatusData {
                    conflicts: 1,
                    problems: 1,
                    ..Default::default()
                },
            ],
        ),
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
            .downcast::<gtk::Expander>()
            .unwrap()
            .child()
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap()
            .child()
            .unwrap()
            .downcast::<gtk::Viewport>()
            .unwrap()
            .child()
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
