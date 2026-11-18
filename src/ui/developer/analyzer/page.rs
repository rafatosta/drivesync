//! File/action inspection from durable analysis receipts. Widgets never start analysis.
use crate::application::{
    analyzer::{self, DecisionPage, Scope},
    Context,
};
use adw::{gtk, prelude::*};
use drivesync_engine::{inventory::*, models::MetadataValue};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .width_chars(1)
        .max_width_chars(1)
        .hexpand(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .build()
}
/// Preserve non-UTF-8 Unix names as visible byte escapes rather than replacement characters.
pub(super) fn file_name(item: &SyncItem) -> String {
    if let Some(local) = item.local_latest.as_ref().or(item.local.as_ref()) {
        return std::str::from_utf8(local.relative_path.as_bytes())
            .map(str::to_owned)
            .unwrap_or_else(|_| {
                format!(
                    "{:?}",
                    <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(
                        local.relative_path.as_bytes().to_vec()
                    )
                )
            })
            .replace('\0', "\\0");
    }
    if let Some(remote) = item.remote_latest.as_ref().or(item.remote.as_ref()) {
        if let MetadataValue::Known(name) = &remote.name {
            return name.replace('\0', "\\0");
        }
    }
    item.remote_ref
        .as_ref()
        .map(|r| format!("Drive item {}", r.file_id.0))
        .unwrap_or_else(|| format!("Item {}", item.id.0))
        .replace('\0', "\\0")
}
fn resource_label(resource: &drivesync_engine::operations::ResourceRef) -> String {
    use drivesync_engine::operations::ResourceRef;
    match resource {
        ResourceRef::Local { relative_path, .. } => format!(
            "Local {:?}",
            <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(
                relative_path.as_bytes().to_vec()
            )
        ),
        ResourceRef::Remote { reference, .. } => format!(
            "Drive ID {} (account {})",
            reference.file_id.0, reference.account_id.0
        ),
    }
}
fn command_label(command: &drivesync_engine::operations::OperationCommand) -> String {
    use drivesync_engine::operations::*;
    fn directory(d: &DirectoryRef) -> String {
        match d {
            DirectoryRef::LocalRoot => "Local sync root".into(),
            DirectoryRef::Local {
                relative_path,
                local_id,
            } => resource_label(&ResourceRef::Local {
                relative_path: relative_path.clone(),
                local_id: local_id.clone(),
            }),
            DirectoryRef::Remote {
                provider_id,
                reference,
            } => resource_label(&ResourceRef::Remote {
                provider_id: provider_id.clone(),
                reference: reference.clone(),
            }),
        }
    }
    fn name(n: &ItemName) -> String {
        match n {
            ItemName::Local(n) => format!("{n:?}"),
            ItemName::Remote(n) => n.clone(),
        }
    }
    match command {
        OperationCommand::Upload {
            source,
            destination,
            name: n,
        }
        | OperationCommand::Download {
            source,
            destination,
            name: n,
        } => format!(
            "{} → {} / {}",
            resource_label(source),
            directory(destination),
            name(n)
        ),
        OperationCommand::CreateFile {
            parent, name: n, ..
        }
        | OperationCommand::CreateDirectory { parent, name: n } => {
            format!("Destination: {} / {}", directory(parent), name(n))
        }
        OperationCommand::UpdateContent { target, .. } => {
            format!("Update existing {}", resource_label(target))
        }
        OperationCommand::Rename {
            target, new_name, ..
        } => format!("{} → {}", resource_label(target), name(new_name)),
        OperationCommand::Move {
            target,
            destination,
            ..
        } => format!("{} → {}", resource_label(target), directory(destination)),
        OperationCommand::Trash { target, .. } => format!("Trash: {}", resource_label(target)),
        OperationCommand::Restore { target, .. } => format!("Restore: {}", resource_label(target)),
    }
}
fn manual_reason(reason: ManualReason) -> &'static str {
    match reason {
        ManualReason::ConcurrentChanges => "Changes on both sides",
        ManualReason::DeleteVsChange => "Deletion versus modification",
        ManualReason::DestinationOccupied => "Independent items require the same destination",
        ManualReason::AssociationRequired => "Explicit identity association required",
        ManualReason::MissingBaseline => "Confirmed baseline needed",
        ManualReason::DeletionAuthorization => "Trash authorization required",
    }
}
fn intent_name(intent: &AnalysisIntent, kind: &ItemKind) -> &'static str {
    match (&intent.direction, &intent.kind) {
        (OperationDirection::Upload, OperationKind::Create) if *kind == ItemKind::Directory => {
            "Create folder in Drive"
        }
        (OperationDirection::Download, OperationKind::Create) if *kind == ItemKind::Directory => {
            "Create local folder"
        }
        (OperationDirection::Upload, OperationKind::Create) => "Upload new file to Drive",
        (OperationDirection::Download, OperationKind::Create) => "Download new file",
        (OperationDirection::Upload, OperationKind::UpdateContent) => "Update content in Drive",
        (OperationDirection::Download, OperationKind::UpdateContent) => "Update local content",
        (OperationDirection::Upload, OperationKind::Rename) => "Rename in Drive",
        (OperationDirection::Download, OperationKind::Rename) => "Rename locally",
        (OperationDirection::Upload, OperationKind::Move) => "Move in Drive",
        (OperationDirection::Download, OperationKind::Move) => "Move locally",
        (_, OperationKind::Trash) => "Move to trash",
        (_, OperationKind::Delete) => "Delete item",
        (_, OperationKind::Restore) => "Restore item",
    }
}
fn requirement_name(requirement: &AnalysisRequirement) -> &'static str {
    match requirement {
        AnalysisRequirement::ObserveLocal => "Observe local state",
        AnalysisRequirement::ObserveRemote => "Observe remote state",
        AnalysisRequirement::VerifyIdentity => "Verify file identity",
        AnalysisRequirement::VerifyBaseline => "Verify synchronized baseline",
        AnalysisRequirement::VerifyContent => "Verify file contents",
        AnalysisRequirement::VerifyName => "Verify file name",
        AnalysisRequirement::VerifyParent => "Verify parent folder",
        AnalysisRequirement::VerifySurvivingState => "Reobserve remaining version",
        AnalysisRequirement::AuthorizeDeletion => "Confirm deletion policy",
        AnalysisRequirement::ResolveDestination => "Identify destination folder",
        AnalysisRequirement::VerifyPreconditions => "Verify execution preconditions",
        AnalysisRequirement::VerifyCapabilities => "Verify operation support",
        AnalysisRequirement::VerifyKind => "Verify file type",
    }
}
fn actions(record: &RecordedAnalysis) -> String {
    let intents = record
        .decisions
        .iter()
        .map(|d| intent_name(&d.intent, &record.item_kind))
        .collect::<Vec<_>>()
        .join(" · ");
    if record.disposition == AnalysisDisposition::PendingUserDecision {
        if intents.is_empty() {
            "Resolve conflict".into()
        } else {
            format!("Resolve conflict · {intents}")
        }
    } else if !intents.is_empty() {
        intents
    } else if record.disposition == AnalysisDisposition::NoChanges {
        "No action needed".into()
    } else {
        record
            .requirements
            .iter()
            .map(requirement_name)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
/// Classify the actual semantic operations; direction is presented separately.
fn operation_category(record: &RecordedAnalysis) -> String {
    if record.stale {
        return "Awaiting analysis".into();
    }
    match record.disposition {
        AnalysisDisposition::PendingUserDecision => "Manual action".into(),
        AnalysisDisposition::NoChanges => "No action needed".into(),
        AnalysisDisposition::PendingEvidence | AnalysisDisposition::PendingOperations => {
            let mut kinds = Vec::new();
            for decision in &record.decisions {
                let name = match decision.intent.kind {
                    OperationKind::Create if record.item_kind == ItemKind::Directory => {
                        "Create folder"
                    }
                    OperationKind::Create => "Create file",
                    OperationKind::UpdateContent => "Update content",
                    OperationKind::Rename => "Rename",
                    OperationKind::Move => "Move",
                    OperationKind::Trash => "Move to trash",
                    OperationKind::Delete => "Delete",
                    OperationKind::Restore => "Restore",
                };
                if !kinds.contains(&name) {
                    kinds.push(name);
                }
            }
            if kinds.is_empty() {
                "Awaiting evidence".into()
            } else {
                kinds.join(" · ")
            }
        }
    }
}
/// Keep propagation direction distinct from operation kind and user conflicts.
fn operation_direction(record: &RecordedAnalysis) -> Option<&'static str> {
    if record.stale
        || matches!(
            record.disposition,
            AnalysisDisposition::PendingUserDecision | AnalysisDisposition::NoChanges
        )
    {
        return None;
    }
    let upload = record
        .decisions
        .iter()
        .any(|d| d.intent.direction == OperationDirection::Upload);
    let download = record
        .decisions
        .iter()
        .any(|d| d.intent.direction == OperationDirection::Download);
    match (upload, download) {
        (true, true) => Some("Upload + Download"),
        (true, false) => Some("Upload"),
        (false, true) => Some("Download"),
        _ => None,
    }
}
fn status(record: &RecordedAnalysis) -> &'static str {
    if record.stale {
        return "Outdated";
    }
    match record.disposition {
        AnalysisDisposition::NoChanges => "No changes",
        AnalysisDisposition::PendingOperations => "Pending · not executed",
        AnalysisDisposition::PendingEvidence => "Needs evidence",
        AnalysisDisposition::PendingUserDecision => "User decision required",
    }
}
#[derive(Default)]
struct State {
    scopes: Vec<Scope>,
    after: Option<String>,
    history: Vec<Option<String>>,
    next: Option<String>,
    loading: bool,
}
struct View {
    root: gtk::glib::WeakRef<gtk::Box>,
    context: Context,
    accounts: gtk::DropDown,
    refresh: gtk::Button,
    previous: gtk::Button,
    next: gtk::Button,
    outdated: gtk::CheckButton,
    spinner: gtk::Spinner,
    list: gtk::ListBox,
    message: gtk::Label,
    range: gtk::Label,
    path: gtk::Label,
    state: RefCell<State>,
    generation: Cell<u64>,
    loading_accounts: Cell<bool>,
    read_gate: std::sync::Arc<std::sync::Mutex<()>>,
    last_flow: Cell<Option<(u64, drivesync_engine::orchestrator::PreparationState)>>,
}
impl View {
    fn flow_version(&self) -> Option<(u64, drivesync_engine::orchestrator::PreparationState)> {
        let id = self
            .state
            .borrow()
            .scopes
            .get(self.accounts.selected() as usize)?
            .account_id
            .clone();
        let sessions = self.context.detectors.borrow();
        let snapshot = sessions.get(&id)?.orchestrator.preparation_status();
        Some((snapshot.events_processed, snapshot.state))
    }
    fn flow_problem(&self) -> Option<String> {
        let id = self
            .state
            .borrow()
            .scopes
            .get(self.accounts.selected() as usize)?
            .account_id
            .clone();
        let sessions = self.context.detectors.borrow();
        let snapshot = sessions.get(&id)?.orchestrator.preparation_status();
        if let Some(error) = snapshot.error {
            return Some(format!("Observation processing stopped: {error}. Retry preparation after resolving the issue."));
        }
        None
    }
    fn controls(&self) {
        let state = self.state.borrow();
        let busy = state.loading || self.loading_accounts.get();
        self.refresh.set_sensitive(!busy);
        self.previous
            .set_sensitive(!busy && !state.history.is_empty());
        self.next.set_sensitive(!busy && state.next.is_some());
        self.accounts
            .set_sensitive(!self.loading_accounts.get() && !state.scopes.is_empty());
        self.spinner.set_spinning(busy);
    }
    fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.range.set_label("");
    }
    fn reset(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.after = None;
        state.next = None;
        state.history.clear();
        drop(state);
        self.load();
    }
    fn render(&self, page: DecisionPage) {
        self.clear();
        let count = page.entries.len();
        for (record, item) in page.entries {
            let body = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_start(16)
                .margin_end(16)
                .margin_top(10)
                .margin_bottom(10)
                .build();
            let icon = gtk::Image::from_icon_name(if item.kind == ItemKind::Directory {
                "folder-symbolic"
            } else {
                "text-x-generic-symbolic"
            });
            icon.set_valign(gtk::Align::Start);
            icon.set_margin_top(2);
            body.append(&icon);
            let content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .hexpand(true)
                .build();
            let name = file_name(&item);
            let title = label(&name);
            title.add_css_class("heading");
            title.set_tooltip_text(Some(&name));
            content.append(&title);
            let summary = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let category = label(&operation_category(&record));
            category.add_css_class("heading");
            category.set_widget_name("analyzer-operation-category");
            summary.append(&category);
            let metadata = match operation_direction(&record) {
                Some(direction) => format!("{direction} · {}", status(&record)),
                None => status(&record).to_owned(),
            };
            let badge = label(&metadata);
            badge.set_xalign(1.0);
            badge.set_valign(gtk::Align::Start);
            badge.add_css_class("caption");
            badge.add_css_class(if record.stale {
                "dim-label"
            } else if record.disposition == AnalysisDisposition::PendingUserDecision {
                "warning"
            } else {
                "accent"
            });
            summary.append(&badge);
            content.append(&summary);
            if !record.requirements.is_empty() {
                let needs = label(&format!(
                    "Needs: {}",
                    record
                        .requirements
                        .iter()
                        .map(requirement_name)
                        .collect::<Vec<_>>()
                        .join(" · ")
                ));
                needs.add_css_class("dim-label");
                needs.add_css_class("caption");
                content.append(&needs);
            }
            if record.stale {
                let note = label("Outdated analysis — reobserve before applying this decision.");
                note.add_css_class("warning");
                content.append(&note);
            }
            let details = gtk::Expander::builder().label("Details").build();
            details.add_css_class("caption");
            let information = gtk::Box::new(gtk::Orientation::Vertical, 4);
            information.set_margin_top(4);
            let action = label(&actions(&record));
            action.add_css_class("dim-label");
            information.append(&action);
            let revision = label(&format!(
                "Analysis #{} · revision {}",
                record.id.0, record.analyzed_item_revision
            ));
            revision.add_css_class("dim-label");
            information.append(&revision);
            if let Some(plan) = &record.plan {
                for (index, step) in plan.steps.iter().enumerate() {
                    let blocked = step
                        .blocked_by
                        .iter()
                        .map(|b| format!("{b:?}"))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let text = label(&format!(
                        "Step {}: {:?}{}",
                        index + 1,
                        step.method,
                        if blocked.is_empty() {
                            " · planned, not executed".into()
                        } else {
                            format!(" · blocked: {blocked}")
                        }
                    ));
                    text.set_widget_name("analyzer-plan-step");
                    information.append(&text);
                    if let Some(command) = &step.command {
                        let description = label(&command_label(command));
                        description.add_css_class("dim-label");
                        information.append(&description);
                    }
                    if !step.depends_on.is_empty() || !step.depends_on_steps.is_empty() {
                        let dependencies = label(&format!(
                            "Depends on items: {} · prior steps: {}",
                            step.depends_on
                                .iter()
                                .map(|i| i.0.clone())
                                .collect::<Vec<_>>()
                                .join(", "),
                            step.depends_on_steps
                                .iter()
                                .map(|i| (i + 1).to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        dependencies.add_css_class("dim-label");
                        information.append(&dependencies);
                    }
                }
                for issue in &plan.manual {
                    let issue = label(&format!(
                        "{} · {}",
                        manual_reason(issue.reason),
                        issue
                            .resources
                            .iter()
                            .map(resource_label)
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ));
                    issue.add_css_class("warning");
                    information.append(&issue);
                }
            }
            details.set_child(Some(&information));
            content.append(&details);
            body.append(&content);
            self.list.append(&body);
        }
        self.state.borrow_mut().next = page.next_cursor;
        self.message.set_label(if count==0{"No analyses to show yet. Decisions appear after preparation and as new observations are processed."}else{"File names reflect the current inventory. Actions below are recorded decisions. Inspect Operator for execution progress and verified completion."});
        if let Some(problem) = self.flow_problem() {
            self.message.set_label(&problem);
        }
        self.range.set_label(&format!(
            "Page {} · {} analyses",
            self.state.borrow().history.len() + 1,
            count
        ));
    }
    fn load(self: &Rc<Self>) {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.clear();
        self.state.borrow_mut().next = None;
        let scope = self
            .state
            .borrow()
            .scopes
            .get(self.accounts.selected() as usize)
            .cloned();
        let Some(scope) = scope else {
            self.path.set_label("");
            self.message.set_label(
                "No configured accounts. Complete account setup to inspect analysis decisions.",
            );
            self.state.borrow_mut().loading = false;
            self.controls();
            return;
        };
        let request = match scope.request {
            Ok(request) => request,
            Err(error) => {
                self.path.set_label("");
                self.message.set_label(&error);
                self.state.borrow_mut().loading = false;
                self.controls();
                return;
            }
        };
        self.path
            .set_label(&request.local_root.display().to_string());
        self.message.set_label("Reading analysis decisions…");
        self.state.borrow_mut().loading = true;
        self.controls();
        let after = self.state.borrow().after.clone();
        let include_outdated = self.outdated.is_active();
        let weak = Rc::downgrade(self);
        let flow_version = self.flow_version();
        let read_gate = self.read_gate.clone();
        gtk::glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || {
                // Serialize stale and replacement reads without blocking GTK or
                // racing Inventory's nonblocking initialization lock.
                let _guard = read_gate.lock().unwrap_or_else(|e| e.into_inner());
                analyzer::read(request, after, include_outdated)
            })
            .await;
            let Some(view) = weak
                .upgrade()
                .filter(|v| v.root.upgrade().is_some() && v.generation.get() == generation)
            else {
                return;
            };
            view.state.borrow_mut().loading = false;
            match result {
                Ok(Ok(page)) => {
                    view.render(page);
                    view.last_flow.set(flow_version);
                }
                Ok(Err(error)) => view.message.set_label(match error.kind {
                    InventoryError::NotFound => {
                        "Inventory is not available yet. Refresh after preparation completes."
                    }
                    InventoryError::UnsupportedSchema => {
                        "Unsupported inventory schema. Existing data has been preserved."
                    }
                    InventoryError::InvalidIdentity => {
                        "Inventory does not match the selected account and folder."
                    }
                    _ => "Analysis records could not be read. Check folder access and try Refresh.",
                }),
                Err(_) => view
                    .message
                    .set_label("Analysis records could not be read. Try Refresh."),
            };
            view.controls();
        });
    }
    fn reload(self: &Rc<Self>) {
        if self.loading_accounts.replace(true) {
            return;
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        self.clear();
        self.state.borrow_mut().loading = false;
        self.state.borrow_mut().history.clear();
        self.state.borrow_mut().after = None;
        self.state.borrow_mut().next = None;
        self.path.set_label("");
        self.message.set_label("Loading configured accounts…");
        self.controls();
        let weak = Rc::downgrade(self);
        let context = self.context.clone();
        gtk::glib::spawn_future_local(async move {
            let result = analyzer::configured(&context).await;
            let Some(view) = weak.upgrade().filter(|v| v.root.upgrade().is_some()) else {
                return;
            };
            let selected = view
                .state
                .borrow()
                .scopes
                .get(view.accounts.selected() as usize)
                .map(|s| s.account_id.clone());
            match result {
                Ok(scopes) => {
                    let strings = scopes.iter().map(|s| s.label.as_str()).collect::<Vec<_>>();
                    let model = gtk::StringList::new(&strings);
                    let index = selected
                        .and_then(|id| scopes.iter().position(|s| s.account_id == id))
                        .unwrap_or(0);
                    let empty = scopes.is_empty();
                    view.state.borrow_mut().scopes = scopes;
                    view.accounts.set_model(Some(&model));
                    view.accounts.set_selected(if empty {
                        gtk::INVALID_LIST_POSITION
                    } else {
                        index as u32
                    });
                    view.loading_accounts.set(false);
                    view.reset();
                }
                Err(error) => {
                    view.loading_accounts.set(false);
                    view.state.borrow_mut().scopes.clear();
                    view.accounts.set_model(Some(&gtk::StringList::new(&[])));
                    view.message.set_label(&error);
                    view.controls();
                }
            }
        });
    }
}
pub(in crate::ui::developer) fn build(context: &Context) -> gtk::Box {
    let (root, view) = create(context);
    root.connect_map(move |_| view.reload());
    root
}
fn create(context: &Context) -> (gtk::Box, Rc<View>) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();
    root.append(&scroll);
    let title = label("Analyzer decisions");
    title.add_css_class("title-1");
    body.append(&title);
    let explanation=label("Inspect the file and the action identified by analysis. Actions identify creation, content updates, renames, moves or manual intervention. Upload/Download indicates direction. Supported new creations are executed after verification; inspect Operator for progress.");
    explanation.add_css_class("dim-label");
    body.append(&explanation);
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let accounts = gtk::DropDown::from_strings(&[]);
    accounts.set_hexpand(true);
    accounts.set_widget_name("analyzer-account");
    accounts.update_property(&[gtk::accessible::Property::Label("Analyzer account")]);
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
        item.child()
            .and_downcast::<gtk::Label>()
            .unwrap()
            .set_label(
                &item
                    .item()
                    .and_downcast::<gtk::StringObject>()
                    .unwrap()
                    .string(),
            );
    });
    accounts.set_factory(Some(&factory));
    let refresh = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh analysis decisions")
        .build();
    refresh.set_widget_name("analyzer-refresh");
    refresh.update_property(&[gtk::accessible::Property::Label(
        "Refresh analysis decisions",
    )]);
    let spinner = gtk::Spinner::new();
    toolbar.append(&accounts);
    toolbar.append(&spinner);
    toolbar.append(&refresh);
    body.append(&toolbar);
    let path = label("");
    path.add_css_class("dim-label");
    body.append(&path);
    let outdated = gtk::CheckButton::with_label("Include outdated analyses");
    body.append(&outdated);
    let associate = gtk::Button::with_label("Associate existing identities…");
    associate.set_halign(gtk::Align::Start);
    associate.set_widget_name("analyzer-associate");
    body.append(&associate);
    let message = label("Select an account to read its analysis decisions.");
    message.set_widget_name("analyzer-message");
    body.append(&message);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.set_widget_name("analyzer-decisions");
    body.append(&list);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let previous = gtk::Button::with_label("Previous");
    let next = gtk::Button::with_label("Next");
    let range = label("");
    range.set_hexpand(true);
    footer.append(&previous);
    footer.append(&range);
    footer.append(&next);
    body.append(&footer);
    let view = Rc::new(View {
        root: root.downgrade(),
        context: context.clone(),
        accounts: accounts.clone(),
        refresh: refresh.clone(),
        previous: previous.clone(),
        next: next.clone(),
        outdated: outdated.clone(),
        spinner,
        list,
        message,
        range,
        path,
        state: RefCell::new(State::default()),
        generation: Cell::new(0),
        loading_accounts: Cell::new(false),
        read_gate: std::sync::Arc::new(std::sync::Mutex::new(())),
        last_flow: Cell::new(None),
    });
    accounts.connect_selected_notify({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                if !view.loading_accounts.get() {
                    view.reset();
                }
            }
        }
    });
    outdated.connect_toggled({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                if !view.loading_accounts.get() {
                    view.reset();
                }
            }
        }
    });
    refresh.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                view.reload();
            }
        }
    });
    previous.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let after = view.state.borrow_mut().history.pop();
                if let Some(after) = after {
                    view.state.borrow_mut().after = after;
                    view.load();
                }
            }
        }
    });
    next.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let mut state = view.state.borrow_mut();
                if let Some(after) = state.next.clone() {
                    let old = state.after.clone();
                    state.history.push(old);
                    state.after = Some(after);
                    drop(state);
                    view.load();
                }
            }
        }
    });
    let weak = Rc::downgrade(&view);
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let Some(view) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        let Some(root) = view.root.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        let first_page = view.state.borrow().after.is_none();
        if root.is_mapped()
            && first_page
            && !view.outdated.is_active()
            && !view.loading_accounts.get()
            && !view.state.borrow().loading
        {
            if let Some(version) = view.flow_version() {
                if view.last_flow.get() != Some(version) {
                    view.load();
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    associate.connect_clicked({
        let weak = Rc::downgrade(&view);
        move |_| {
            if let Some(view) = weak.upgrade() {
                let scope = view
                    .state
                    .borrow()
                    .scopes
                    .get(view.accounts.selected() as usize)
                    .cloned();
                if let (Some(scope), Some(root)) = (scope, view.root.upgrade()) {
                    let weak = Rc::downgrade(&view);
                    super::association::present(
                        root.upcast_ref(),
                        view.context.clone(),
                        scope,
                        Rc::new(move || {
                            if let Some(view) = weak.upgrade() {
                                view.reset();
                            }
                        }),
                    );
                }
            }
        }
    });
    view.controls();
    (root, view)
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
