//! Application preparation snapshots, independent of console visibility and without SQL or scanning in widgets.
use crate::application::{detectors, Context};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    orchestrator::{PreparationFailure, PreparationState},
    scanner::{ScanIssueKind, ScanIssueLocation, ScannerState},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build()
}
fn clock(time: Option<drivesync_engine::Timestamp>) -> String {
    time.and_then(|t| gtk::glib::DateTime::from_unix_local(t.unix_millis / 1000).ok())
        .and_then(|t| t.format("%H:%M:%S").ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "—".into())
}
fn metric(row: &gtk::FlowBox, title: &str) -> gtk::Label {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
    box_.set_size_request(130, -1);
    box_.append(&label(title));
    let value = label("—");
    value.add_css_class("title-2");
    box_.append(&value);
    row.insert(&box_, -1);
    value
}
fn issue_name(kind: ScanIssueKind) -> &'static str {
    match kind {
        ScanIssueKind::AccessDenied => "Access denied",
        ScanIssueKind::ReadFailed => "Could not read entry",
        ScanIssueKind::MetadataUnavailable => "Metadata incomplete",
        ScanIssueKind::EntryDisappeared => "Entry disappeared",
        ScanIssueKind::EntryChanged => "Entry changed during scan",
        ScanIssueKind::UnsupportedEntryType => "Unsupported entry type",
        ScanIssueKind::SymlinkProblem => "Could not inspect symbolic link",
        ScanIssueKind::ResourceExhausted => "Resource limit reached",
        ScanIssueKind::InventoryRecordFailed => "Could not record observation",
        ScanIssueKind::MountBoundary => "Mount point not scanned",
    }
}
struct View {
    root: gtk::glib::WeakRef<gtk::Box>,
    context: Context,
    accounts: gtk::DropDown,
    ids: RefCell<Vec<String>>,
    busy: Cell<bool>,
    load_error: RefCell<Option<String>>,
    refresh: gtk::Button,
    rescan: gtk::Button,
    status: gtk::Label,
    gate: gtk::Label,
    path: gtk::Label,
    times: gtk::Label,
    current: gtk::Label,
    feedback: gtk::Label,
    files: gtk::Label,
    directories: gtk::Label,
    recorded: gtk::Label,
    issues: gtk::Label,
    issue_title: gtk::Label,
    list: gtk::ListBox,
    spinner: gtk::Spinner,
    last_report: Cell<usize>,
}
impl View {
    fn render(&self) {
        let id = self
            .ids
            .borrow()
            .get(self.accounts.selected() as usize)
            .cloned();
        let sessions = self.context.detectors.borrow();
        let session = id.as_ref().and_then(|id| sessions.get(id));
        self.rescan.set_sensitive(false);
        self.spinner.stop();
        self.status.remove_css_class("success");
        self.status.remove_css_class("error");
        for metric in [&self.files, &self.directories, &self.recorded, &self.issues] {
            metric.set_label("—");
        }
        self.times.set_label("");
        self.current.set_label("");
        self.path.set_label("");
        let Some(session) = session else {
            self.status.set_label("Unavailable");
            self.gate.set_label("Analysis is suspended.");
            let message=self.load_error.borrow().clone().or_else(||id.as_ref().and_then(|id|self.context.preparation_errors.borrow().get(id).cloned())).unwrap_or_else(||"Select a configured account. Preparation starts automatically when the application opens.".into());
            self.feedback.set_label(&message);
            self.clear_issues();
            self.visibility();
            return;
        };
        self.feedback
            .set_label(self.load_error.borrow().as_deref().unwrap_or(""));
        self.path.set_label(&session.root.display().to_string());
        let snapshot = session.orchestrator.preparation_status();
        let working = matches!(
            snapshot.state,
            PreparationState::Starting
                | PreparationState::Scanning
                | PreparationState::Analyzing
                | PreparationState::ProcessingEvents
        ) || snapshot
            .scanner
            .as_ref()
            .is_some_and(|s| s.state == ScannerState::Running);
        self.rescan
            .set_sensitive(!working && !self.busy.get() && !self.context.shutting_down.get());
        self.spinner.set_spinning(working);
        let state = match snapshot.state {
            PreparationState::Idle => "Waiting to start",
            PreparationState::Starting => "Starting capture",
            PreparationState::Scanning => "Scanning",
            PreparationState::Analyzing => "Recording decisions",
            PreparationState::ProcessingEvents => "Processing changes",
            PreparationState::ReadyForAnalyzer => "Scan complete",
            PreparationState::Partial => "Partial scan",
            PreparationState::Failed => "Preparation failed",
            PreparationState::Cancelled => "Scan cancelled",
            PreparationState::Stopped => "Stopped",
        };
        self.status.set_label(state);
        if snapshot.state == PreparationState::ReadyForAnalyzer {
            self.status.add_css_class("success");
            self.gate.set_label("Local scan complete. New observations are delivered to the Analyzer; file operations are not executed.");
        } else if matches!(
            snapshot.state,
            PreparationState::Analyzing | PreparationState::ProcessingEvents
        ) {
            self.gate
                .set_label("Recording analysis decisions. No file operations are executed.");
        } else {
            self.gate.set_label("Analysis suspended. A complete local scan is required; events remain stored in the detector queue.");
        }
        if matches!(
            snapshot.state,
            PreparationState::Partial | PreparationState::Failed
        ) {
            self.status.add_css_class("error");
        }
        if let Some(error) = snapshot.error {
            self.feedback.set_label(&match error {
                PreparationFailure::Inventory(e) => format!(
                    "Inventory could not be opened ({:?}). Existing data has been preserved.",
                    e.kind
                ),
                other => other.to_string(),
            });
        }
        if let Some(scan) = snapshot.scanner {
            self.files.set_label(&scan.files_observed.to_string());
            self.directories
                .set_label(&scan.directories_observed.to_string());
            self.recorded.set_label(&scan.items_recorded.to_string());
            self.issues.set_label(&scan.issues_recorded.to_string());
            self.times.set_label(&format!(
                "Started: {} · Finished: {}",
                clock(scan.started_at),
                clock(scan.finished_at)
            ));
            self.current.set_label(
                &scan
                    .current_directory
                    .map(|p| format!("Current folder: {}", String::from_utf8_lossy(p.as_bytes())))
                    .unwrap_or_else(|| {
                        if working {
                            "Current folder: monitored root".into()
                        } else {
                            String::new()
                        }
                    }),
            );
        }
        let key = snapshot
            .report
            .as_ref()
            .map_or(0, |report| std::sync::Arc::as_ptr(report) as usize);
        if key != self.last_report.get() {
            self.clear_issues();
            self.last_report.set(key);
            if let Some(report) = snapshot.report {
                self.issue_title.set_label(&format!(
                    "Problems · {}{}",
                    report.issues.len(),
                    if report.issues.len() > 100 {
                        " · showing first 100"
                    } else {
                        ""
                    }
                ));
                for issue in report.issues.iter().take(100) {
                    let path=match &issue.location{ScanIssueLocation::ScanRoot=>"Monitored folder".into(),ScanIssueLocation::Item(id)=>id.0.clone(),ScanIssueLocation::RelativePath(p)=>std::str::from_utf8(p.as_bytes()).map(str::to_owned).unwrap_or_else(|_|format!("{:?}",<std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(p.as_bytes().to_vec())))};
                    let row = adw::ActionRow::builder()
                        .title(issue_name(issue.kind))
                        .subtitle(&path)
                        .subtitle_lines(2)
                        .build();
                    row.set_use_markup(false);
                    self.list.append(&row);
                }
                if report.issues.is_empty() {
                    self.list.append(
                        &adw::ActionRow::builder()
                            .title("No problems reported.")
                            .build(),
                    );
                }
            }
        }
        if key == 0 {
            self.issue_title.set_label("Problems");
            let message = if working {
                "Detailed problems will be available when this scan finishes."
            } else {
                "No completed report available."
            };
            if let Some(text) = self
                .list
                .first_child()
                .and_downcast::<gtk::ListBoxRow>()
                .and_then(|row| row.child())
                .and_downcast::<gtk::Label>()
            {
                text.set_label(message);
            } else if self.list.first_child().is_none() {
                self.list.append(&label(message));
            }
        }
        self.visibility();
    }
    fn visibility(&self) {
        for text in [&self.feedback, &self.current, &self.times, &self.path] {
            text.set_visible(!text.text().is_empty());
        }
    }
    fn clear_issues(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.last_report.set(0);
        self.issue_title.set_label("Problems");
    }
    fn refresh(self: &Rc<Self>) {
        if self.busy.replace(true) {
            return;
        }
        self.refresh.set_sensitive(false);
        self.accounts.set_sensitive(false);
        self.rescan.set_sensitive(false);
        let weak = Rc::downgrade(self);
        let context = self.context.clone();
        gtk::glib::spawn_future_local(async move {
            let result = detectors::refresh(&context).await;
            let Some(view) = weak.upgrade().filter(|v| v.root.upgrade().is_some()) else {
                return;
            };
            let selected = view
                .ids
                .borrow()
                .get(view.accounts.selected() as usize)
                .cloned();
            match result {
                Ok(values) => {
                    let ids = values
                        .iter()
                        .map(|(a, _)| a.id.0.clone())
                        .collect::<Vec<_>>();
                    let names = values
                        .iter()
                        .map(|(a, _)| {
                            format!(
                                "{} · {}",
                                a.display_name.as_deref().unwrap_or("Account"),
                                a.email.as_deref().unwrap_or(&a.id.0)
                            )
                        })
                        .collect::<Vec<_>>();
                    let strings = names.iter().map(String::as_str).collect::<Vec<_>>();
                    let selected = selected
                        .and_then(|id| ids.iter().position(|v| v == &id))
                        .unwrap_or(0);
                    let empty = ids.is_empty();
                    *view.ids.borrow_mut() = ids;
                    view.accounts
                        .set_model(Some(&gtk::StringList::new(&strings)));
                    view.accounts.set_selected(if empty {
                        gtk::INVALID_LIST_POSITION
                    } else {
                        selected as u32
                    });
                    *view.load_error.borrow_mut() = None;
                }
                Err(error) => {
                    *view.load_error.borrow_mut() = Some(error);
                }
            }
            view.busy.set(false);
            view.refresh.set_sensitive(true);
            view.accounts.set_sensitive(!view.ids.borrow().is_empty());
            view.render();
        });
    }
}
pub(in crate::ui::developer) fn build(context: &Context) -> gtk::Box {
    let (root, view) = create(context);
    root.connect_map(move |_| view.refresh());
    root
}
fn create(context: &Context) -> (gtk::Box, Rc<View>) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_start(20)
        .margin_end(20)
        .margin_top(20)
        .margin_bottom(20)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();
    root.append(&scroll);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_widget_name("scanner-account");
    accounts.set_hexpand(true);
    accounts.update_property(&[gtk::accessible::Property::Label("Scanner account")]);
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .hexpand(true)
                .build(),
        ));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let text = item.item().and_downcast::<gtk::StringObject>().unwrap();
        item.child()
            .and_downcast::<gtk::Label>()
            .unwrap()
            .set_label(&text.string());
    });
    accounts.set_factory(Some(&factory));
    let refresh = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh configured accounts")
        .build();
    refresh.set_widget_name("scanner-refresh");
    row.append(&accounts);
    row.append(&refresh);
    body.append(&row);
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let status = label("Waiting to start");
    status.set_widget_name("scanner-status");
    status.add_css_class("title-2");
    status.set_hexpand(true);
    let spinner = gtk::Spinner::new();
    heading.append(&status);
    heading.append(&spinner);
    let rescan = gtk::Button::with_label("Scan again");
    rescan.set_widget_name("scanner-rescan");
    heading.append(&rescan);
    body.append(&heading);
    let gate = label("Analysis suspended.");
    gate.set_widget_name("scanner-gate");
    body.append(&gate);
    let path = label("");
    path.add_css_class("dim-label");
    path.set_selectable(true);
    body.append(&path);
    let metrics = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(20)
        .row_spacing(14)
        .min_children_per_line(1)
        .max_children_per_line(4)
        .homogeneous(true)
        .build();
    metrics.set_widget_name("scanner-metrics");
    let files = metric(&metrics, "Files observed");
    let directories = metric(&metrics, "Folders observed");
    let recorded = metric(&metrics, "Observations recorded");
    let issues = metric(&metrics, "Problems");
    body.append(&metrics);
    let times = label("");
    times.add_css_class("dim-label");
    body.append(&times);
    let current = label("");
    body.append(&current);
    let feedback = label("");
    feedback.set_widget_name("scanner-feedback");
    feedback.add_css_class("error");
    body.append(&feedback);
    let issue_title = label("Problems");
    issue_title.add_css_class("title-3");
    body.append(&issue_title);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.set_widget_name("scanner-problems");
    body.append(&list);
    let note=label("The Scanner observes local files and updates the inventory. It does not transfer files. Problems shown here belong to this application's latest scan.");
    note.add_css_class("dim-label");
    body.append(&note);
    let view = Rc::new(View {
        root: root.downgrade(),
        context: context.clone(),
        accounts: accounts.clone(),
        ids: RefCell::new(Vec::new()),
        busy: Cell::new(false),
        load_error: RefCell::new(None),
        refresh: refresh.clone(),
        rescan: rescan.clone(),
        status,
        gate,
        path,
        times,
        current,
        feedback,
        files,
        directories,
        recorded,
        issues,
        issue_title,
        list,
        spinner,
        last_report: Cell::new(0),
    });
    accounts.connect_selected_notify({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                if !view.busy.get() {
                    view.clear_issues();
                    view.render();
                }
            }
        }
    });
    refresh.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                view.refresh();
            }
        }
    });
    rescan.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let id = view
                    .ids
                    .borrow()
                    .get(view.accounts.selected() as usize)
                    .cloned();
                if let Some(id) = id {
                    if let Err(error) = detectors::rescan(&view.context, &id) {
                        *view.load_error.borrow_mut() = Some(error);
                    }
                    view.render();
                }
            }
        }
    });
    let weak = Rc::downgrade(&view);
    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(view) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        let Some(root) = view.root.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        if root.is_mapped() && !view.busy.get() {
            view.render();
        }
        gtk::glib::ControlFlow::Continue
    });
    view.render();
    (root, view)
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
