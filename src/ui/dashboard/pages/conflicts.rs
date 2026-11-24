//! Real conflict review. Buttons submit revision-bound Engine choices, never simulated outcomes.
use crate::ui::dashboard::sync::{
    components::ConflictResolver,
    models::{ConflictChoice as ViewChoice, ConflictResolverState, ConflictView},
};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    conflicts::{
        ConflictChoice, ConflictError, ConflictResolutionStatus, ConflictVersion,
        ResolveConflictRequest,
    },
    Account, Engine,
};
use gettextrs::gettext;
pub(super) fn present(
    parent: &impl IsA<gtk::Widget>,
    engine: Engine,
    account: Account,
) -> gtk::Window {
    let window = gtk::Window::builder()
        .title(format!(
            "{} — {}",
            gettext("Synchronization conflicts"),
            account.email.as_deref().unwrap_or("DriveSync")
        ))
        .default_width(560)
        .default_height(420)
        .modal(true)
        .build();
    if let Some(parent) = parent.root().and_downcast::<gtk::Window>() {
        window.set_transient_for(Some(&parent));
    }
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(18);
    body.set_margin_bottom(18);
    body.set_margin_start(18);
    body.set_margin_end(18);
    let feedback = gtk::Label::builder()
        .label(gettext("Loading conflicts…"))
        .wrap(true)
        .build();
    body.append(&feedback);
    window.set_child(Some(&gtk::ScrolledWindow::builder().child(&body).build()));
    window.present();
    let body = body.downgrade();
    let feedback = feedback.downgrade();
    gtk::glib::spawn_future_local(async move {
        let result = engine.list_conflicts(account.id.clone()).await;
        let (Some(body), Some(feedback)) = (body.upgrade(), feedback.upgrade()) else {
            return;
        };
        let conflicts = match result {
            Ok(rows) => rows,
            Err(_) => {
                feedback.set_label(&gettext(
                    "Conflicts could not be loaded. Check that synchronization is running.",
                ));
                return;
            }
        };
        feedback.set_label(&if conflicts.is_empty() {
            gettext("No unresolved conflicts.")
        } else {
            gettext("Choose which version to use. File contents are preserved before replacement.")
        });
        for conflict in conflicts {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let path = conflict
                .local_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| gettext("Unidentified item"));
            row.append(
                &gtk::Label::builder()
                    .label(&path)
                    .xalign(0.0)
                    .wrap(true)
                    .selectable(true)
                    .build(),
            );
            let descriptions = [
                ("Content", gettext("The copies have different content.")),
                ("Name", gettext("The copies were renamed differently.")),
                (
                    "Parent",
                    gettext("The copies were moved to different folders."),
                ),
                ("Kind", gettext("The copies have incompatible file types.")),
                (
                    "Existence",
                    gettext("One copy is missing or trashed while the other changed."),
                ),
            ];
            let reasons = descriptions
                .iter()
                .filter(|(key, _)| conflict.reason.contains(key))
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>();
            let reason = if reasons.is_empty() {
                gettext("The copies cannot be associated safely from the available evidence.")
            } else {
                reasons.join(" ")
            };
            row.append(
                &gtk::Label::builder()
                    .label(&reason)
                    .xalign(0.0)
                    .wrap(true)
                    .build(),
            );
            let message = gtk::Label::builder().xalign(0.0).wrap(true).build();
            row.append(&message);
            if !conflict.can_resolve {
                message.set_label(&gettext(
                    "Restore compatible file types and access before choosing a version.",
                ));
            } else if conflict.resolution_pending {
                message.set_label(&gettext(
                    "A choice is recorded. Synchronization will recheck both copies.",
                ));
            } else {
                let open = gtk::Button::with_label(&gettext("Open preserved copies"));
                open.set_visible(false);
                open.connect_clicked({
                    let path = conflict.archive_path.clone();
                    let message = message.downgrade();
                    move |button| {
                        let file = gtk::gio::File::for_path(&path);
                        let parent = button.root().and_downcast::<gtk::Window>();
                        let message = message.clone();
                        gtk::glib::spawn_future_local(async move {
                            if gtk::FileLauncher::new(Some(&file))
                                .launch_future(parent.as_ref())
                                .await
                                .is_err()
                            {
                                if let Some(label) = message.upgrade() {
                                    label.set_label(&gettext(
                                        "Preserved copies could not be opened.",
                                    ));
                                }
                            }
                        });
                    }
                });
                row.append(&open);
                let review = gtk::Button::with_label(&gettext("Review versions"));
                review.set_widget_name("conflict-review");
                row.append(&review);
                let engine = engine.clone();
                let account = account.id.clone();
                let message = message.downgrade();
                let open = open.downgrade();
                let active = std::cell::RefCell::new(None);
                review.connect_clicked(move |button| {
                    if let Some(previous) = active.borrow_mut().take() {
                        let previous: std::rc::Rc<ConflictResolver> = previous;
                        previous.window.close();
                    }
                    let resolver = std::rc::Rc::new(ConflictResolver::new());
                    resolver
                        .window
                        .set_transient_for(button.root().and_downcast::<gtk::Window>().as_ref());
                    resolver.window.set_destroy_with_parent(true);
                    resolver.set_conflict(ConflictView {
                        id: conflict.id.0.clone(),
                        revision: conflict.revision,
                        name: path.clone(),
                        differences: reason.clone(),
                        local: version_text(&conflict.local),
                        remote: version_text(&conflict.remote),
                    });
                    resolver.local.set_widget_name("conflict-keep-local");
                    resolver.remote.set_widget_name("conflict-keep-remote");
                    resolver.confirm.set_widget_name("conflict-confirm");
                    resolver.connect_dismissed(|| {});
                    resolver.connect_choice({
                        let engine = engine.clone();
                        let account = account.clone();
                        let message = message.clone();
                        let open = open.clone();
                        let resolver = std::rc::Rc::downgrade(&resolver);
                        let button = button.downgrade();
                        move |request| {
                            let choice = match request.choice {
                                ViewChoice::KeepLocal => ConflictChoice::KeepLocal,
                                ViewChoice::KeepRemote => ConflictChoice::KeepRemote,
                            };
                            if let Some(button) = button.upgrade() {
                                button.set_sensitive(false);
                            }
                            gtk::glib::spawn_future_local(apply_choice(
                                engine.clone(),
                                ResolveConflictRequest {
                                    account_id: account.clone(),
                                    conflict_id: drivesync_engine::models::Id(request.id),
                                    expected_revision: request.revision,
                                    choice,
                                },
                                resolver.clone(),
                                button.clone(),
                                message.clone(),
                                open.clone(),
                            ));
                        }
                    });
                    *active.borrow_mut() = Some(resolver.clone());
                    resolver.window.present();
                });
            }
            body.append(&row);
        }
    });
    window
}

fn version_text(version: &ConflictVersion) -> String {
    let mut facts = Vec::new();
    if let Some(number) = version.provider_version {
        facts.push(gettext("Version {version}").replace("{version}", &number.to_string()));
    }
    if let Some(size) = version.size {
        facts.push(gtk::glib::format_size(size).into());
    }
    if let Some(value) = &version.checksum {
        facts.push(format!("{} {}", value.algorithm, value.value));
    }
    if let Some(instant) = version.modified_at {
        if let Ok(date) = gtk::glib::DateTime::from_unix_utc(instant.unix_millis.div_euclid(1000))
            .and_then(|date| date.to_local())
            .and_then(|date| date.format("%x %X"))
        {
            facts.push(date.into());
        }
    }
    facts.join("\n")
}

async fn apply_choice(
    engine: Engine,
    request: ResolveConflictRequest,
    resolver: std::rc::Weak<ConflictResolver>,
    review: gtk::glib::WeakRef<gtk::Button>,
    message: gtk::glib::WeakRef<gtk::Label>,
    open: gtk::glib::WeakRef<gtk::Button>,
) {
    use ConflictResolverState::*;
    let show = |state| {
        if let Some(resolver) = resolver.upgrade() {
            resolver.set_state(state);
        }
        if let Some(label) = message.upgrade() {
            label.set_label(&ConflictResolver::state_text(state));
        }
    };
    let account = request.account_id.clone();
    let id = request.conflict_id.clone();
    let outcome = match engine.resolve_conflict(request).await {
        Err(ConflictError::Stale | ConflictError::NotFound) => Stale,
        Err(_) => Failed,
        Ok(()) => {
            show(Applying);
            loop {
                gtk::glib::timeout_future(std::time::Duration::from_secs(1)).await;
                if message.upgrade().is_none() {
                    return;
                }
                match engine
                    .conflict_resolution_status(account.clone(), id.clone())
                    .await
                {
                    Ok(ConflictResolutionStatus::Applying) => {}
                    Ok(ConflictResolutionStatus::Resolved) => break Resolved,
                    Ok(ConflictResolutionStatus::Stale) => break Stale,
                    Ok(ConflictResolutionStatus::Failed) | Err(_) => break Failed,
                }
            }
        }
    };
    show(outcome);
    if outcome == Resolved {
        if let Some(button) = open.upgrade() {
            button.set_visible(true);
        }
    } else if outcome == Failed {
        if let Some(button) = review.upgrade() {
            button.set_sensitive(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn named(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
        if widget.widget_name() == name {
            return widget.clone().downcast().ok();
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(found) = named(&current, name) {
                return Some(found);
            }
            child = current.next_sibling();
        }
        None
    }
    #[test]
    #[ignore = "requires real isolated stage8 conflict, authorized Devel credentials and a GTK display"]
    fn real_conflict_button_submits_reviewed_choice_and_preserves_both_versions() {
        adw::init().unwrap();
        gtk::glib::MainContext::default().block_on(async {
            let env = |k: &str| std::env::var(k).expect(k);
            let local = std::path::PathBuf::from(env("DRIVESYNC_STAGE8_LOCAL_ROOT"));
            assert!(local
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("DriveSync-stage8-validation-"));
            let engine = Engine::open(drivesync_engine::EngineConfig {
                data_directory: env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY").into(),
                application_id: "com.rtosta.DriveSync.Devel".into(),
                google_oauth_file: Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
            })
            .await
            .unwrap();
            let accounts = engine
                .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
                .await
                .unwrap();
            let selected = accounts
                .accounts
                .into_iter()
                .filter(|a| a.email.as_deref() == Some("rtosta.coc@gmail.com"))
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 1);
            let account = selected[0].clone();
            engine
                .start_synchronization(drivesync_engine::StartSynchronizationRequest {
                    account_id: account.id.clone(),
                    local_root: local.clone(),
                    remote_root_id: drivesync_engine::models::RemoteId(env(
                        "DRIVESYNC_STAGE8_REMOTE_ROOT",
                    )),
                })
                .await
                .unwrap();
            // Allow startup captures to replace stale persisted evidence before reviewing a choice.
            gtk::glib::timeout_future(std::time::Duration::from_secs(6)).await;
            let rows = engine.list_conflicts(account.id.clone()).await.unwrap();
            assert_eq!(rows.len(), 1);
            let conflict = rows[0].clone();
            assert_eq!(
                engine
                    .resolve_conflict(ResolveConflictRequest {
                        account_id: account.id.clone(),
                        conflict_id: conflict.id.clone(),
                        expected_revision: conflict.revision - 1,
                        choice: ConflictChoice::KeepLocal
                    })
                    .await,
                Err(drivesync_engine::conflicts::ConflictError::Stale)
            );
            assert!(engine
                .resolve_conflict(ResolveConflictRequest {
                    account_id: drivesync_engine::AccountId("unrelated-account".into()),
                    conflict_id: conflict.id.clone(),
                    expected_revision: conflict.revision,
                    choice: ConflictChoice::KeepLocal
                })
                .await
                .is_err());
            let parent = gtk::Window::new();
            let button = gtk::Button::with_label("review");
            parent.set_child(Some(&button));
            parent.present();
            let dialog = present(&button, engine.clone(), account.clone());
            let mut clicked = false;
            for _ in 0..200 {
                if let Some(review) = named(dialog.upcast_ref(), "conflict-review") {
                    review.emit_clicked();
                    let resolver = gtk::Window::list_toplevels()
                        .into_iter()
                        .find(|window| named(window, "conflict-keep-local").is_some())
                        .unwrap();
                    let button = named(&resolver, "conflict-keep-local").unwrap();
                    button.emit_clicked();
                    named(&resolver, "conflict-confirm").unwrap().emit_clicked();
                    clicked = true;
                    break;
                }
                gtk::glib::timeout_future(std::time::Duration::from_millis(50)).await;
            }
            assert!(clicked, "real conflict choice button was not populated");
            let mut finished = false;
            for _ in 0..120 {
                if engine
                    .list_conflicts(account.id.clone())
                    .await
                    .unwrap()
                    .is_empty()
                {
                    finished = true;
                    break;
                }
                gtk::glib::timeout_future(std::time::Duration::from_secs(1)).await;
            }
            assert!(finished, "GUI choice did not converge");
            assert_eq!(std::fs::read(local.join("file.bin")).unwrap(), b"local008");
            let archive = local.join(".drivesync/conflicts").join(conflict.id.0);
            assert_eq!(
                std::fs::read(archive.join("local.bin")).unwrap(),
                b"local008"
            );
            assert_eq!(
                std::fs::read(archive.join("remote.bin")).unwrap(),
                b"remote08"
            );
            engine.stop_synchronization(account.id).await.unwrap();
            dialog.close();
            parent.close();
        });
    }
}
