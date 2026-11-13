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
fn metric(grid: &gtk::Grid, title: &str, column: i32, row: i32) -> gtk::Label {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 8);
    block.set_hexpand(true);
    block.append(&label(title, "dim-label"));
    let value = label("—", "title-2");
    block.append(&value);
    grid.attach(&block, column, row, 1, 1);
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
fn description(event: &DetectorEventSummary) -> String {
    match (&event.event.previous, &event.event.current) {
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
}
impl View {
    fn clear(&self) {
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
            self.status.set_label("Not implemented");
            self.detail.set_label("Remote scheduling and polling diagnostics will be provided by the engine. No remote detector is running.");
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
        if let Ok(status) = session
            .orchestrator
            .status(drivesync_engine::detectors::LocalStatusRequest {})
        {
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
        self.last.set_label(
            &snapshot
                .last_event_at
                .map(time)
                .unwrap_or_else(|| "—".into()),
        );
        self.detected
            .set_label(&snapshot.events_detected.to_string());
        self.pending.set_label(
            &snapshot
                .pending_events
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        );
        self.pending.set_tooltip_text(Some("The durable event queue is not implemented. Diagnostic history is not a pending-work queue."));
        self.count
            .set_label(&format!("{} events", snapshot.events.len()));
        for event in snapshot.events {
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
    let body = gtk::Box::new(gtk::Orientation::Vertical, 24);
    for edge in [
        gtk::PositionType::Left,
        gtk::PositionType::Right,
        gtk::PositionType::Top,
        gtk::PositionType::Bottom,
    ] {
        match edge {
            gtk::PositionType::Left => body.set_margin_start(24),
            gtk::PositionType::Right => body.set_margin_end(24),
            gtk::PositionType::Top => body.set_margin_top(24),
            _ => body.set_margin_bottom(24),
        }
    }
    body.append(&label("Developer / Detector", "title-1"));
    let account_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_hexpand(true);
    accounts.set_tooltip_text(Some("Account whose detector is being inspected"));
    accounts.set_widget_name("detector-account");
    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Reload accounts and configured folders")
        .build();
    account_row.append(&accounts);
    account_row.append(&refresh_button);
    body.append(&account_row);
    let feedback = label("", "error");
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
    body.append(&switcher);
    body.append(&tabs);
    let grid = gtk::Grid::builder()
        .column_homogeneous(true)
        .column_spacing(24)
        .row_spacing(28)
        .build();
    let status = metric(&grid, "Detector status", 0, 0);
    let last = metric(&grid, "Last event", 1, 0);
    let detected = metric(&grid, "Events detected", 0, 1);
    let pending = metric(&grid, "Pending events", 1, 1);
    body.append(&grid);
    let detail = label("Loading configured accounts…", "dim-label");
    body.append(&detail);
    body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label("Event stream", "title-2");
    title.set_hexpand(true);
    heading.append(&title);
    let count = label("0 events", "dim-label");
    heading.append(&count);
    body.append(&heading);
    let stream = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    body.append(&stream);
    body.append(&label("Observation only · Last 100 events per account · History resets when its session is replaced or the app exits. No file synchronization is performed.", "dim-label"));
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();
    outer.append(&scroller);
    let view = View {
        status,
        last,
        detected,
        pending,
        count,
        stream,
        detail,
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
    accounts.connect_selected_notify({
        let render = render_selection.clone();
        move |_| render()
    });
    tabs.connect_visible_child_name_notify(move |_| render_selection());
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
                    "{id:?}/{remote}/{error:?}/{:?}/{:?}",
                    session.map(|s| s.orchestrator.diagnostics().events_detected),
                    session.map(|s| s
                        .orchestrator
                        .status(drivesync_engine::detectors::LocalStatusRequest {}))
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
