//! Session view of actual downloads reported by the engine.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, DownloadProgress, RemoteCheckProgress};
use gettextrs::gettext;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(in crate::features::dashboard) const PAGE: &str = "transfers";
const RECENT_LIMIT: usize = 200;
const RATE_WINDOW: Duration = Duration::from_secs(3);
type Key = (AccountId, PathBuf);

#[derive(Clone, Default)]
pub(in crate::features::dashboard) struct TransferFeed(Arc<Mutex<FeedState>>);

#[derive(Default)]
struct FeedState {
    updates: VecDeque<Update>,
    dirty: bool,
    phases: HashMap<AccountId, (String, RemoteCheckProgress)>,
}

#[derive(Clone)]

struct Update {
    key: Key,
    account: String,
    progress: DownloadProgress,
    samples: VecDeque<(Instant, u64)>,
}

impl TransferFeed {
    pub fn phase(&self, id: &AccountId, account: &str, phase: Option<RemoteCheckProgress>) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(phase) = phase {
                state.phases.insert(id.clone(), (account.into(), phase));
            } else {
                state.phases.remove(id);
            }
            state.dirty = true;
        }
    }

    pub fn report(&self, id: &AccountId, account: &str, path: &Path, progress: DownloadProgress) {
        if let Ok(mut state) = self.0.lock() {
            let key = (id.clone(), path.to_path_buf());
            let mut samples = state
                .updates
                .iter()
                .position(|update| update.key == key)
                .and_then(|index| state.updates.remove(index))
                .map_or_else(VecDeque::new, |update| update.samples);
            if let DownloadProgress::Receiving { received, total } = progress {
                if received < total {
                    let now = Instant::now();
                    if samples
                        .back()
                        .is_some_and(|(_, previous)| received < *previous)
                    {
                        samples.clear();
                    }
                    while samples
                        .front()
                        .is_some_and(|(time, _)| now.duration_since(*time) > RATE_WINDOW)
                    {
                        samples.pop_front();
                    }
                    samples.push_back((now, received));
                } else {
                    samples.clear();
                }
            } else {
                samples.clear();
            }
            state.updates.push_back(Update {
                key,
                account: account.into(),
                progress,
                samples,
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

impl Update {
    fn rate(&self) -> Option<(Instant, u64)> {
        let &(start, first) = self.samples.front()?;
        let &(end, last) = self.samples.back()?;
        let elapsed = end.duration_since(start).as_secs_f64();
        (elapsed > 0.0 && last > first).then(|| (end, ((last - first) as f64 / elapsed) as u64))
    }
}

pub(super) fn phase_label(phase: &RemoteCheckProgress) -> String {
    match phase {
        RemoteCheckProgress::ReadingChanges => gettext("Reading the Google Drive change log…"),
        RemoteCheckProgress::ListingFolder(name) | RemoteCheckProgress::CheckingItem(name) => {
            let name: String = name
                .chars()
                .filter(|character| !character.is_control())
                .collect();
            let message = if matches!(phase, RemoteCheckProgress::CheckingItem(_)) {
                "Checking item: %s"
            } else {
                "Checking folder: %s"
            };
            gettext(message).replace("%s", &name)
        }
        RemoteCheckProgress::ApplyingChanges => gettext("Applying changes to the local folder…"),
    }
}

pub(in crate::features::dashboard) fn build(feed: TransferFeed) -> gtk::Box {
    let root = crate::ui::content();
    root.set_widget_name("transfers-page");
    let status = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .build();
    root.append(&status);
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
    let weak_status = status.downgrade();
    gtk::glib::timeout_add_local(Duration::from_millis(150), move || {
        let (Some(list), Some(empty)) = (weak_list.upgrade(), weak_empty.upgrade()) else {
            return gtk::glib::ControlFlow::Break;
        };
        let Some((updates, phases)) = feed.0.lock().ok().and_then(|mut state| {
            std::mem::take(&mut state.dirty).then(|| (state.updates.clone(), state.phases.clone()))
        }) else {
            for row in rows.values_mut() {
                row.expire_rate();
            }
            return gtk::glib::ControlFlow::Continue;
        };
        if let Some(status) = weak_status.upgrade() {
            let mut lines: Vec<_> = phases
                .values()
                .map(|(account, phase)| format!("{account} · {}", phase_label(phase)))
                .collect();
            lines.sort();
            status.set_label(&lines.join("\n"));
            status.set_visible(!lines.is_empty());
        }
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
            row.update(update.progress, update.rate());
        }
        empty.set_visible(rows.is_empty() && phases.is_empty());
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
    rate_at: Option<Instant>,
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
            rate_at: None,
        }
    }

    fn expire_rate(&mut self) {
        if self
            .rate_at
            .is_some_and(|time| time.elapsed() >= RATE_WINDOW)
        {
            self.bar.set_text(None);
            self.rate_at = None;
        }
    }

    fn update(&mut self, progress: DownloadProgress, rate: Option<(Instant, u64)>) {
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
        self.rate_at = None;
        self.bar.set_text(None);
        if let Some((received, total)) = measured.filter(|(_, total)| *total > 0) {
            let fraction = received as f64 / total as f64;
            self.bar.set_fraction(fraction);
            if let Some((time, bytes)) = rate.filter(|(time, _)| time.elapsed() < RATE_WINDOW) {
                self.rate_at = Some(time);
                self.bar.set_text(Some(&format!(
                    "{:.0}% · {}/s",
                    fraction * 100.0,
                    gtk::glib::format_size(bytes),
                )));
            }
        }
    }
}
