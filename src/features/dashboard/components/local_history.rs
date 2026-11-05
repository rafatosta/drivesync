//! Temporary account-local feedback for activity hints, not synchronization history.
use crate::core::local_observation::{Observation, Update};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    AccountId, DriveSyncEngine, LocalActivity, LocalBatch, LocalCoverageGap, LocalItemKind,
    LocalNotice, LocalWatchState,
};
use gettextrs::gettext;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{mpsc::TryRecvError, Arc},
    time::Duration,
};

const LIMIT: i32 = 200;

/// Owns only presentation; filesystem activity is collected off the GTK thread.
/// The timer holds weak widgets and cancels observation when the view is dropped.
pub(in crate::features::dashboard) struct LocalHistory {
    root: gtk::Box,
    restart: Rc<dyn Fn()>,
}
impl LocalHistory {
    pub fn new(
        engine: Arc<DriveSyncEngine>,
        id: AccountId,
        publish: impl Fn(&LocalBatch) + 'static,
    ) -> Self {
        Self::build_with(
            move || {
                let engine = engine.clone();
                let id = id.clone();
                let resolver = engine.clone();
                let account = id.clone();
                Observation::start_with(
                    move || resolver.local_observation_scope(&account).map_err(|_| ()),
                    move |scope| engine.folder_state(&id, scope),
                )
            },
            publish,
        )
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
    /// A weak-view callback suitable for successful configuration notifications.
    pub fn restart_callback(&self) -> Rc<dyn Fn()> {
        self.restart.clone()
    }

    #[cfg(test)]
    fn build(start: impl Fn() -> Observation + 'static) -> Self {
        Self::build_with(start, |_| {})
    }

    fn build_with(
        start: impl Fn() -> Observation + 'static,
        publish: impl Fn(&LocalBatch) + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_widget_name("local-activity-history");
        let group = adw::PreferencesGroup::builder()
            .title(gettext("Local changes"))
            .description(gettext("Temporary history · Most recent 200 entries"))
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
        status.set_widget_name("local-activity-status");
        status.add_css_class("dim-label");
        body.append(&status);
        let empty = gtk::Label::builder()
            .label(gettext("No changes detected in this session."))
            .xalign(0.0)
            .wrap(true)
            .build();
        empty.set_widget_name("local-activity-empty");
        empty.add_css_class("dim-label");
        body.append(&empty);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list.set_widget_name("local-activity-list");
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(240)
            .propagate_natural_height(true)
            .child(&list)
            .build();
        body.append(&scroll);
        group.add(&body);
        root.append(&group);
        let session = Rc::new(RefCell::new(None::<Observation>));
        let restart: Rc<dyn Fn()> = {
            let session = session.clone();
            let root = root.downgrade();
            let list = list.downgrade();
            let status = status.downgrade();
            let empty = empty.downgrade();
            Rc::new(move || {
                if root.upgrade().is_none() {
                    session.borrow_mut().take();
                    return;
                }
                session.borrow_mut().take();
                if let Some(list) = list.upgrade() {
                    while let Some(row) = list.first_child() {
                        list.remove(&row);
                    }
                }
                if let Some(empty) = empty.upgrade() {
                    empty.set_visible(true);
                }
                if let Some(status) = status.upgrade() {
                    status.set_label(&gettext("Starting local observation…"));
                    status.remove_css_class("warning");
                }
                *session.borrow_mut() = Some(start());
            })
        };
        restart();
        let weak_root = root.downgrade();
        let weak_list = list.downgrade();
        let weak_status = status.downgrade();
        let weak_empty = empty.downgrade();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            let (Some(_root), Some(list), Some(status), Some(empty)) = (
                weak_root.upgrade(),
                weak_list.upgrade(),
                weak_status.upgrade(),
                weak_empty.upgrade(),
            ) else {
                session.borrow_mut().take();
                return gtk::glib::ControlFlow::Break;
            };
            for _ in 0..4 {
                let received = session.borrow().as_ref().map(|s| s.receiver.try_recv());
                match received {
                    Some(Ok(Update::Batch(batch))) => {
                        render(&list, &empty, &status, &batch);
                        publish(&batch);
                        if batch.state == LocalWatchState::Stopped {
                            session.borrow_mut().take();
                            break;
                        }
                    }
                    Some(Ok(Update::Unconfigured)) => {
                        status.set_label(&gettext("Choose a local folder to observe changes."));
                        session.borrow_mut().take();
                        break;
                    }
                    Some(Ok(Update::PersistenceFailed)) => {
                        status.set_label(&gettext(
                            "Local changes could not be saved. Observation stopped.",
                        ));
                        status.add_css_class("warning");
                        session.borrow_mut().take();
                        break;
                    }
                    Some(Ok(Update::Failed)) | Some(Err(TryRecvError::Disconnected)) => {
                        status.set_label(&gettext(
                            "Could not observe this folder. Check its access permissions.",
                        ));
                        status.add_css_class("warning");
                        session.borrow_mut().take();
                        break;
                    }
                    Some(Err(TryRecvError::Empty)) | None => break,
                }
            }
            gtk::glib::ControlFlow::Continue
        });
        Self { root, restart }
    }
}

fn render(list: &gtk::ListBox, empty: &gtk::Label, status: &gtk::Label, batch: &LocalBatch) {
    let unverified = batch
        .notices
        .iter()
        .any(|n| n.activity == LocalActivity::Recheck(LocalCoverageGap::FilesystemUnverified));
    let text = match batch.state {
        LocalWatchState::Initializing => gettext("Starting local observation…"),
        LocalWatchState::Observing => gettext("Observing local changes."),
        LocalWatchState::Incomplete if unverified => {
            gettext("This folder cannot reliably report changes. Some changes may not appear.")
        }
        LocalWatchState::Incomplete => {
            gettext("Observation is incomplete. Some changes may not appear.")
        }
        LocalWatchState::Stopped => {
            gettext("Observation stopped. Check the folder and select it again.")
        }
    };
    status.set_label(&text);
    if matches!(
        batch.state,
        LocalWatchState::Incomplete | LocalWatchState::Stopped
    ) {
        status.add_css_class("warning");
    } else {
        status.remove_css_class("warning");
    }
    for row in batch
        .notices
        .iter()
        .filter_map(|notice| activity_row(notice, None))
    {
        list.prepend(&row);
        if let Some(oldest) = list.row_at_index(LIMIT) {
            list.remove(&oldest);
        }
    }
    empty.set_visible(list.first_child().is_none());
}

pub(super) fn activity_row(notice: &LocalNotice, account: Option<&str>) -> Option<adw::ActionRow> {
    let name = if account.is_some() {
        notice.path.as_os_str()
    } else {
        notice.path.file_name()?
    };
    let activity = activity_label(notice)?;
    // Both title and subtitle are plain text. Never interpret file names as markup.
    let text: String = name
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect();
    let (kind, icon) = match notice.object_type {
        LocalItemKind::Directory => (gettext("Folder"), "folder-symbolic"),
        LocalItemKind::File => (gettext("File"), "text-x-generic-symbolic"),
        LocalItemKind::Symlink => (gettext("Link"), "emblem-symbolic-link-symbolic"),
        LocalItemKind::Special => (gettext("Special item"), "text-x-generic-symbolic"),
        LocalItemKind::Unknown => (gettext("Item"), "dialog-question-symbolic"),
    };
    let mut detail = format!("{kind} · {activity}");
    if let Some(account) = account {
        detail.push_str(&format!(" · {account}"));
    }
    let row = adw::ActionRow::builder()
        .title(&text)
        .subtitle(&detail)
        .use_markup(false)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(2);
    row.set_tooltip_text(Some(&text));
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    Some(row)
}

/// Labels activity rows, excluding coverage notices handled by the status label.
fn activity_label(notice: &LocalNotice) -> Option<String> {
    Some(match &notice.activity {
        LocalActivity::Created => gettext("Created"),
        LocalActivity::Removed => gettext("Removed"),
        LocalActivity::Written => gettext("Write detected"),
        LocalActivity::ClosedAfterWrite => gettext("Closed after opening for writing"),
        LocalActivity::MetadataChanged => gettext("Properties changed"),
        LocalActivity::MovedIn => gettext("Entered the observed folder"),
        LocalActivity::MovedOut => gettext("Left the observed folder"),
        LocalActivity::Moved { from } if from.parent() == notice.path.parent() => {
            gettext("Renamed")
        }
        LocalActivity::Moved { from } if from.file_name() == notice.path.file_name() => {
            gettext("Moved")
        }
        LocalActivity::Moved { .. } => gettext("Moved and renamed"),
        LocalActivity::Recheck(_) => return None,
    })
}

#[cfg(test)]
mod tests;
