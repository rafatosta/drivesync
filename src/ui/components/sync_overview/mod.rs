//! Standalone GTK presentation with no account, storage or synchronization dependencies.
mod models;
use gettextrs::{gettext, ngettext};
use gtk::{glib, prelude::*};
pub use models::SyncOverviewState;
use std::{cell::RefCell, rc::Rc, time::SystemTime};

/// Reusable overview. The caller exclusively controls state and completion.
#[derive(Clone)]
pub struct SyncOverview {
    root: gtk::Box,
    icon: gtk::Image,
    title: gtk::Label,
    description: gtk::Label,
    percentage: gtk::Label,
    progress: gtk::ProgressBar,
    details: gtk::Label,
    action: gtk::Button,
    detail_revealer: gtk::Revealer,
    tick: Rc<RefCell<Option<gtk::TickCallbackId>>>,
}

impl Default for SyncOverview {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncOverview {
    /// Creates a checking overview with no known count.
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("card");
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(body.upcast_ref(), 24);
        }
        root.append(&body);
        let icon = gtk::Image::new();
        icon.set_pixel_size(32);
        let title = gtk::Label::new(None);
        title.add_css_class("title-2");
        let description = gtk::Label::new(None);
        let percentage = gtk::Label::new(None);
        percentage.add_css_class("title-1");
        let progress = gtk::ProgressBar::new();
        progress.set_hexpand(true);
        let details = gtk::Label::new(None);
        details.add_css_class("dim-label");
        let action = gtk::Button::with_label(&gettext("Resolve pending items"));
        action.add_css_class("suggested-action");
        action.set_halign(gtk::Align::Center);
        for label in [&title, &description, &details] {
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
        }
        body.append(&icon);
        body.append(&title);
        body.append(&description);
        body.append(&percentage);
        body.append(&progress);
        let detail_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .transition_duration(150)
            .child(&details)
            .build();
        body.append(&detail_revealer);
        body.append(&action);
        let view = Self {
            root,
            icon,
            title,
            description,
            percentage,
            progress,
            details,
            action,
            detail_revealer,
            tick: Rc::default(),
        };
        view.set_state(SyncOverviewState::Checking {
            checked_items: None,
        });
        view
    }

    /// Returns the stable widget to place in a container.
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Updates existing widgets without inferring state or deriving progress from counts.
    pub fn set_state(&self, state: SyncOverviewState) {
        let mut fraction = None;
        let active = matches!(
            state,
            SyncOverviewState::Checking { .. } | SyncOverviewState::Syncing { .. }
        );
        self.icon.remove_css_class("success");
        self.icon.remove_css_class("warning");
        self.action
            .set_visible(matches!(state, SyncOverviewState::Attention { .. }));
        let (title, description, details, icon) = match state {
            SyncOverviewState::Checking { checked_items } => (
                gettext("Checking for changes"),
                gettext("Identifying items that need to be synced"),
                checked_items
                    .map(|n| count(n, "{n} item checked", "{n} items checked"))
                    .unwrap_or_default(),
                "view-refresh-symbolic",
            ),
            SyncOverviewState::Syncing {
                completed_items,
                total_items,
                remaining_items,
                remaining_bytes,
                progress,
            } => {
                fraction = progress
                    .filter(|v| v.is_finite())
                    .map(|v| v.clamp(0.0, 1.0));
                let description = match (completed_items, total_items) {
                    (Some(completed), Some(total)) => {
                        gettext("{completed} of {total} items synced")
                            .replace("{completed}", &number(completed))
                            .replace("{total}", &number(total))
                    }
                    (Some(completed), None) => {
                        count(completed, "{n} item synced", "{n} items synced")
                    }
                    (None, _) => gettext("Synchronizing your files"),
                };
                let mut details = Vec::new();
                if let Some(n) = remaining_items {
                    details.push(count(n, "{n} item remaining", "{n} items remaining"));
                }
                if let Some(n) = remaining_bytes {
                    details
                        .push(gettext("{size} remaining").replace("{size}", &glib::format_size(n)));
                }
                (
                    gettext("Syncing"),
                    description,
                    details.join("\n"),
                    "view-refresh-symbolic",
                )
            }
            SyncOverviewState::Waiting { message } => (
                gettext("Waiting for verification"),
                message,
                String::new(),
                "edit-find-symbolic",
            ),
            SyncOverviewState::Inactive { message } => (
                gettext("Synchronization inactive"),
                message,
                String::new(),
                "media-playback-pause-symbolic",
            ),
            SyncOverviewState::Completed { last_sync } => {
                self.icon.add_css_class("success");
                (
                    gettext("Everything is synced"),
                    gettext("Your files are up to date"),
                    last_sync
                        .and_then(date)
                        .map(|d| gettext("Last sync: {time}").replace("{time}", &d))
                        .unwrap_or_default(),
                    "object-select-symbolic",
                )
            }
            SyncOverviewState::Attention { pending_items } => {
                self.icon.add_css_class("warning");
                (
                    gettext("Sync needs attention"),
                    pending_items
                        .map(|n| {
                            count(
                                n,
                                "{n} item needs your attention",
                                "{n} items need your attention",
                            )
                        })
                        .unwrap_or_else(|| gettext("Review pending items to continue")),
                    String::new(),
                    "dialog-warning-symbolic",
                )
            }
        };
        self.title.set_label(&title);
        self.set_stage(&description);
        self.details.set_label(&details);
        self.detail_revealer.set_reveal_child(!details.is_empty());
        self.icon.set_icon_name(Some(icon));
        self.progress.set_visible(active);
        self.percentage.set_visible(fraction.is_some());
        if let Some(value) = fraction {
            self.progress.set_fraction(value);
            self.percentage.set_label(
                &gettext("{percent}%").replace("{percent}", &format!("{:.0}", value * 100.0)),
            );
        }
        let pulse = active && fraction.is_none();
        if !pulse {
            if let Some(id) = self.tick.borrow_mut().take() {
                id.remove();
            }
        } else if self.tick.borrow().is_none() {
            self.progress.set_fraction(0.0);
            let last = std::cell::Cell::new(0);
            let id = self.progress.add_tick_callback(move |bar, clock| {
                if clock.frame_time() - last.get() >= 100_000 {
                    bar.pulse();
                    last.set(clock.frame_time());
                }
                glib::ControlFlow::Continue
            });
            *self.tick.borrow_mut() = Some(id);
        }
    }

    /// Shows the backend's current stage without inventing measurable progress.
    pub fn set_stage(&self, description: &str) {
        self.description.set_label(description);
    }

    /// Connects the action; navigation and resolution belong to the caller.
    pub fn connect_attention_requested(&self, callback: impl Fn() + 'static) {
        self.action.connect_clicked(move |_| callback());
    }
}

fn number(n: u64) -> String {
    let digits = n.to_string();
    let separator = gettextrs::pgettext("Thousands separator", ",");
    let mut result = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push_str(&separator);
        }
        result.push(digit);
    }
    result
}
fn count(n: u64, singular: &str, plural: &str) -> String {
    ngettext(singular, plural, n.min(u32::MAX as u64) as u32).replace("{n}", &number(n))
}
fn date(time: SystemTime) -> Option<String> {
    let seconds = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).ok()?,
        Err(e) => -i64::try_from(e.duration().as_secs()).ok()?,
    };
    glib::DateTime::from_unix_local(seconds)
        .ok()?
        .format("%x %X")
        .ok()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests;
