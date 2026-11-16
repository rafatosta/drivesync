//! Explicit identity selection; never preselects a pair or executes file operations.
use crate::application::{
    analyzer::{self, AssociationCandidates, Scope},
    Context,
};
use adw::{gtk, prelude::*};
use drivesync_engine::inventory::AssociateItemsRequest;
use std::{cell::RefCell, rc::Rc};
fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .max_width_chars(60)
        .build()
}
fn dropdown(name: &str) -> gtk::DropDown {
    let dropdown = gtk::DropDown::from_strings(&[]);
    dropdown.set_widget_name(name);
    dropdown.set_hexpand(true);
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .max_width_chars(45)
                .build(),
        ));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let text = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .unwrap()
            .string();
        let label = item.child().and_downcast::<gtk::Label>().unwrap();
        label.set_label(&text);
        label.set_tooltip_text(Some(&text));
    });
    dropdown.set_factory(Some(&factory));
    dropdown.set_list_factory(Some(&factory));
    dropdown
}
pub(super) fn present(
    parent: &gtk::Widget,
    context: Context,
    scope: Scope,
    changed: Rc<dyn Fn()>,
) -> gtk::Window {
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();
    body.append(&label(&scope.label));
    body.append(&label("Select both identities explicitly. Equal names or content never associate files. This records a link only; it does not confirm synchronization or execute operations."));
    let local = dropdown("association-local");
    let remote = dropdown("association-remote");
    body.append(&label("Local item and identity"));
    body.append(&local);
    body.append(&label("Remote item and identity"));
    body.append(&remote);
    let message = label("Loading independent items…");
    body.append(&message);
    let confirm = gtk::Button::with_label("Associate selected identities");
    confirm.set_widget_name("associate-confirm");
    confirm.add_css_class("suggested-action");
    confirm.set_sensitive(false);
    body.append(&confirm);
    let window = gtk::Window::builder()
        .title("Associate identities")
        .default_width(560)
        .modal(true)
        .child(&body)
        .build();
    if let Some(parent) = parent.root().and_downcast::<gtk::Window>() {
        window.set_transient_for(Some(&parent));
        window.set_application(parent.application().as_ref());
    }
    let candidates: Rc<RefCell<Option<AssociationCandidates>>> = Rc::new(RefCell::new(None));
    for dropdown in [&local, &remote] {
        let (candidates, local, remote, confirm, message) = (
            candidates.clone(),
            local.clone(),
            remote.clone(),
            confirm.clone(),
            message.clone(),
        );
        dropdown.connect_selected_notify(move |_| {
            let data = candidates.borrow();
            confirm.set_sensitive(data.as_ref().is_some_and(|data| {
                data.local
                    .get(local.selected().wrapping_sub(1) as usize)
                    .zip(data.remote.get(remote.selected().wrapping_sub(1) as usize))
                    .is_some_and(|(l, r)| l.kind == r.kind)
            }));
            message.set_label(if confirm.is_sensitive() {
                "Ready to record the explicitly selected identities. No file content will change."
            } else {
                "Select both identities of the same type to enable association."
            });
        });
    }
    confirm.connect_clicked({
        let (candidates, local, remote, message, confirm, window, scope) = (
            candidates.clone(),
            local.clone(),
            remote.clone(),
            message.clone(),
            confirm.clone(),
            window.downgrade(),
            scope.clone(),
        );
        move |_| {
            let Some((l, r)) = candidates.borrow().as_ref().and_then(|d| {
                d.local
                    .get(local.selected().wrapping_sub(1) as usize)
                    .cloned()
                    .zip(
                        d.remote
                            .get(remote.selected().wrapping_sub(1) as usize)
                            .cloned(),
                    )
            }) else {
                return;
            };
            confirm.set_sensitive(false);
            local.set_sensitive(false);
            remote.set_sensitive(false);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let choice = AssociateItemsRequest {
                expected_local: l,
                expected_remote: r,
                decision_id: format!("user-association-{nanos}"),
                decided_at: drivesync_engine::Timestamp {
                    unix_millis: (nanos / 1_000_000).min(i64::MAX as u128) as i64,
                },
            };
            let (context, scope, message, window, changed) = (
                context.clone(),
                scope.clone(),
                message.clone(),
                window.clone(),
                changed.clone(),
            );
            gtk::glib::spawn_future_local(async move {
                match analyzer::associate(&context, scope, choice).await {
                    Ok(note) => {
                        changed();
                        if let Some(note) = note {
                            message.set_label(&note)
                        } else if let Some(w) = window.upgrade() {
                            w.close()
                        }
                    }
                    Err(error) => message.set_label(&format!(
                        "{error}. Reopen this dialog to reload current identities."
                    )),
                }
            });
        }
    });
    let opening = scope.request.clone();
    let (local, remote, message, candidates) = (
        local.clone(),
        remote.clone(),
        message.clone(),
        candidates.clone(),
    );
    gtk::glib::spawn_future_local(async move {
        let result = match opening {
            Ok(opening) => {
                gtk::gio::spawn_blocking(move || analyzer::association_candidates(opening))
                    .await
                    .map_err(|_| "Association worker unavailable".to_string())
                    .and_then(|r| r.map_err(|e| format!("Inventory unavailable: {:?}", e.kind)))
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(data) => {
                let names = |items: &[drivesync_engine::inventory::SyncItem]| {
                    items
                        .iter()
                        .map(|i| format!("{} · ID {}", super::page::file_name(i), i.id.0))
                        .collect::<Vec<_>>()
                };
                let mut l = vec!["Select a local identity…".to_string()];
                l.extend(names(&data.local));
                let mut r = vec!["Select a remote identity…".to_string()];
                r.extend(names(&data.remote));
                *candidates.borrow_mut() = Some(data);
                local.set_model(Some(&gtk::StringList::new(
                    &l.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                remote.set_model(Some(&gtk::StringList::new(
                    &r.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                local.set_selected(0);
                remote.set_selected(0);
                message.set_label(if l.len()==1||r.len()==1{"No eligible pair. Both sides must be independent, present and without a baseline."}else{"Choose both identities to enable association."});
            }
            Err(error) => message.set_label(&error),
        }
    });
    window.present();
    window
}
