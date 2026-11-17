//! Isolated SyncStatus scenarios; buttons emit presentation updates, never engine work.
use crate::ui::components::alerts::{AlertAction, AlertItem, AlertKind, Alerts};
use crate::ui::components::sync_status::{SyncActivity, SyncStatus, SyncStatusData};
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};

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
    let navigation = adw::NavigationView::new();
    navigation.set_vexpand(true);
    let home = adw::NavigationPage::new(&root, &gettext("Overview"));
    navigation.add(&home);
    let card = SyncStatus::new();
    root.append(card.widget());
    let initial = SyncStatusData {
        conflicts: 2,
        problems: 1,
        ..Default::default()
    };
    let state = Rc::new(RefCell::new(initial.clone()));
    let items = Rc::new(RefCell::new(demo_alerts(&initial)));
    let alerts = Rc::new_cyclic(|weak: &std::rc::Weak<Alerts>| {
        let weak = weak.clone();
        let items = items.clone();
        let state = state.clone();
        let card = card.clone();
        Alerts::new(&navigation, move |id, _action| {
            let Some(alerts) = weak.upgrade() else {
                return;
            };
            // These controls only remove synthetic preview items, never real files.
            items.borrow_mut().retain(|item| item.id != id);
            let mut state = state.borrow_mut();
            state.conflicts = items
                .borrow()
                .iter()
                .filter(|i| i.kind == AlertKind::Conflict)
                .count() as u64;
            state.problems = items
                .borrow()
                .iter()
                .filter(|i| i.kind == AlertKind::Problem)
                .count() as u64;
            card.update(&state);
            alerts.update(&items.borrow());
        })
    });
    root.append(alerts.widget());
    alerts.update(&items.borrow());
    let controls = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(3)
        .row_spacing(6)
        .column_spacing(6)
        .build();
    for (name, snapshots) in scenarios() {
        let button = gtk::Button::with_label(name);
        let card = card.clone();
        let alerts = alerts.clone();
        let items = items.clone();
        let state = state.clone();
        button.connect_clicked(move |_| {
            let next = SyncStatusData::aggregate(&snapshots);
            *items.borrow_mut() = demo_alerts(&next);
            alerts.update(&items.borrow());
            card.update(&next);
            *state.borrow_mut() = next;
        });
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
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.append(&navigation);
    shell
}

/// Synthetic review choices used only in the Developer home preview.
fn demo_alerts(data: &SyncStatusData) -> Vec<AlertItem> {
    let mut items = Vec::new();
    for index in 0..data.conflicts {
        items.push(AlertItem {
            id: format!("preview/conflict/{index}"),
            kind: AlertKind::Conflict,
            title: if index == 0 {
                "Relatório.docx".into()
            } else {
                format!("Planejamento {}.ods", index + 1)
            },
            detail: gettext("Preview account · Modified on this computer and in Google Drive."),
            actions: [
                ("local", "Keep computer version (simulation)"),
                ("remote", "Keep Google Drive version (simulation)"),
                ("both", "Keep both versions (simulation)"),
            ]
            .into_iter()
            .map(|(id, label)| AlertAction {
                id: id.into(),
                label: gettext(label),
            })
            .collect(),
        });
    }
    for index in 0..data.problems {
        items.push(AlertItem {
            id: format!("preview/problem/{index}"),
            kind: AlertKind::Problem,
            title: format!("Documentos/Arquivo {}.pdf", index + 1),
            detail: gettext("Preview account · Permission to access this item is missing."),
            actions: vec![AlertAction {
                id: "retry".into(),
                label: gettext("Retry after adjustment (simulation)"),
            }],
        });
    }
    items
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
        let navigation = page
            .first_child()
            .unwrap()
            .downcast::<adw::NavigationView>()
            .unwrap();
        let home = navigation.visible_page().unwrap().child().unwrap();
        let controls = home
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
