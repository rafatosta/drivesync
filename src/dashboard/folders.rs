//! Account-local folder drafts and existing engine configuration on the dashboard.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveFolder, DriveSyncEngine, FolderConfiguration, RestoredAccount};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

#[derive(Clone, Default)]
struct Selection {
    remote: Option<DriveFolder>,
    local: Option<PathBuf>,
    saved: Option<FolderConfiguration>,
    local_available: bool,
}
impl Selection {
    fn from_saved(saved: Option<FolderConfiguration>) -> Self {
        Self {
            remote: saved.as_ref().map(|f| f.remote.clone()),
            local: saved.as_ref().map(|f| f.local.clone()),
            saved,
            local_available: false,
        }
    }
    fn dirty(&self) -> bool {
        match &self.saved {
            Some(saved) => {
                self.remote.as_ref().map(|r| &r.id) != Some(&saved.remote.id)
                    || self.local.as_ref() != Some(&saved.local)
            }
            None => self.remote.is_some() || self.local.is_some(),
        }
    }
    fn ready(&self) -> bool {
        self.remote.is_some() && self.local.is_some()
    }
}

/// Weak handles prevent chooser callbacks from retaining removed account pages.
#[derive(Clone)]
struct Widgets {
    remote: gtk::glib::WeakRef<adw::ActionRow>,
    local: gtk::glib::WeakRef<adw::ActionRow>,
    remote_button: gtk::glib::WeakRef<gtk::Button>,
    local_button: gtk::glib::WeakRef<gtk::Button>,
    feedback: gtk::glib::WeakRef<gtk::Label>,
    warning: gtk::glib::WeakRef<gtk::Image>,
    sync: gtk::glib::WeakRef<adw::ActionRow>,
    status: gtk::glib::WeakRef<gtk::Label>,
    editable: bool,
    start: gtk::glib::WeakRef<gtk::Button>,
}
impl Widgets {
    fn refresh(&self, draft: &Selection, busy: bool) {
        let unavailable =
            draft.saved.is_some() && !draft.local_available && !draft.dirty() && !busy;
        if let Some(warning) = self.warning.upgrade() {
            warning.set_visible(unavailable);
        }
        if let Some(feedback) = self.feedback.upgrade() {
            if unavailable {
                feedback.add_css_class("error");
            } else {
                feedback.remove_css_class("error");
            }
        }
        if let Some(row) = self.remote.upgrade() {
            row.set_subtitle(
                draft
                    .remote
                    .as_ref()
                    .map(|r| r.name.as_str())
                    .unwrap_or("Not configured"),
            );
        }
        if let Some(row) = self.local.upgrade() {
            let path = if !draft.dirty() {
                draft.saved.as_ref().map(|s| &s.local_display)
            } else {
                draft.local.as_ref()
            };
            row.set_subtitle(
                &path
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Not configured".into()),
            );
        }
        for button in [&self.remote_button, &self.local_button] {
            if let Some(button) = button.upgrade() {
                button.set_sensitive(self.editable && !busy);
            }
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label(if busy {
                "Saving folder configuration…"
            } else if draft.dirty() {
                "Folder configuration incomplete"
            } else if draft.saved.is_some() && !draft.local_available {
                "Local folder unavailable"
            } else if draft.saved.is_some() {
                "Configuration saved"
            } else {
                "Configuration required"
            });
        }
        if let Some(start) = self.start.upgrade() {
            start.set_sensitive(
                self.editable
                    && draft.saved.is_some()
                    && draft.local_available
                    && !draft.dirty()
                    && !busy,
            );
        }
        if let Some(sync) = self.sync.upgrade() {
            sync.set_subtitle(
                if draft.saved.is_some() && draft.local_available && !draft.dirty() && !busy {
                    "Folders are configured"
                } else {
                    "Choose folders before synchronization"
                },
            );
        }
    }
    fn message(&self, text: &str) {
        if let Some(label) = self.feedback.upgrade() {
            label.set_label(text);
            label.set_visible(!text.is_empty());
        }
    }
}

/// Builds independent choosers that save automatically once both folders exist.
/// Existing engine validation, portal handling and account isolation are reused.
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    sync: &adw::ActionRow,
    status: &gtk::Label,
    start: &gtk::Button,
) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let group = adw::PreferencesGroup::builder()
        .title("Folders")
        .description("The local folder cannot overlap another account’s folder.")
        .build();
    let remote = crate::flow::row("Google Drive", "Not configured", "folder-remote-symbolic");
    let local = crate::flow::row(
        "Folder on this computer",
        "Not configured",
        "folder-symbolic",
    );
    let warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
    warning.set_widget_name("local-folder-warning");
    warning.add_css_class("error");
    warning.set_tooltip_text(Some(
        "The saved local folder is unavailable. Choose it again.",
    ));
    warning.set_visible(false);
    local.add_suffix(&warning);
    let remote_button = gtk::Button::with_label("Change…");
    let local_button = gtk::Button::with_label("Change…");
    remote_button.set_widget_name("change-remote-folder");
    local_button.set_widget_name("change-local-folder");
    for (row, button, tooltip) in [
        (&remote, &remote_button, "Choose Google Drive folder"),
        (&local, &local_button, "Choose folder on this computer"),
    ] {
        row.set_subtitle_lines(2);
        button.set_valign(gtk::Align::Center);
        button.set_tooltip_text(Some(tooltip));
        row.add_suffix(button);
        row.set_activatable_widget(Some(button));
        group.add(row);
    }
    content.append(&group);
    let feedback = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .build();
    feedback.set_widget_name("folder-feedback");
    content.append(&feedback);
    let ui = Widgets {
        remote: remote.downgrade(),
        local: local.downgrade(),
        remote_button: remote_button.downgrade(),
        local_button: local_button.downgrade(),
        feedback: feedback.downgrade(),
        warning: warning.downgrade(),
        sync: sync.downgrade(),
        status: status.downgrade(),
        editable: crate::automatic::eligible(saved),
        start: start.downgrade(),
    };
    let draft = Rc::new(RefCell::new(Selection::from_saved(saved.folder.clone())));
    ui.refresh(&draft.borrow(), false);
    if let Some(folder) = &saved.folder {
        ui.refresh(&draft.borrow(), true);
        ui.message("Checking the saved local folder…");
        let path = folder.local.clone();
        let worker = engine.clone();
        let ui = ui.clone();
        let draft = draft.clone();
        crate::flow::work(
            move || worker.check_local_folder(&path),
            move |result| {
                draft.borrow_mut().local_available = result.is_ok();
                ui.refresh(&draft.borrow(), false);
                if result.is_ok() {
                    ui.message("");
                } else {
                    ui.message("The saved local folder is unavailable. It may have been moved, removed, or access may have changed. Choose the folder again.");
                }
            },
        );
    }
    {
        let ui = ui.clone();
        let draft = draft.clone();
        let engine = engine.clone();
        let id = saved.account.id.clone();
        remote_button.connect_clicked(move |button| {
            let ui = ui.clone();
            let draft = draft.clone();
            let save_engine = engine.clone();
            let save_id = id.clone();
            crate::folder_picker::show(
                engine.clone(),
                id.clone(),
                button.root().and_downcast::<gtk::Window>().as_ref(),
                move |folder| {
                    draft.borrow_mut().remote = Some(folder);
                    autosave(
                        save_engine.clone(),
                        save_id.clone(),
                        ui.clone(),
                        draft.clone(),
                    );
                },
            );
        });
    }
    {
        let ui = ui.clone();
        let draft = draft.clone();
        let engine = engine.clone();
        let id = saved.account.id.clone();
        local_button.connect_clicked(move |button| {
            let dialog = gtk::FileDialog::builder()
                .title("Choose Local Folder")
                .modal(true)
                .build();
            if let Some(path) = draft.borrow().local.as_ref() {
                dialog.set_initial_folder(Some(&gtk::gio::File::for_path(path)));
            }
            let parent = button.root().and_downcast::<gtk::Window>();
            let ui = ui.clone();
            let draft = draft.clone();
            let engine = engine.clone();
            let id = id.clone();
            gtk::glib::spawn_future_local(async move {
                match dialog.select_folder_future(parent.as_ref()).await {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            draft.borrow_mut().local = Some(path);
                            autosave(engine.clone(), id.clone(), ui.clone(), draft.clone());
                        } else {
                            ui.message("Choose a folder available on this computer.");
                        }
                    }
                    Err(e)
                        if e.matches(gtk::DialogError::Dismissed)
                            || e.matches(gtk::DialogError::Cancelled) => {}
                    Err(_) => ui.message("Could not open the folder selector. Please try again."),
                }
            });
        });
    }
    content
}

/// Validates and persists a complete changed pair off the GTK thread.
/// Partial selections remain local; failures preserve the last saved pair.
fn autosave(
    engine: Arc<DriveSyncEngine>,
    id: drivesync_engine::AccountId,
    ui: Widgets,
    draft: Rc<RefCell<Selection>>,
) {
    ui.refresh(&draft.borrow(), false);
    if !ui.editable {
        return;
    }
    if !draft.borrow().ready() {
        ui.message("Choose both a Google Drive folder and a folder on this computer.");
        return;
    }
    if !draft.borrow().dirty() && draft.borrow().local_available {
        ui.message("");
        return;
    }
    let (Some(remote), Some(local)) = (draft.borrow().remote.clone(), draft.borrow().local.clone())
    else {
        return;
    };
    ui.refresh(&draft.borrow(), true);
    ui.message("Checking folders and saving your settings…");
    crate::flow::work(
        move || {
            engine.configure_folder(&id, &remote.id, &local)?;
            engine.saved_folder(&id)
        },
        move |result| {
            match result {
                Ok(Some(saved)) => {
                    crate::automatic::configured(saved.clone());
                    *draft.borrow_mut() = Selection::from_saved(Some(saved));
                    draft.borrow_mut().local_available = true;
                    ui.message("");
                }
                Ok(None) => ui.message("Could not load the saved folders. Please try again."),
                Err(e) => ui.message(&e.to_string()),
            }
            ui.refresh(&draft.borrow(), false);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_save_requires_complete_changed_pair() {
        let mut draft = Selection::default();
        assert!(!draft.ready());
        assert!(!draft.dirty());
        draft.remote = Some(DriveFolder {
            id: "remote".into(),
            name: "Projects".into(),
        });
        assert!(draft.dirty());
        assert!(!draft.ready());
        draft.local = Some("/tmp/projects".into());
        assert!(draft.ready());
        let saved = FolderConfiguration {
            account_id: drivesync_engine::AccountId("google:test".into()),
            remote: draft.remote.clone().unwrap(),
            local: draft.local.clone().unwrap(),
            local_display: "/tmp/projects".into(),
        };
        let mut draft = Selection::from_saved(Some(saved.clone()));
        assert!(!draft.dirty());
        draft.local = Some("/tmp/other".into());
        assert!(draft.dirty());
        let draft = Selection::from_saved(draft.saved);
        assert_eq!(draft.local, Some(saved.local));
        assert!(!draft.dirty());
    }
}
