//! Session view of actual downloads reported by the engine.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, DownloadProgress};
use gettextrs::gettext;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

pub(in crate::features::dashboard) const PAGE: &str = "transfers";
const RECENT_LIMIT: usize = 200;
type Key = (AccountId, PathBuf);

#[derive(Clone, Default)]
pub(in crate::features::dashboard) struct TransferFeed(Arc<Mutex<FeedState>>);

#[derive(Default)]
struct FeedState {
    updates: VecDeque<Update>,
    dirty: bool,
}

#[derive(Clone)]

struct Update {
    key: Key,
    account: String,
    progress: DownloadProgress,
}

impl TransferFeed {
    pub fn report(&self, id: &AccountId, account: &str, path: &Path, progress: DownloadProgress) {
        if let Ok(mut state) = self.0.lock() {
            let key = (id.clone(), path.to_path_buf());
            if let Some(index) = state.updates.iter().position(|update| update.key == key) {
                state.updates.remove(index);
            }
            state.updates.push_back(Update {
                key,
                account: account.into(),
                progress,
            });
            if state
                .updates
                .iter()
                .filter(|update| terminal(update.progress))
                .count()
                > RECENT_LIMIT
            {
                if let Some(index) = state
                    .updates
                    .iter()
                    .position(|update| terminal(update.progress))
                {
                    state.updates.remove(index);
                }
            }
            state.dirty = true;
        }
    }
}

pub(in crate::features::dashboard) fn build(feed: TransferFeed) -> gtk::Box {
    let root = crate::ui::content();
    root.set_widget_name("transfers-page");
    let empty = gtk::Label::new(Some(&gettext("No downloads in this session.")));
    empty.add_css_class("dim-label");
    empty.set_wrap(true);
    root.append(&empty);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.set_widget_name("transfers-list");
    root.append(&list);

    let mut rows: HashMap<Key, TransferRow> = HashMap::new();
    let (weak_list, weak_empty) = (list.downgrade(), empty.downgrade());
    gtk::glib::timeout_add_local(Duration::from_millis(150), move || {
        let (Some(list), Some(empty)) = (weak_list.upgrade(), weak_empty.upgrade()) else {
            return gtk::glib::ControlFlow::Break;
        };
        let Some(updates) =
            feed.0.lock().ok().and_then(|mut state| {
                std::mem::take(&mut state.dirty).then(|| state.updates.clone())
            })
        else {
            return gtk::glib::ControlFlow::Continue;
        };
        let keys: HashSet<_> = updates.iter().map(|update| &update.key).collect();
        rows.retain(|key, row| {
            let keep = keys.contains(key);
            if !keep {
                list.remove(&row.widget);
            }
            keep
        });
        for update in &updates {
            let row = rows.entry(update.key.clone()).or_insert_with(|| {
                let row = TransferRow::new(&update.key.1, &update.account);
                list.prepend(&row.widget);
                row
            });
            row.update(update.progress);
        }
        empty.set_visible(rows.is_empty());
        gtk::glib::ControlFlow::Continue
    });
    root
}

fn terminal(progress: DownloadProgress) -> bool {
    matches!(
        progress,
        DownloadProgress::Completed | DownloadProgress::Failed
    )
}

struct TransferRow {
    widget: adw::ActionRow,
    bar: gtk::ProgressBar,
    account: String,
}

impl TransferRow {
    fn new(path: &Path, account: &str) -> Self {
        let title: String = path
            .to_string_lossy()
            .chars()
            .map(|character| {
                if character.is_control() {
                    '\u{fffd}'
                } else {
                    character
                }
            })
            .collect();
        let widget = adw::ActionRow::builder()
            .title(&title)
            .use_markup(false)
            .selectable(false)
            .activatable(false)
            .build();
        widget.set_title_lines(2);
        widget.add_prefix(&gtk::Image::from_icon_name("folder-download-symbolic"));
        let bar = gtk::ProgressBar::builder()
            .valign(gtk::Align::Center)
            .show_text(true)
            .build();
        widget.add_suffix(&bar);
        Self {
            widget,
            bar,
            account: account.into(),
        }
    }

    fn update(&self, progress: DownloadProgress) {
        let status = match progress {
            DownloadProgress::Started { .. } => gettext("Starting download…"),
            DownloadProgress::Receiving { received, total } if received == total => {
                gettext("Verifying download…")
            }
            DownloadProgress::Receiving { .. } => gettext("Downloading"),
            DownloadProgress::Completed => gettext("Download completed"),
            DownloadProgress::Failed => gettext("Download failed"),
        };
        self.widget
            .set_subtitle(&format!("{} · {status}", self.account));
        let measured = match progress {
            DownloadProgress::Started { total } => Some((0, total)),
            DownloadProgress::Receiving { received, total } => Some((received, total)),
            _ => None,
        };
        self.bar
            .set_visible(measured.is_some_and(|(_, total)| total > 0));
        if let Some((received, total)) = measured.filter(|(_, total)| *total > 0) {
            self.bar.set_fraction(received as f64 / total as f64);
        }
    }
}
