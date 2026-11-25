//! GTK ownership and callbacks follow the existing dashboard wrapper convention.
use super::models::*;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};

fn label() -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .selectable(false)
        .build()
}
/// A single surface for account status and its current operation.
pub(super) fn feedback_card(status: &SyncStatus, activity: &SyncActivity) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_widget_name("dashboard-synchronization-feedback");
    status.labels.append(&activity.widget);
    card.append(&status.list);
    card
}

type DetailsCallback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

#[derive(Clone)]
/// Selected-scope summary. Only an audited backend may supply `Synced`.
pub struct SyncStatus {
    #[cfg(test)]
    pub widget: gtk::Box,
    list: gtk::ListBox,
    details_callback: DetailsCallback,
    labels: gtk::Box,
    row: gtk::ListBoxRow,
    chevron: gtk::Image,
    icon: gtk::Image,
    title: gtk::Label,
    description: gtk::Label,
    state: Rc<RefCell<StatusView>>,
}
impl SyncStatus {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let icon = gtk::Image::from_icon_name("view-refresh-symbolic");
        icon.set_pixel_size(24);
        icon.set_size_request(32, 26);
        icon.set_valign(gtk::Align::Start);
        widget.append(&icon);
        let title = label();
        title.add_css_class("heading");
        title.set_widget_name("dashboard-synchronization-status");
        let description = label();
        description.add_css_class("dim-label");
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
        labels.set_valign(gtk::Align::Center);
        labels.set_hexpand(true);
        labels.append(&title);
        labels.append(&description);
        widget.append(&labels);
        widget.set_hexpand(true);
        let chevron = gtk::Image::from_icon_name("go-next-symbolic");
        chevron.set_valign(gtk::Align::Center);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_top(12);
        header.set_margin_bottom(12);
        header.set_margin_start(14);
        header.set_margin_end(14);
        header.append(&widget);
        header.append(&chevron);
        let row = gtk::ListBoxRow::builder()
            .child(&header)
            .selectable(false)
            .build();
        row.set_widget_name("dashboard-synchronization-details");
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        list.append(&row);
        let details_callback: DetailsCallback = Rc::default();
        list.connect_row_activated({
            let callback = details_callback.clone();
            move |_, row| {
                if row.is_activatable() {
                    if let Some(callback) = callback.borrow().as_ref() {
                        callback();
                    }
                }
            }
        });
        let value = Self {
            #[cfg(test)]
            widget,
            list,
            details_callback,
            labels,
            row,
            chevron,
            icon,
            title,
            description,
            state: Rc::new(RefCell::new(StatusView::Initializing)),
        };
        value.reset();
        value
    }
    pub fn set_state(&self, state: StatusView, description: &str) {
        let title = match state {
            StatusView::Initializing => "Preparing synchronization…",
            StatusView::Waiting => "Waiting for changes",
            StatusView::WaitingForEvidence => "Waiting for verification",
            StatusView::Recovering => "Checking interrupted operations…",
            StatusView::Synchronizing => "Synchronizing…",
            StatusView::Synced => "Everything synchronized",
            StatusView::Attention => "Synchronization needs attention",
            StatusView::Offline => "Offline",
            StatusView::Failed => "Synchronization failed.",
            StatusView::Stopped => "Synchronization stopped.",
        };
        self.icon.set_icon_name(Some(match state {
            StatusView::Synced => "emblem-ok-symbolic",
            StatusView::Attention | StatusView::Failed => "dialog-warning-symbolic",
            StatusView::Offline => "network-offline-symbolic",
            StatusView::Waiting | StatusView::WaitingForEvidence => "document-open-recent-symbolic",
            _ => "view-refresh-symbolic",
        }));
        for class in ["accent", "success", "warning", "error", "dim-label"] {
            self.icon.remove_css_class(class);
        }
        self.icon.add_css_class(match state {
            StatusView::Synced => "success",
            StatusView::Attention => "warning",
            StatusView::Failed => "error",
            StatusView::Waiting
            | StatusView::WaitingForEvidence
            | StatusView::Offline
            | StatusView::Stopped => "dim-label",
            _ => "accent",
        });
        self.title.set_label(&gettext(title));
        self.description.set_label(description);
        self.description.set_visible(!description.is_empty());
        let actionable = matches!(
            state,
            StatusView::Attention | StatusView::Failed | StatusView::Offline
        );
        self.chevron.set_visible(actionable);
        self.row.set_activatable(actionable);
        self.row.set_focusable(actionable);
        self.row
            .set_cursor_from_name(actionable.then_some("pointer"));
        self.row.set_tooltip_text(
            actionable
                .then(|| gettext("Synchronization details"))
                .as_deref(),
        );
        *self.state.borrow_mut() = state;
    }
    pub fn connect_details(&self, callback: impl Fn() + 'static) {
        *self.details_callback.borrow_mut() = Some(Box::new(callback));
    }
    pub fn diagnostic(&self) -> (String, String) {
        (self.title.text().into(), self.description.text().into())
    }
    pub fn state(&self) -> StatusView {
        self.state.borrow().clone()
    }
    pub fn reset(&self) {
        self.set_state(StatusView::Initializing, "");
    }
}
#[derive(Clone)]
/// Current operation only; clearing activity never asserts convergence.
pub struct SyncActivity {
    pub widget: gtk::Box,
    text: gtk::Label,
    details: gtk::Label,
    progress: gtk::ProgressBar,
    spinner: gtk::Spinner,
    state: Rc<RefCell<Option<ActivityView>>>,
}
impl SyncActivity {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let text = label();
        text.set_lines(3);
        text.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let details = label();
        let spinner = gtk::Spinner::new();
        details.add_css_class("dim-label");
        details.add_css_class("caption");
        let operation = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        operation.append(&spinner);
        text.set_hexpand(true);
        operation.append(&text);
        let progress = gtk::ProgressBar::new();
        progress.set_show_text(true);
        progress.set_widget_name("dashboard-synchronization-progress");
        widget.append(&operation);
        widget.append(&details);
        widget.append(&progress);
        widget.set_visible(false);
        Self {
            widget,
            text,
            details,
            progress,
            spinner,
            state: Rc::default(),
        }
    }
    pub fn set_activity(&self, mut activity: ActivityView) {
        let rename = matches!(activity.kind, ActivityKind::Rename { .. });
        let movement = matches!(activity.kind, ActivityKind::Move { .. });
        let directory_creation = activity.kind == ActivityKind::CreateDirectory;
        let workspace_shortcut = matches!(
            activity.kind,
            ActivityKind::CreateWorkspaceShortcut | ActivityKind::UpdateWorkspaceShortcut
        );
        let trash = activity.kind == ActivityKind::Trash;
        if rename || movement || directory_creation || workspace_shortcut || trash {
            activity.progress = ProgressView::None;
        }
        let verb = match activity.kind {
            ActivityKind::Download | ActivityKind::DownloadDiscoveredRemote => "Downloading…",
            ActivityKind::UpdateRemoteContent => "Updating Drive copy…",
            ActivityKind::UpdateLocalContent => "Updating local copy…",
            ActivityKind::Upload | ActivityKind::UploadDiscoveredLocal => "Uploading…",
            ActivityKind::Rename { .. } => "Renaming…",
            ActivityKind::Move { .. } => "Moving…",

            ActivityKind::Trash => "Moving to trash…",
            ActivityKind::CreateWorkspaceShortcut => "Creating Workspace shortcut…",
            ActivityKind::UpdateWorkspaceShortcut => "Updating Workspace shortcut…",
            ActivityKind::CreateDirectory => "Creating directory…",
            ActivityKind::Recover => "Checking interrupted operation…",
            ActivityKind::Verify => "Verifying content…",
            ActivityKind::Apply => "Applying changes…",
        };
        let direction = match activity.direction {
            Direction::RemoteToLocal => "Drive → Computer",
            Direction::LocalToRemote => "Computer → Drive",
            Direction::None => "",
        };
        let name = match &activity.kind {
            ActivityKind::Rename { previous_name } => {
                format!("{previous_name} → {}", activity.name)
            }
            ActivityKind::Move {
                source_parent,
                destination_parent,
            } => {
                format!("{} ({source_parent} → {destination_parent})", activity.name)
            }
            _ => activity.name.clone(),
        };
        let phase = match activity.phase {
            Phase::Running if activity.kind == ActivityKind::DownloadDiscoveredRemote => {
                "New file discovered on Drive"
            }
            Phase::Running if activity.kind == ActivityKind::UploadDiscoveredLocal => {
                "New file discovered on computer"
            }
            Phase::Running if activity.kind == ActivityKind::UpdateLocalContent => {
                "Downloading and verifying content"
            }
            Phase::Running if activity.kind == ActivityKind::UpdateRemoteContent => {
                "Uploading and verifying content"
            }
            Phase::Confirmed if activity.kind == ActivityKind::UpdateRemoteContent => {
                "Drive update verified"
            }
            Phase::Confirmed if activity.kind == ActivityKind::CreateWorkspaceShortcut => {
                "Workspace shortcut creation verified"
            }
            Phase::Confirmed if workspace_shortcut => "Workspace shortcut update verified",
            Phase::Confirmed if trash => "Move to trash verified",
            Phase::Confirmed if directory_creation => "Directory creation verified",
            Phase::Confirmed if movement => "Move verified",
            Phase::Confirmed if rename => "Rename verified",
            Phase::Confirmed => "Local update verified",
            Phase::Running => "",
            Phase::Failed => "Operation failed",
        };
        let (headline, phase) = if activity.phase == Phase::Running {
            (
                format!("{} · {name}", gettext(verb).trim_end_matches('…')),
                phase,
            )
        } else {
            (format!("{} · {name}", gettext(phase)), "")
        };
        self.text.set_label(&headline);
        let details = [direction, phase]
            .into_iter()
            .filter(|label| !label.is_empty())
            .map(gettext)
            .collect::<Vec<_>>()
            .join(" · ");
        self.details.set_label(&details);
        self.details.set_visible(!details.is_empty());
        self.render_progress(&activity.progress);
        *self.state.borrow_mut() = Some(activity);
        self.widget.set_visible(true);
    }
    fn render_progress(&self, value: &ProgressView) {
        let measured = value.measured();
        self.progress.set_visible(measured.is_some());
        let indeterminate = measured.is_none() && *value != ProgressView::None;
        self.spinner.set_visible(indeterminate);
        self.spinner.set_spinning(indeterminate);
        if let Some((done, total)) = measured {
            self.progress.set_fraction(done as f64 / total as f64);
            let template = match value {
                ProgressView::Bytes { .. } => "{completed} of {total} bytes",
                _ => "{completed} of {total} confirmed steps for this item",
            };
            let measurement = gettext(template)
                .replace("{completed}", &done.to_string())
                .replace("{total}", &total.to_string());
            let percent = u128::from(done) * 100 / u128::from(total);
            self.progress.set_text(Some(
                &gettext("{measurement} · {percent}%")
                    .replace("{measurement}", &measurement)
                    .replace("{percent}", &percent.to_string()),
            ));
        } else {
            self.progress.set_fraction(0.0);
            self.progress.set_text(None);
        }
    }
    /// Incremental telemetry requires an exact scope and a published operation identity.
    pub fn update_progress(&self, scope: &str, operation: &str, value: ProgressView) -> bool {
        let Some(mut state) = self.state() else {
            return false;
        };
        if state.scope != scope
            || state.operation_id.as_deref() != Some(operation)
            || matches!(
                state.kind,
                ActivityKind::Rename { .. }
                    | ActivityKind::Move { .. }
                    | ActivityKind::CreateDirectory
                    | ActivityKind::CreateWorkspaceShortcut
                    | ActivityKind::UpdateWorkspaceShortcut
                    | ActivityKind::Trash
            )
        {
            return false;
        }
        state.progress = value;
        self.set_activity(state);
        true
    }
    pub fn clear(&self) {
        self.state.borrow_mut().take();
        self.widget.set_visible(false);
        self.render_progress(&ProgressView::Indeterminate);
        self.spinner.stop();
    }
    pub fn state(&self) -> Option<ActivityView> {
        self.state.borrow().clone()
    }
}
type AttentionCallback = Rc<RefCell<Option<Box<dyn Fn(AttentionAction)>>>>;
#[derive(Clone)]
/// Deduplicated current issues and UI intentions, without synchronization I/O.
pub struct SyncAttention {
    pub widget: gtk::Box,
    items: Rc<RefCell<Vec<AttentionItemView>>>,
    callback: AttentionCallback,
}
impl SyncAttention {
    pub fn new() -> Self {
        Self {
            widget: gtk::Box::new(gtk::Orientation::Vertical, 6),
            items: Rc::default(),
            callback: Rc::default(),
        }
    }
    pub fn set_items(&self, items: Vec<AttentionItemView>) {
        self.clear();
        let mut unique = Vec::new();
        for item in items {
            if unique
                .iter()
                .any(|old: &AttentionItemView| old.scope == item.scope && old.id == item.id)
            {
                continue;
            }
            let button = gtk::Button::new();
            let text = label();
            text.set_label(&format!("{}\n{}", item.title, item.summary));
            button.set_child(Some(&text));
            let action = item.action.clone();
            let callback = self.callback.clone();
            button.connect_clicked(move |_| {
                if let Some(callback) = callback.borrow().as_ref() {
                    callback(action.clone());
                }
            });
            self.widget.append(&button);
            unique.push(item);
        }
        if !unique.is_empty() {
            let heading = label();
            heading.set_label(
                &gettext("{count} items need attention")
                    .replace("{count}", &unique.len().to_string()),
            );
            self.widget.prepend(&heading);
        }
        self.widget.set_visible(!unique.is_empty());
        *self.items.borrow_mut() = unique;
    }
    pub fn clear(&self) {
        while let Some(child) = self.widget.first_child() {
            self.widget.remove(&child);
        }
        self.items.borrow_mut().clear();
        self.widget.set_visible(false);
    }
    pub fn items(&self) -> Vec<AttentionItemView> {
        self.items.borrow().clone()
    }
    pub fn issue(&self, id: &str) -> Option<AttentionItemView> {
        self.items
            .borrow()
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }
    pub fn connect_activated(&self, callback: impl Fn(AttentionAction) + 'static) {
        *self.callback.borrow_mut() = Some(Box::new(callback));
    }
}
#[derive(Clone)]
/// Informational diagnosis; exposes no retry or destructive commands.
pub struct SyncIssueDialog {
    pub window: gtk::Window,
    text: gtk::Label,
    issue: Rc<RefCell<Option<IssueView>>>,
}
impl SyncIssueDialog {
    pub fn new() -> Self {
        let text = label();
        text.set_margin_top(18);
        text.set_margin_bottom(18);
        text.set_margin_start(18);
        text.set_margin_end(18);
        let window = gtk::Window::builder()
            .modal(true)
            .default_width(480)
            .default_height(280)
            .build();
        window.set_child(Some(&gtk::ScrolledWindow::builder().child(&text).build()));
        Self {
            window,
            text,
            issue: Rc::default(),
        }
    }
    pub fn state(&self) -> Option<IssueView> {
        self.issue.borrow().clone()
    }
    pub fn set_issue(&self, issue: IssueView) {
        *self.issue.borrow_mut() = Some(issue.clone());
        self.window.set_title(Some(&issue.title));
        self.text.set_label(&format!(
            "{}\n{}\n{}",
            gettext(match issue.severity {
                Severity::Info => "Information",
                Severity::Warning => "Warning",
                Severity::Error => "Error",
            }),
            issue.description,
            issue.guidance.unwrap_or_default()
        ));
    }
    pub fn present(&self) {
        self.window.present();
    }
    pub fn close(&self) {
        self.window.close();
    }
}
type ChoiceCallback = Rc<RefCell<Option<Box<dyn Fn(ConflictChoiceRequest)>>>>;
#[derive(Clone)]
/// Revision-bound, explicitly confirmed choice; submission never implies resolution.
pub struct ConflictResolver {
    pub window: gtk::Window,
    text: gtk::Label,
    feedback: gtk::Label,
    pub local: gtk::Button,
    pub remote: gtk::Button,
    pub confirm: gtk::Button,
    pub later: gtk::Button,
    conflict: Rc<RefCell<Option<ConflictView>>>,
    state: Rc<RefCell<ConflictResolverState>>,
    choice: Rc<RefCell<Option<ConflictChoice>>>,
    callback: ChoiceCallback,
}
impl ConflictResolver {
    pub fn new() -> Self {
        let window = gtk::Window::builder()
            .modal(true)
            .default_width(560)
            .default_height(540)
            .title(gettext("Synchronization conflicts"))
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for edge in [
            gtk::PositionType::Top,
            gtk::PositionType::Bottom,
            gtk::PositionType::Left,
            gtk::PositionType::Right,
        ] {
            match edge {
                gtk::PositionType::Top => body.set_margin_top(18),
                gtk::PositionType::Bottom => body.set_margin_bottom(18),
                gtk::PositionType::Left => body.set_margin_start(18),
                _ => body.set_margin_end(18),
            }
        }
        let text = label();
        body.append(&text);
        let feedback = label();
        body.append(&feedback);
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let local = gtk::Button::with_label(&gettext("Use local version"));
        let remote = gtk::Button::with_label(&gettext("Use Drive version"));
        let confirm = gtk::Button::with_label(&gettext("Confirm choice"));
        let later = gtk::Button::with_label(&gettext("Decide later"));
        for button in [&local, &remote, &confirm, &later] {
            actions.append(button);
        }
        body.append(&actions);
        window.set_child(Some(&gtk::ScrolledWindow::builder().child(&body).build()));
        let value = Self {
            window,
            text,
            feedback,
            local,
            remote,
            confirm,
            later,
            conflict: Rc::default(),
            state: Rc::new(RefCell::new(ConflictResolverState::Loading)),
            choice: Rc::default(),
            callback: Rc::default(),
        };
        for (button, choice) in [
            (&value.local, ConflictChoice::KeepLocal),
            (&value.remote, ConflictChoice::KeepRemote),
        ] {
            let state = value.state.clone();
            let selected = value.choice.clone();
            let confirm = value.confirm.downgrade();
            let feedback = value.feedback.downgrade();
            button.connect_clicked(move |_| {
                if *state.borrow() == ConflictResolverState::Reviewing {
                    *selected.borrow_mut() = Some(choice);
                    *state.borrow_mut() = ConflictResolverState::Confirming;
                    if let Some(confirm) = confirm.upgrade() {
                        confirm.set_sensitive(true);
                    }
                    if let Some(feedback) = feedback.upgrade() {
                        feedback.set_label(&ConflictResolver::state_text(
                            ConflictResolverState::Confirming,
                        ));
                    }
                }
            });
        }
        value.confirm.connect_clicked({
            let state = value.state.clone();
            let selected = value.choice.clone();
            let conflict = value.conflict.clone();
            let callback = value.callback.clone();
            let local = value.local.downgrade();
            let remote = value.remote.downgrade();
            let feedback = value.feedback.downgrade();
            move |confirm| {
                if *state.borrow() != ConflictResolverState::Confirming {
                    return;
                }
                let (Some(conflict), Some(choice)) =
                    (conflict.borrow().clone(), *selected.borrow())
                else {
                    return;
                };
                *state.borrow_mut() = ConflictResolverState::Submitting;
                confirm.set_sensitive(false);
                for button in [local.upgrade(), remote.upgrade()].into_iter().flatten() {
                    button.set_sensitive(false);
                }
                if let Some(feedback) = feedback.upgrade() {
                    feedback.set_label(&ConflictResolver::state_text(
                        ConflictResolverState::Submitting,
                    ));
                }
                if let Some(callback) = callback.borrow().as_ref() {
                    callback(ConflictChoiceRequest {
                        id: conflict.id,
                        revision: conflict.revision,
                        choice,
                    });
                }
            }
        });
        value.set_state(ConflictResolverState::Loading);
        value
    }
    pub fn set_conflict(&self, conflict: ConflictView) {
        self.text.set_label(&format!(
            "{}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}",
            conflict.name,
            conflict.differences,
            gettext("Computer"),
            conflict.local,
            gettext("Drive"),
            conflict.remote,
            gettext("File contents are preserved before replacement.")
        ));
        *self.conflict.borrow_mut() = Some(conflict);
        self.choice.borrow_mut().take();
        self.set_state(ConflictResolverState::Reviewing);
    }
    pub fn set_state(&self, state: ConflictResolverState) {
        *self.state.borrow_mut() = state;
        self.local
            .set_sensitive(state == ConflictResolverState::Reviewing);
        self.remote
            .set_sensitive(state == ConflictResolverState::Reviewing);
        self.later.set_label(&gettext(
            if matches!(
                state,
                ConflictResolverState::Reviewing | ConflictResolverState::Confirming
            ) {
                "Decide later"
            } else {
                "Close"
            },
        ));
        self.confirm
            .set_sensitive(state == ConflictResolverState::Confirming);
        self.feedback.set_label(&Self::state_text(state));
    }
    pub(crate) fn state_text(state: ConflictResolverState) -> String {
        gettext(match state {
            ConflictResolverState::Loading => "Loading conflicts…",
            ConflictResolverState::Reviewing => "Review versions",
            ConflictResolverState::Confirming => "Confirm choice",
            ConflictResolverState::Submitting => "Submitting choice…",
            ConflictResolverState::Applying => {
                "Choice recorded. Synchronization will recheck both copies."
            }
            ConflictResolverState::Resolved => "No unresolved conflicts.",
            ConflictResolverState::Stale => {
                "The file changed. Reopen conflicts to review its current versions."
            }
            ConflictResolverState::Failed => "Operation failed",
        })
    }
    pub fn state(&self) -> ConflictResolverState {
        *self.state.borrow()
    }
    pub fn connect_choice(&self, callback: impl Fn(ConflictChoiceRequest) + 'static) {
        *self.callback.borrow_mut() = Some(Box::new(callback));
    }
    pub fn connect_dismissed(&self, callback: impl Fn() + 'static) {
        self.window.connect_close_request(move |_| {
            callback();
            gtk::glib::Propagation::Proceed
        });
        let window = self.window.downgrade();
        self.later.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
    }
}
