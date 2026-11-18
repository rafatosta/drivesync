//! Read-only views of the application's shared pipeline; opening a page starts no work.
use crate::application::Context;
use adw::{gtk, prelude::*};
use std::{cell::RefCell, rc::Rc, time::Duration};
fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build()
}
pub(super) fn build(context: &Context, operator: bool) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_start(24);
    root.set_margin_end(24);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_widget_name(if operator {
        "operator-inspection"
    } else {
        "orchestrator-inspection"
    });
    let title = label(if operator { "Operator" } else { "Orchestrator" });
    title.add_css_class("title-1");
    root.append(&title);
    root.append(&label(if operator {"Creates files and folders, transfers and updates content, renames, moves, trashes and restores resources. This session shows commands dispatched by the Analyzer."}else{"Detector → Inventory / Scanner → Analyzer → Operator → verification → confirmed baseline. Event acknowledgement records processing; it does not prove synchronization."}));
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_widget_name(if operator {
        "operator-account"
    } else {
        "orchestrator-account"
    });
    root.append(&accounts);
    let state = label("Select a configured account.");
    root.append(&state);
    let details = label("");
    root.append(&details);
    let history = label("");
    history.set_valign(gtk::Align::Start);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&history)
        .build();
    root.append(&scroll);
    let ids = Rc::new(RefCell::new(Vec::<String>::new()));
    let render: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let accounts = accounts.clone();
        let ids = ids.clone();
        move || {
            let sessions = context.detectors.borrow();
            let mut next: Vec<String> = sessions.keys().cloned().collect();
            next.sort();
            if *ids.borrow() != next {
                let selected = ids.borrow().get(accounts.selected() as usize).cloned();
                accounts.set_model(Some(&gtk::StringList::new(
                    &next.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                accounts.set_selected(
                    selected
                        .and_then(|id| next.iter().position(|n| *n == id))
                        .unwrap_or(0) as u32,
                );
                *ids.borrow_mut() = next;
            }
            let Some(session) = ids
                .borrow()
                .get(accounts.selected() as usize)
                .and_then(|id| sessions.get(id))
            else {
                let failures = context
                    .preparation_errors
                    .borrow()
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                state.set_label(if failures.is_empty() {"No active account session. Account preparation starts automatically after setup."} else {&failures});
                details.set_label("");
                history.set_label("");
                return;
            };
            let preparation = session.orchestrator.preparation_status();
            let execution = session.orchestrator.execution_status();
            state.set_label(&format!(
                "Pipeline: {:?} · Execution: {}",
                preparation.state,
                if execution.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
            let progress = execution
                .progress
                .map(|p| {
                    format!(
                        "{:?}: {} / {} bytes",
                        p.phase,
                        p.bytes_transferred
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "—".into()),
                        p.total_bytes
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "—".into())
                    )
                })
                .unwrap_or_default();
            let failure = preparation
                .error
                .map(|e| format!("Error: {e}\n"))
                .unwrap_or_default();
            details.set_label(&format!("Local folder: {}\nProcessed events: {} · Verified creations: {} · Failed attempts: {}\n{}{}",session.root.display(),preparation.events_processed,execution.completed,execution.failed,failure,progress));
            history.set_label(
                &execution
                    .recent
                    .iter()
                    .rev()
                    .map(|event| {
                        format!(
                            "{} · {:?} · {}{}",
                            event
                                .command
                                .as_ref()
                                .map(command_label)
                                .unwrap_or_else(|| event.item_id.0.clone()),
                            event.stage,
                            event
                                .operation_id
                                .as_ref()
                                .map(|id| format!("operation {}", id.0))
                                .unwrap_or_else(|| "checking prerequisites".into()),
                            event.error.map(|e| format!(" · {e:?}")).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    });
    render();
    let weak = root.downgrade();
    gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(_root) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        render();
        gtk::glib::ControlFlow::Continue
    });
    root
}

fn command_label(command: &drivesync_engine::operations::OperationCommand) -> String {
    use drivesync_engine::operations::{ItemName, OperationCommand as C, ResourceRef};
    if let C::Trash { target, .. } = command {
        return match target {
            ResourceRef::Local { .. } => "Move local item to trash".into(),
            ResourceRef::Remote { .. } => "Move remote item to trash".into(),
        };
    }
    let (action, name) = match command {
        C::Upload { name, .. } => ("Upload", name),
        C::Download { name, .. } => ("Download", name),
        C::CreateDirectory { name, .. } => ("Create folder", name),
        _ => return "Unsupported action".into(),
    };
    let name = match name {
        ItemName::Local(name) => name.to_string_lossy().into_owned(),
        ItemName::Remote(name) => name.clone(),
    };
    format!("{action}: {name}")
}
