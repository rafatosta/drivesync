//! Account/folder-scoped opt-in; asynchronous reads never silently grant consent.
use adw::{gtk, prelude::*};
use drivesync_engine::{
    ConflictPreference, DriveSyncEngine, EngineError, FolderConfiguration, RestoredAccount,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

const RULES: [ConflictPreference; 4] = [
    ConflictPreference::Ask,
    ConflictPreference::KeepLocal,
    ConflictPreference::KeepRemote,
    ConflictPreference::KeepBoth,
];
const LIMITS: &str = "Only simultaneous binary edits at the same path, with uploads enabled. Both versions are preserved. Deletions, renames and uncertain evidence always need attention. Dates never choose a winner. Asking again stops new choices; accepted recovery continues.";

fn row() -> adw::ComboRow {
    let row = adw::ComboRow::builder()
        .title("Simultaneous file edits")
        .subtitle(LIMITS)
        .model(&gtk::StringList::new(&[
            "Always ask",
            "Keep local · preserve Drive copy",
            "Keep Drive · preserve local copy",
            "Keep both as synchronized files",
        ]))
        .selected(0)
        .sensitive(false)
        .build();
    row.set_subtitle_lines(0);
    row.set_title_lines(0);
    row
}

/// Holds a reviewed folder snapshot so a delayed save cannot authorize a new pair.
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    folders: &gtk::Box,
) -> adw::ComboRow {
    let row = row();
    let job = crate::automatic::Controller::attach(engine.clone(), saved);
    let loaded = Rc::new(RefCell::new(
        None::<(FolderConfiguration, ConflictPreference)>,
    ));
    let busy = Rc::new(Cell::new(false));
    let guard = Rc::new(Cell::new(false));
    let retry_read = Rc::new(Cell::new(false));
    let weak = row.downgrade();
    let controls = folders.downgrade();
    let read_engine = engine.clone();
    let read_loaded = loaded.clone();
    let read_busy = busy.clone();
    let read_guard = guard.clone();
    let read_job = job.clone();
    let read_retry = retry_read.clone();
    let mut last = None;
    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(row) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        let folder = read_job.folder();
        row.set_sensitive(
            !read_busy.get()
                && !read_job.running()
                && read_loaded
                    .borrow()
                    .as_ref()
                    .is_some_and(|(f, _)| Some(f) == folder.as_ref())
                && controls.upgrade().is_some_and(|w| w.is_sensitive()),
        );
        if !read_busy.get() && !read_job.running() && (folder != last || read_retry.get()) {
            read_retry.set(false);
            last = folder.clone();
            read_loaded.replace(None);
            row.set_sensitive(false);
            read_guard.set(true);
            row.set_selected(0);
            read_guard.set(false);
            if let Some(folder) = folder {
                read_busy.set(true);
                let engine = read_engine.clone();
                let weak = row.downgrade();
                let loaded = read_loaded.clone();
                let busy = read_busy.clone();
                let guard = read_guard.clone();
                let retry = read_retry.clone();
                crate::flow::work(
                    move || {
                        let rule = engine.conflict_preference(&folder.account_id)?;
                        if engine.saved_folder(&folder.account_id)?.as_ref() != Some(&folder) {
                            return Err(EngineError::InvalidPlan);
                        }
                        Ok((folder, rule))
                    },
                    move |result| {
                        busy.set(false);
                        if let Some(row) = weak.upgrade() {
                            guard.set(true);
                            match result {
                            Ok((folder, rule)) => {
                                row.set_selected(RULES.iter().position(|r| *r == rule).unwrap() as u32);
                                row.set_subtitle(LIMITS);
                                loaded.replace(Some((folder, rule)));
                            }
                            Err(EngineError::SyncBusy) => {
                                retry.set(true);
                                row.set_subtitle("Waiting for synchronization to finish before reading the saved conflict preference. No new rule was authorized.");
                            }
                            Err(error) => row.set_subtitle(&format!("Could not read conflict preference: {error}. No new rule was authorized. Reopen this account to retry.")),
                        }
                            guard.set(false);
                        }
                    },
                );
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    row.connect_selected_notify(move |row| {
        if guard.get() { return; }
        let Some((folder, previous)) = loaded.borrow().clone() else { return; };
        let Some(rule) = RULES.get(row.selected() as usize).copied() else { return; };
        if busy.get() || job.running() {
            guard.set(true);
            row.set_selected(RULES.iter().position(|r| *r == previous).unwrap() as u32);
            guard.set(false);
            return;
        }
        busy.set(true);
        row.set_sensitive(false);
        let engine = engine.clone();
        let weak = row.downgrade();
        let loaded = loaded.clone();
        let busy = busy.clone();
        let guard = guard.clone();
        let retry = retry_read.clone();
        crate::flow::work(move || engine.set_conflict_preference(&folder, rule).map(|()| folder), move |result| {
            busy.set(false);
            if let Some(row) = weak.upgrade() {
                guard.set(true);
                match result {
                    Ok(folder) => { loaded.replace(Some((folder, rule))); row.set_subtitle(LIMITS); }
                    Err(error) => {
                        row.set_selected(RULES.iter().position(|r| *r == previous).unwrap() as u32);
                        loaded.replace(None);
                        if error == EngineError::SyncBusy {
                            // Reload durable consent after contention; never retry a rejected write.
                            retry.set(true);
                            row.set_subtitle("The preference was not saved because synchronization is running. The saved rule will be read again automatically.");
                        } else {
                            row.set_subtitle(&format!("Could not confirm the saved conflict preference: {error}. Reopen this account to read the durable rule."));
                        }
                    }
                }
                guard.set(false);
            }
        });
    });
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn preferences_present_ask_and_preservation_limits() {
        gtk::init().unwrap();
        adw::init().unwrap();
        let row = row();
        assert_eq!(row.selected(), 0);
        assert!(!row.is_sensitive());
        assert_eq!(row.model().unwrap().n_items(), 4);
        assert!(row
            .subtitle()
            .unwrap()
            .contains("Both versions are preserved"));
        assert!(row.subtitle().unwrap().contains("Deletions"));
        assert!(row.subtitle().unwrap().contains("Dates never choose"));
        let window = adw::Window::builder().default_width(360).build();
        let group = adw::PreferencesGroup::new();
        group.add(&row);
        window.set_content(Some(&group));
        window.present();
        let context = gtk::glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        assert!(row.width() <= 360);
        window.close();
    }
}
