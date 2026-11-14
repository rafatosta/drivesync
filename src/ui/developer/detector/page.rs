//! Per-account detector inspection; engine snapshots own counters and event context.
use crate::application::{detectors, Context};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    models::{FileLocation, FileScope, Timestamp},
    orchestrator::DetectorEventSummary,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

fn label(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes([class])
        .build()
}
fn metric(metrics: &gtk::FlowBox, title: &str) -> gtk::Label {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
    block.set_hexpand(true);
    block.set_size_request(140, -1);
    block.append(&label(title, "dim-label"));
    let value = label("—", "title-2");
    block.append(&value);
    metrics.insert(&block, -1);
    value
}
fn time(timestamp: Timestamp) -> String {
    gtk::glib::DateTime::from_unix_local(timestamp.unix_millis.div_euclid(1000))
        .and_then(|date| date.format("%H:%M:%S"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "—".into())
}
fn location(location: &FileLocation) -> String {
    match location {
        FileLocation::Local(path) => String::from_utf8_lossy(path.as_bytes()).into_owned(),
        FileLocation::Remote { name, .. } => name.clone().unwrap_or_else(|| "Remote item".into()),
    }
}
fn event_label(kind: drivesync_engine::models::FileEventKind) -> (&'static str, &'static str) {
    use drivesync_engine::models::FileEventKind::*;
    match kind {
        Created => ("CREATE", "detector-event-created"),
        Modified => ("MODIFY", "detector-event-modified"),
        Renamed => ("RENAME", "detector-event-location"),
        Moved => ("MOVE", "detector-event-location"),
        Deleted => ("DELETE", "detector-event-deleted"),
        MetadataChanged => ("METADATA", "detector-event-modified"),
        RecheckRequired => ("RECHECK", "detector-event-recheck"),
        ScopeUnavailable => ("UNAVAILABLE", "detector-event-deleted"),
    }
}
fn uncertainty_message(reason: drivesync_engine::models::ObservationUncertainty) -> &'static str {
    use drivesync_engine::models::ObservationUncertainty::*;
    match reason {
        CaptureStarted => "Capture started; initial verification required",
        CaptureRebuilt => "Directory capture was rebuilt",
        CoverageIncomplete => "Observation coverage is incomplete",
        NativeOverflow => "Filesystem notifications were lost",
        ResourceLimit => "Observation resource limit reached",
        UnpairedMove => "Move could not be correlated",
        UnclassifiedSignal => "Filesystem signal could not be classified",
        ObservationUnstable => "Entry changed or disappeared during observation",
        WriteInProgress => "Write did not close before the normalization deadline",
        ConflictingSequence => "Overlapping operations require verification",
    }
}
fn description(event: &DetectorEventSummary) -> String {
    let location = match (&event.event.previous, &event.event.current) {
        (Some(previous), Some(current)) if previous != current => {
            format!("{} → {}", location(previous), location(current))
        }
        (_, Some(current)) => location(current),
        (Some(previous), _) => location(previous),
        _ => match &event.scope {
            FileScope::Local(path) => String::from_utf8_lossy(path.as_bytes()).into_owned(),
            FileScope::WholeSync => "Verify monitored folder".into(),
            FileScope::Remote(_) => "Remote scope".into(),
        },
    };
    match event.event.uncertainty {
        Some(reason) => format!("{location} — {}", uncertainty_message(reason)),
        None => location,
    }
}
#[derive(Clone)]
struct View {
    status: gtk::Label,
    last: gtk::Label,
    detected: gtk::Label,
    pending: gtk::Label,
    count: gtk::Label,
    stream: gtk::ListBox,
    detail: gtk::Label,
    extra_detail: gtk::Label,
    extra: gtk::Expander,
    delivery_toggle: gtk::glib::WeakRef<gtk::Button>,
    control_busy: Rc<Cell<bool>>,
}
impl View {
    fn clear(&self) {
        self.extra.set_visible(false);
        if let Some(button) = self.delivery_toggle.upgrade() {
            button.set_sensitive(false);
        }
        self.status.remove_css_class("success");
        self.status.remove_css_class("error");
        for value in [&self.last, &self.detected, &self.pending] {
            value.set_label("—");
        }
        self.count.set_label("0 events");
        while let Some(child) = self.stream.first_child() {
            self.stream.remove(&child);
        }
    }
    fn render(&self, context: &Context, id: Option<&str>, remote: bool, error: Option<&str>) {
        self.clear();
        if remote {
            self.render_remote(context, id);
            return;
        }
        if let Some(error) = error {
            self.status.set_label("Unavailable");
            self.status.add_css_class("error");
            self.detail.set_label(error);
            return;
        }
        let sessions = context.detectors.borrow();
        let Some(session) = id.and_then(|id| sessions.get(id)) else {
            self.status.set_label("Unavailable");
            self.detail
                .set_label("Select a configured account to inspect its local detector.");
            return;
        };
        let snapshot = session.orchestrator.diagnostics();
        if let Some(button) = self.delivery_toggle.upgrade() {
            use drivesync_engine::detectors::DeliveryState;
            let paused = snapshot.delivery == Some(DeliveryState::Paused);
            button.set_icon_name(if paused {
                "media-playback-start-symbolic"
            } else {
                "media-playback-pause-symbolic"
            });
            button.set_tooltip_text(Some(if paused {
                "Resume delivery"
            } else {
                "Interrupt delivery for this account · Continue detecting and persisting changes"
            }));
            button.update_property(&[gtk::accessible::Property::Label(if paused {
                "Resume delivery"
            } else {
                "Interrupt delivery"
            })]);
            button.set_sensitive(snapshot.delivery.is_some() && !self.control_busy.get());
        }

        if let Ok(status) = session.orchestrator.local_status() {
            use drivesync_engine::detectors::LocalDetectorState::*;
            self.status.set_label(match status.state {
                Running => "Monitoring",
                Starting => "Starting",
                Degraded => "Degraded",
                Failed => "Failed",
                Stopping => "Stopping",
                Stopped => "Stopped",
            });
            self.status.remove_css_class("success");
            self.status.remove_css_class("error");
            if status.state == Running {
                self.status.add_css_class("success");
            }
            if matches!(status.state, Failed | Degraded) {
                self.status.add_css_class("error");
            }
            self.detail.set_label(&format!(
                "Folder: {}\nActivity: {:?} · Coverage: {:?}{}",
                session.root.display(),
                status.activity,
                status.last_coverage,
                status
                    .last_error
                    .map(|e| format!(" · Error: {e:?}"))
                    .unwrap_or_default()
            ));
        }
        if let Some(delivery) = snapshot.delivery {
            self.detail.set_label(&format!(
                "{}
Delivery: {delivery:?} · Leased: {} · Confirmed: {} · Initial/recovery observations: {}{}",
                self.detail.text(),
                snapshot.leased_events.unwrap_or(0),
                snapshot.confirmed_events.unwrap_or(0),
                snapshot.initial_observations,
                snapshot
                    .queue_error
                    .map(|e| format!(" · Last queue error: {e:?}"))
                    .unwrap_or_default()
            ));
        }
        self.last.set_label(
            &snapshot
                .last_event_at
                .map(time)
                .unwrap_or_else(|| "—".into()),
        );
        self.detected.set_label(
            &snapshot
                .events_detected
                .saturating_sub(snapshot.initial_observations)
                .to_string(),
        );
        self.pending.set_label(
            &snapshot
                .pending_events
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        );
        self.pending.set_tooltip_text(Some(if snapshot.pending_events.is_some() { "Unconfirmed durable events, including reservations. Pausing delivery keeps filesystem capture and persistence active." } else { "This observation-only session has no durable queue." }));
        self.count
            .set_label(&format!("{} events", snapshot.events.len()));
        self.render_events(snapshot.events, false);
    }
    fn render_remote(&self, context: &Context, id: Option<&str>) {
        use drivesync_engine::{detectors::*, models::*};
        let sessions = context.detectors.borrow();
        let Some(session) = id.and_then(|id| sessions.get(id)) else {
            self.status.set_label("Unavailable");
            self.detail
                .set_label("Select a configured account to inspect its remote detector.");
            return;
        };
        let Some(snapshot) = session.orchestrator.remote_diagnostics() else {
            self.status.set_label("Unavailable");
            self.detail.set_label(
                session
                    .remote_error
                    .as_deref()
                    .unwrap_or("Remote detector unavailable"),
            );
            return;
        };
        let status = snapshot.status;
        self.status.set_label(match status.state {
            RemoteState::Stopped => "Stopped",
            RemoteState::Starting => "Starting",
            RemoteState::Running => "Monitoring",
            RemoteState::Degraded => "Degraded",
            RemoteState::Stopping => "Stopping",
            RemoteState::Failed => "Failed",
        });
        if status.state == RemoteState::Running {
            self.status.add_css_class("success");
        }
        if matches!(status.state, RemoteState::Failed | RemoteState::Degraded) {
            self.status.add_css_class("error");
        }
        if let Some(button) = self.delivery_toggle.upgrade() {
            let paused = snapshot.queue.delivery == DeliveryState::Paused;
            button.set_icon_name(if paused {
                "media-playback-start-symbolic"
            } else {
                "media-playback-pause-symbolic"
            });
            button.set_tooltip_text(Some(if paused {
                "Resume delivery"
            } else {
                "Interrupt delivery for this account · Continue detecting and persisting changes"
            }));
            button.update_property(&[gtk::accessible::Property::Label(if paused {
                "Resume delivery"
            } else {
                "Interrupt delivery"
            })]);
            button.set_sensitive(!self.control_busy.get());
        }
        let last = snapshot
            .events
            .last()
            .map(|file| time(file.observed_at))
            .unwrap_or_else(|| "—".into());
        self.last.set_label(&last);
        self.detected
            .set_label(&snapshot.queue.changes_detected.to_string());
        self.pending.set_label(
            &snapshot
                .queue
                .pending
                .saturating_add(snapshot.queue.leased)
                .to_string(),
        );
        self.pending.set_tooltip_text(Some("Unconfirmed durable remote events. Pausing delivery keeps polling and persistence active."));
        self.count
            .set_label(&format!("{} events", snapshot.events.len()));
        self.extra.set_visible(true);
        self.detail.set_label(&format!(
            "Interval: {:.1}s · Last poll: {} · Next poll: {}{}",
            status.polling_interval.as_secs_f64(),
            status
                .last_poll_started_at
                .map(time)
                .unwrap_or_else(|| "—".into()),
            status.next_poll_at.map(time).unwrap_or_else(|| "—".into()),
            status
                .last_error
                .map(|error| format!("\nError: {error:?}"))
                .unwrap_or_default(),
        ));
        self.extra_detail.set_label(&format!(
            "Folder: {}\nInterval: {:.1}s · Last poll: {} · Next poll: {} · Duration: {}\nActivity: {:?} · Coverage: {:?} · Cursor: {} · Returned: {} · Produced: {}\nDelivery: {:?} · Leased: {} · Confirmed: {} · Initial/recovery observations: {} · Failures: {} · Recoveries: {}{}{}",
            session.remote_root_id, status.polling_interval.as_secs_f64(),
            status.last_poll_started_at.map(time).unwrap_or_else(|| "—".into()),
            status.next_poll_at.map(time).unwrap_or_else(|| "—".into()),
            status.last_poll_duration.map(|duration| format!("{}ms", duration.as_millis())).unwrap_or_else(|| "—".into()),
            status.activity, status.coverage, status.cursor.as_deref().unwrap_or("—"),
            status.observed_in_current_round, status.emitted_in_current_round,
            snapshot.queue.delivery, snapshot.queue.leased, snapshot.queue.confirmed, snapshot.queue.initial_observations,
            status.consecutive_failures, status.recovery_attempts,
            status.last_error.map(|error| format!(" · Error: {error:?}")).unwrap_or_default(),
            snapshot.queue.last_error.map(|error| format!(" · Queue: {error:?}")).unwrap_or_default(),
        ));
        self.render_events(
            snapshot
                .events
                .into_iter()
                .map(|file| DetectorEventSummary {
                    id: file.id,
                    observed_at: file.observed_at,
                    event: file.event,
                    scope: file.scope,
                    entry_kind: match file.entry_kind {
                        Some(RemoteEntryKind::Directory) => Some(LocalEntryKind::Directory),
                        Some(RemoteEntryKind::File) => Some(LocalEntryKind::Regular),
                        Some(RemoteEntryKind::Shortcut) => Some(LocalEntryKind::Symlink),
                        None => None,
                    },
                })
                .collect(),
            true,
        );
    }
    fn render_events(&self, events: Vec<DetectorEventSummary>, remote: bool) {
        for event in events {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            row.set_margin_start(12);
            row.set_margin_end(12);
            let timestamp = label(&time(event.observed_at), "caption");
            timestamp.set_width_chars(8);
            row.append(&timestamp);
            let (title, class) = event_label(event.event.kind);
            let kind = label(title, "caption-heading");
            kind.add_css_class("detector-event");
            kind.add_css_class(class);
            kind.set_valign(gtk::Align::Center);
            kind.set_wrap(false);
            kind.set_tooltip_text(Some("Event kind reported by the engine; native hints are not treated as proven operations."));
            row.append(&kind);
            use drivesync_engine::models::LocalEntryKind;
            let (icon, title) = if event.scope == FileScope::WholeSync {
                ("folder-saved-search-symbolic", "Monitored scope")
            } else {
                match event.entry_kind {
                    Some(LocalEntryKind::Directory) => ("folder-symbolic", "Folder"),
                    Some(LocalEntryKind::Regular) => ("text-x-generic-symbolic", "File"),
                    Some(LocalEntryKind::Symlink) if remote => {
                        ("emblem-symbolic-link-symbolic", "Google Drive shortcut")
                    }
                    Some(LocalEntryKind::Symlink) => {
                        ("emblem-symbolic-link-symbolic", "Symbolic link")
                    }
                    Some(LocalEntryKind::Unknown) | None => {
                        ("dialog-question-symbolic", "Unknown type")
                    }
                    Some(_) => ("drive-harddisk-symbolic", "Special filesystem entry"),
                }
            };
            let entry_type = gtk::Image::from_icon_name(icon);
            entry_type.set_tooltip_text(Some(title));
            entry_type.set_valign(gtk::Align::Center);
            entry_type.update_property(&[gtk::accessible::Property::Label(title)]);
            row.append(&entry_type);
            let path = label(&description(&event), "body");
            path.set_hexpand(true);
            path.set_selectable(true);
            row.append(&path);
            self.stream.append(&row);
        }
    }
}
pub(in crate::ui::developer) fn build(context: &Context) -> gtk::Box {
    build_inner(context, true)
}
fn build_inner(context: &Context, load_accounts: bool) -> gtk::Box {
    thread_local! {
        static STYLE: gtk::CssProvider = {
            let css = gtk::CssProvider::new();
            css.load_from_data(include_str!("style.css"));
            gtk::style_context_add_provider_for_display(&gtk::gdk::Display::default().expect("GTK display"), &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            css
        };
    }
    STYLE.with(|_| {});

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for edge in [
        gtk::PositionType::Left,
        gtk::PositionType::Right,
        gtk::PositionType::Top,
        gtk::PositionType::Bottom,
    ] {
        match edge {
            gtk::PositionType::Left => body.set_margin_start(20),
            gtk::PositionType::Right => body.set_margin_end(20),
            gtk::PositionType::Top => body.set_margin_top(16),
            _ => body.set_margin_bottom(0),
        }
    }
    let account_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_hexpand(true);
    let selected_account = gtk::SignalListItemFactory::new();
    selected_account.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("dropdown item");
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .width_chars(1)
                .max_width_chars(28)
                .build(),
        ));
    });
    selected_account.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("dropdown item");
        let text = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .expect("account text");
        let label = item
            .child()
            .and_downcast::<gtk::Label>()
            .expect("account label");
        label.set_label(&text.string());
        label.set_tooltip_text(Some(&text.string()));
    });
    accounts.set_factory(Some(&selected_account));
    accounts.set_tooltip_text(Some("Account whose detector is being inspected"));
    accounts.set_widget_name("detector-account");
    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Reload accounts and configured folders")
        .build();
    account_row.append(&accounts);
    account_row.append(&refresh_button);
    body.append(&account_row);
    let delivery_toggle = gtk::Button::from_icon_name("media-playback-pause-symbolic");
    delivery_toggle.set_widget_name("detector-delivery-toggle");
    delivery_toggle.set_tooltip_text(Some(
        "Pause delivery for this account. Continue detecting and persisting changes.",
    ));
    delivery_toggle.set_halign(gtk::Align::Start);
    delivery_toggle.set_sensitive(false);
    account_row.append(&delivery_toggle);
    let control_busy = Rc::new(Cell::new(false));
    let feedback = label("", "error");
    feedback.set_visible(false);
    feedback.connect_label_notify(|label| label.set_visible(!label.text().is_empty()));
    body.append(&feedback);
    let tabs = gtk::Stack::new();
    tabs.add_titled(
        &gtk::Box::new(gtk::Orientation::Vertical, 0),
        Some("local"),
        "Local",
    );
    tabs.add_titled(
        &gtk::Box::new(gtk::Orientation::Vertical, 0),
        Some("remote"),
        "Remote",
    );
    let switcher = gtk::StackSwitcher::builder()
        .stack(&tabs)
        .halign(gtk::Align::Center)
        .build();
    switcher.set_widget_name("detector-tabs");
    body.append(&switcher);
    let metrics = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(4)
        .column_spacing(12)
        .row_spacing(8)
        .build();
    metrics.set_widget_name("detector-metrics");
    let status = metric(&metrics, "Detector status");
    let last = metric(&metrics, "Last event");
    let detected = metric(&metrics, "Events detected");
    let pending = metric(&metrics, "Pending events");
    pending.set_widget_name("detector-pending");
    body.append(&metrics);
    let detail = label("Loading configured accounts…", "dim-label");
    body.append(&detail);
    let extra_detail = label("", "dim-label");
    extra_detail.set_selectable(true);
    let extra = gtk::Expander::builder()
        .label("Polling diagnostics")
        .child(&extra_detail)
        .visible(false)
        .build();
    extra.set_widget_name("detector-polling-diagnostics");
    body.append(&extra);
    body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label("Event stream", "title-2");
    title.set_hexpand(true);
    title.set_tooltip_text(Some(
        "Durably captured observations. Pausing delivery keeps this stream and capture active.",
    ));
    heading.append(&title);
    let count = label("0 events", "dim-label");
    heading.append(&count);
    body.append(&heading);
    let stream = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    stream.set_widget_name("detector-event-stream");
    let events = gtk::Box::new(gtk::Orientation::Vertical, 12);
    events.set_margin_start(20);
    events.set_margin_end(20);
    events.set_margin_bottom(16);
    events.append(&stream);
    events.append(&label("Diagnostic delivery only · Shared delivery for Local and Remote · Initial observations are counted separately · Last 100 observations per source · No file synchronization is performed.", "dim-label"));
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&events)
        .build();
    scroller.set_widget_name("detector-event-scroll");
    scroller.set_margin_top(12);
    outer.append(&body);
    outer.append(&scroller);
    let view = View {
        status,
        last,
        detected,
        pending,
        count,
        stream,
        detail,
        extra_detail,
        extra,
        delivery_toggle: delivery_toggle.downgrade(),
        control_busy: control_busy.clone(),
    };
    let ids = Rc::new(RefCell::new(Vec::<String>::new()));
    let errors = Rc::new(RefCell::new(
        std::collections::HashMap::<String, String>::new(),
    ));
    let busy = Rc::new(Cell::new(false));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let accounts = accounts.downgrade();
        let ids = ids.clone();
        let errors = errors.clone();
        let view = view.clone();
        let busy = busy.clone();
        let feedback = feedback.clone();
        move || {
            if busy.replace(true) {
                return;
            }
            let context = context.clone();
            let accounts = accounts.clone();
            let ids = ids.clone();
            let errors = errors.clone();
            let view = view.clone();
            let busy = busy.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                let result = detectors::refresh(&context).await;
                busy.set(false);
                let Some(accounts) = accounts.upgrade() else {
                    return;
                };
                match result {
                    Ok(values) => {
                        let selected = ids.borrow().get(accounts.selected() as usize).cloned();
                        let names: Vec<_> = values
                            .iter()
                            .map(|(a, _)| {
                                format!(
                                    "{} · {}",
                                    a.display_name.as_deref().unwrap_or("Account"),
                                    a.email.as_deref().unwrap_or(&a.id.0)
                                )
                            })
                            .collect();
                        *errors.borrow_mut() = values
                            .iter()
                            .filter_map(|(account, error)| {
                                error
                                    .as_ref()
                                    .map(|error| (account.id.0.clone(), error.clone()))
                            })
                            .collect();
                        feedback.set_label("");
                        *ids.borrow_mut() = values.iter().map(|(a, _)| a.id.0.clone()).collect();
                        let strings: Vec<_> = names.iter().map(String::as_str).collect();
                        accounts.set_model(Some(&gtk::StringList::new(&strings)));
                        let index = selected
                            .and_then(|id| {
                                ids.borrow().iter().position(|candidate| *candidate == id)
                            })
                            .unwrap_or(0);
                        accounts.set_selected(if names.is_empty() {
                            gtk::INVALID_LIST_POSITION
                        } else {
                            index as u32
                        });
                        if names.is_empty() {
                            view.status.set_label("No configured accounts");
                            view.clear();
                        }
                    }
                    Err(error) => {
                        feedback.set_label(&error);
                    }
                }
            });
        }
    });
    refresh_button.connect_clicked({
        let refresh = refresh.clone();
        move |_| refresh()
    });
    if load_accounts {
        outer.connect_map({
            let refresh = refresh.clone();
            move |_| refresh()
        });
    } else {
        let mut keys: Vec<_> = context.detectors.borrow().keys().cloned().collect();
        keys.sort();
        let strings: Vec<_> = keys.iter().map(String::as_str).collect();
        accounts.set_model(Some(&gtk::StringList::new(&strings)));
        *ids.borrow_mut() = keys;
        accounts.set_selected(0);
    }
    let render_selection: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let ids = ids.clone();
        let errors = errors.clone();
        let accounts = accounts.downgrade();
        let tabs = tabs.downgrade();
        let view = view.clone();
        move || {
            let (Some(accounts), Some(tabs)) = (accounts.upgrade(), tabs.upgrade()) else {
                return;
            };
            let id = ids.borrow().get(accounts.selected() as usize).cloned();
            view.render(
                &context,
                id.as_deref(),
                tabs.visible_child_name().as_deref() == Some("remote"),
                id.as_ref()
                    .and_then(|id| errors.borrow().get(id).cloned())
                    .as_deref(),
            );
        }
    });
    delivery_toggle.connect_clicked({
        let context = context.clone();
        let ids = ids.clone();
        let accounts = accounts.downgrade();
        let render = render_selection.clone();
        let feedback = feedback.downgrade();
        let selected_tabs = tabs.downgrade();
        move |button| {
            if control_busy.replace(true) {
                return;
            }
            let id = accounts
                .upgrade()
                .and_then(|accounts| ids.borrow().get(accounts.selected() as usize).cloned());
            let session = id.as_ref().and_then(|id| {
                context
                    .detectors
                    .borrow()
                    .get(id)
                    .map(|session| session.orchestrator.clone())
            });
            let Some(session) = session else {
                control_busy.set(false);
                return;
            };
            button.set_sensitive(false);
            let remote = selected_tabs
                .upgrade()
                .is_some_and(|tabs| tabs.visible_child_name().as_deref() == Some("remote"));
            let action_id = id.clone();
            let selected_accounts = accounts.clone();
            let selected_tabs = selected_tabs.clone();
            let selected_ids = ids.clone();
            let control_busy = control_busy.clone();
            let render = render.clone();
            let feedback = feedback.clone();
            if let Some(feedback) = feedback.upgrade() {
                feedback.set_label("");
            }
            gtk::glib::spawn_future_local(async move {
                let result = gtk::gio::spawn_blocking(move || {
                    if session.diagnostics().delivery
                        == Some(drivesync_engine::detectors::DeliveryState::Paused)
                    {
                        session.resume_delivery()
                    } else {
                        session.pause_delivery()
                    }
                })
                .await;
                control_busy.set(false);
                let selected = selected_accounts.upgrade().and_then(|accounts| {
                    selected_ids
                        .borrow()
                        .get(accounts.selected() as usize)
                        .cloned()
                });
                let same_source = selected_tabs.upgrade().is_some_and(|tabs| {
                    (tabs.visible_child_name().as_deref() == Some("remote")) == remote
                });
                if let Some(feedback) = feedback
                    .upgrade()
                    .filter(|_| selected == action_id && same_source)
                {
                    match result {
                        Ok(Ok(())) => (),
                        Ok(Err(error)) => feedback.set_label(&error.to_string()),
                        Err(_) => feedback.set_label("Could not change detector delivery"),
                    }
                }
                render();
            });
        }
    });
    accounts.connect_selected_notify({
        let render = render_selection.clone();
        let feedback = feedback.downgrade();
        move |_| {
            if let Some(feedback) = feedback.upgrade() {
                feedback.set_label("");
            }
            render();
        }
    });
    tabs.connect_visible_child_name_notify({
        let feedback = feedback.downgrade();
        move |_| {
            if let Some(feedback) = feedback.upgrade() {
                feedback.set_label("");
            }
            render_selection();
        }
    });
    // UI refresh reads snapshots only; it never schedules detector scans or remote polling.
    let mut previous = String::new();
    let weak = outer.downgrade();
    let context = context.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(widget) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        if widget.is_mapped() && !busy.get() {
            let id = ids.borrow().get(accounts.selected() as usize).cloned();
            let remote = tabs.visible_child_name().as_deref() == Some("remote");
            let error = id.as_ref().and_then(|id| errors.borrow().get(id).cloned());
            let key = {
                let sessions = context.detectors.borrow();
                let session = id.as_ref().and_then(|id| sessions.get(id));
                format!(
                    "{id:?}/{remote}/{error:?}/{:?}/{:?}/{:?}",
                    session.map(|s| {
                        let snapshot = s.orchestrator.diagnostics();
                        (
                            snapshot.events_detected,
                            snapshot.pending_events,
                            snapshot.confirmed_events,
                            snapshot.delivery,
                            snapshot.queue_error,
                        )
                    }),
                    session.map(|s| s.orchestrator.local_status()),
                    session.map(|s| s.orchestrator.remote_diagnostics().map(|snapshot| (
                        snapshot.status,
                        snapshot.queue.detected,
                        snapshot.queue.pending,
                        snapshot.queue.leased,
                        snapshot.queue.confirmed,
                        snapshot.queue.delivery,
                        snapshot.queue.last_error
                    )))
                )
            };
            if key != previous {
                view.render(&context, id.as_deref(), remote, error.as_deref());
                previous = key;
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    outer
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
