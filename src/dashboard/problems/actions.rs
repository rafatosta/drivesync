//! Explicit decisions bound to displayed evidence, with safe account-job exclusion.
use super::{models, present_items};
use crate::automatic::{Controller, ResolutionTask};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    AccountId, AccountSummary, ConflictRecord, DriveSyncEngine, ResolutionChoice, ResolutionRequest,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

pub(super) struct Center {
    engine: Arc<DriveSyncEngine>,
    id: AccountId,
    account: String,
    job: Controller,
    window: gtk::glib::WeakRef<adw::Window>,
    list: gtk::Box,
    status: gtk::Label,
    refresh: gtk::Button,
    later: gtk::Button,
    pause: gtk::Button,
    busy: Cell<bool>,
    loading: Cell<bool>,
}

pub(super) fn open(
    button: &gtk::Button,
    engine: Arc<DriveSyncEngine>,
    account: &AccountSummary,
    job: Controller,
) {
    let parent = button
        .root()
        .and_then(|root| root.downcast::<gtk::Window>().ok());
    let window = adw::Window::builder()
        .title("Conflicts and problems")
        .default_width(640)
        .default_height(560)
        .modal(true)
        .build();
    window.set_transient_for(parent.as_ref());
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let later = gtk::Button::with_label("Decide later");
    header.pack_end(&later);
    toolbar.add_top_bar(&header);
    let content = crate::flow::content();
    let identity = account.email.as_deref().unwrap_or(&account.id.0).to_owned();
    let label = gtk::Label::builder()
        .label(format!("Account: {identity}"))
        .wrap(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    content.append(&label);
    if let Some(result) = job.snapshot().result {
        let errors = match result {
            Ok(report) => report
                .issues
                .into_iter()
                .map(|issue| {
                    format!(
                        "{}: {}",
                        issue
                            .relative_path
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "Account".into()),
                        issue.error
                    )
                })
                .collect::<Vec<_>>(),
            Err(error) => vec![error.to_string()],
        };
        if !errors.is_empty() {
            let label = gtk::Label::builder()
                .label(format!(
                    "Errors from the latest cycle (saved observations may be older):\n{}",
                    errors.join("\n")
                ))
                .wrap(true)
                .selectable(true)
                .xalign(0.0)
                .build();
            content.append(&label);
        }
    }
    let status = gtk::Label::builder()
        .wrap(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    content.append(&status);
    let refresh = gtk::Button::with_label("Reload saved observations");
    content.append(&refresh);
    let pause = gtk::Button::with_label("Pause safely");
    pause.set_visible(false);
    content.append(&pause);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.append(&list);
    toolbar.set_content(Some(&crate::flow::scroll(&content, 760)));
    window.set_content(Some(&toolbar));
    let center = Rc::new(Center {
        engine,
        id: account.id.clone(),
        account: identity,
        job,
        window: window.downgrade(),
        list,
        status,
        refresh,
        later,
        pause,
        busy: Cell::new(false),
        loading: Cell::new(false),
    });
    let close = center.clone();
    window.connect_close_request(move |_| {
        if close.busy.get() {close.job.pause();close.status.set_label("Pausing at a safe boundary. Accepted effects remain saved and may require recovery.");gtk::glib::Propagation::Stop} else {gtk::glib::Propagation::Proceed}
    });
    let weak = window.downgrade();
    center.later.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    let weak = Rc::downgrade(&center);
    center.refresh.connect_clicked(move |_| {
        if let Some(center) = weak.upgrade() {
            center.load(None)
        }
    });
    let weak = Rc::downgrade(&center);
    center.pause.connect_clicked(move |_| {
        if let Some(center) = weak.upgrade() {
            center.job.pause();
            center.status.set_label(
                "Pausing safely. This does not undo accepted effects; recovery may be required.",
            );
        }
    });
    center.load(None);
    window.present();
}

impl Center {
    /// Reloading reads saved state only and never acknowledges pending evidence.
    fn load(self: &Rc<Self>, notice: Option<String>) {
        if self.busy.get() || self.loading.replace(true) {
            return;
        }
        self.refresh.set_sensitive(false);
        self.list.set_sensitive(false);
        self.status.set_label("Loading saved observations…");
        let engine = self.engine.clone();
        let id = self.id.clone();
        let center = self.clone();
        crate::flow::work(
            move || {
                let records = engine.saved_conflicts(&id)?;
                let progress = engine.saved_resolutions(&id)?;
                Ok::<_, drivesync_engine::EngineError>((records, progress))
            },
            move |result| {
                center.loading.set(false);
                center.refresh.set_sensitive(true);
                center.list.set_sensitive(true);
                match result {
                Err(error)=>center.status.set_label(&format!("Saved state could not be loaded: {error}. Retry reloading. Existing evidence has not been cleared.")),
                Ok((records,progress))=>{
                    while let Some(child)=center.list.first_child(){center.list.remove(&child)}
                    let items=models::pending(&records);
                    let message=if items.is_empty(){"No saved pending items. This is not a new synchronization check."}else{"Saved observations may be outdated. No network check was made. Decide later keeps unaccepted items pending."};
                    center.status.set_label(&notice.map(|notice|format!("{notice}\n\n{message}")).unwrap_or_else(||message.into()));
                    if !progress.is_empty() {
                        let group=adw::PreferencesGroup::builder().title("Accepted decisions awaiting recovery").description("These choices are already saved. Paths below are planned or previously verified; current copy availability has not been checked.").build();
                        for operation in progress {
                            let row=adw::ActionRow::builder().title(operation.source_path.display().to_string()).subtitle(format!("Choice: {:?}. Stage: {:?}. Target: {}\nPreservation locations: {}{}",operation.choice,operation.state,operation.target_path.display(),operation.preserved_paths.iter().map(|p|p.display().to_string()).collect::<Vec<_>>().join("; "),operation.additional_path.map(|p|format!("\nAdditional synchronized file: {}",p.display())).unwrap_or_default())).use_markup(false).subtitle_selectable(true).build();row.set_title_lines(0);row.set_subtitle_lines(0);group.add(&row);
                        }
                        let resume=gtk::Button::with_label("Resume accepted decisions");let weak=Rc::downgrade(&center);resume.connect_clicked(move |_|{if let Some(center)=weak.upgrade(){center.run(ResolutionTask::ResumeAccepted)}});group.add(&resume);center.list.append(&group);
                    }
                    present_items(&center.list,&items,Some(&center));
                }
            }
            },
        );
    }
    fn run(self: &Rc<Self>, task: ResolutionTask) {
        if self.busy.get() || self.loading.get() {
            return;
        }
        self.busy.set(true);
        self.list.set_sensitive(false);
        self.refresh.set_sensitive(false);
        self.later.set_sensitive(false);
        self.pause.set_visible(true);
        self.status.set_label("Revalidating the accepted decision and preserving versions. Pause requests stop at a safe boundary; they do not undo completed effects.");
        let center = self.clone();
        if !self.job.run_resolution(self.engine.clone(),task,move |result| {
            center.busy.set(false);center.later.set_sensitive(true);center.pause.set_visible(false);
            let notice=match result {
                Ok(reports)=>format!("Verified {} decision(s). {}",reports.len(),reports.iter().map(|report|format!("Active file: {}. Preserved versions: {}{}",report.path.display(),report.preserved_paths.iter().map(|p|p.display().to_string()).collect::<Vec<_>>().join("; "),report.additional_path.as_ref().map(|p|format!(". Additional synchronized file: {}",p.display())).unwrap_or_default())).collect::<Vec<_>>().join("\n")),
                Err(error)=>format!("Decision not confirmed: {error}. Copies and accepted intentions are retained. If evidence changed, synchronize again and review the new observation. Use recovery for an already accepted decision; do not assume it was undone."),
            };
            center.load(Some(notice));
        }) {
            self.busy.set(false);self.list.set_sensitive(true);self.refresh.set_sensitive(true);self.later.set_sensitive(true);self.pause.set_visible(false);
            self.status.set_label("The account is busy or unavailable. Wait for its current operation to stop, then retry. No new decision was dispatched.");
        }
    }
}

fn choice_label(choice: ResolutionChoice) -> &'static str {
    match choice {
        ResolutionChoice::KeepLocal => "Keep local version…",
        ResolutionChoice::KeepRemote => "Keep Drive version…",
        ResolutionChoice::KeepBoth => "Keep both versions…",
    }
}

/// No decision is dispatched until the explicit response; closing means cancel.
fn confirmation(
    parent: &adw::Window,
    account: &str,
    record: &ConflictRecord,
    choice: ResolutionChoice,
) -> adw::MessageDialog {
    let dialog = adw::MessageDialog::new(Some(parent), Some("Apply this version choice?"), None);
    let effect=match choice {
        ResolutionChoice::KeepLocal=>"The local bytes will replace the observed Drive version at its original ID. Available versions are preserved before replacement.",
        ResolutionChoice::KeepRemote=>"The observed Drive bytes will become the active local version. The displaced local version is preserved beside the file.",
        ResolutionChoice::KeepBoth=>"The local version stays at the original file name and Drive ID. The observed Drive version becomes another synchronized file named with ‘DriveSync remote conflict’ and a unique operation ID. Both observed versions are also preserved locally.",
    };
    let label=gtk::Label::builder().label(format!("Account: {account}\nFile: {}\nObserved Drive revision: {}\n\n{effect}\n\nA live check must still confirm these exact versions, identity, access and destination. Changed evidence blocks application. Accepted operations can require recovery after interruption. Cancel leaves this item pending.",record.relative_path.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"Unavailable".into()),record.remote_version.map(|v|v.to_string()).unwrap_or_else(||"Unavailable".into()))).use_markup(false).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).selectable(true).xalign(0.0).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(160)
        .max_content_height(340)
        .propagate_natural_height(true)
        .child(&label)
        .build();
    dialog.set_extra_child(Some(&scroll));
    dialog.add_responses(&[("cancel", "Cancel"), ("apply", "Apply choice")]);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog
}

pub(super) fn add_choices(row: &adw::ExpanderRow, center: &Rc<Center>, record: &ConflictRecord) {
    let buttons = gtk::Box::new(gtk::Orientation::Vertical, 6);
    buttons.set_margin_start(12);
    buttons.set_margin_end(12);
    buttons.set_margin_top(6);
    buttons.set_margin_bottom(12);
    for choice in [
        ResolutionChoice::KeepLocal,
        ResolutionChoice::KeepRemote,
        ResolutionChoice::KeepBoth,
    ] {
        let button = gtk::Button::with_label(choice_label(choice));
        let record = record.clone();
        let weak = Rc::downgrade(center);
        button.connect_clicked(move |_| {
            let Some(center) = weak.upgrade() else { return };
            if center.busy.get() || center.loading.get() {
                return;
            }
            let Some(window) = center.window.upgrade() else {
                return;
            };
            let Ok(request) = ResolutionRequest::from_record(&record, choice) else {
                return;
            };
            let dialog = confirmation(&window, &center.account, &record, choice);
            let weak = Rc::downgrade(&center);
            dialog.connect_response(Some("apply"), move |_, _| {
                if let Some(center) = weak.upgrade() {
                    center.run(ResolutionTask::Apply(Box::new(request.clone())))
                }
            });
            dialog.present();
        });
        buttons.append(&button);
    }
    row.add_row(&buttons);
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Exercise GTK response semantics without credentials, storage or a worker.
    #[test]
    #[ignore = "requires a desktop display"]
    fn explicit_choices_are_literal_and_cancel_by_default() {
        adw::init().unwrap();
        let parent = adw::Window::builder()
            .default_width(360)
            .default_height(640)
            .build();
        let record = ConflictRecord {
            conflict_id: "a".repeat(64),
            remote_id: Some("file".into()),
            remote_parent_id: Some("root".into()),
            remote_name: Some("<report>&.txt".into()),
            relative_path: Some("<report>&.txt".into()),
            diagnosis: drivesync_engine::ChangeKind::ConcurrentChanges,
            status: drivesync_engine::ConflictStatus::Pending,
            local_size: Some(10),
            local_modified_unix: None,
            remote_size: Some(20),
            local_fingerprint: Some(format!("{}:1:2", "b".repeat(64))),
            remote_version: Some(2),
            outcomes: vec![],
        };
        for choice in [
            ResolutionChoice::KeepLocal,
            ResolutionChoice::KeepRemote,
            ResolutionChoice::KeepBoth,
        ] {
            let dialog = confirmation(&parent, "test@example.invalid", &record, choice);
            assert_eq!(dialog.default_response().as_deref(), Some("cancel"));
            assert_eq!(dialog.close_response(), "cancel");
            let scroll = dialog
                .extra_child()
                .unwrap()
                .downcast::<gtk::ScrolledWindow>()
                .unwrap();
            let viewport = scroll.child().unwrap().downcast::<gtk::Viewport>().unwrap();
            let label = viewport.child().unwrap().downcast::<gtk::Label>().unwrap();
            assert!(!label.uses_markup());
            assert!(label.is_selectable());
            assert!(label.wraps());
            assert!(label.text().contains("<report>&.txt"));
            assert!(label.text().contains("revision: 2"));
            if choice == ResolutionChoice::KeepBoth {
                assert!(label.text().contains("original file name and Drive ID"));
                assert!(label.text().contains("another synchronized file"));
            }
            let applied = Rc::new(Cell::new(false));
            let observed = applied.clone();
            dialog.connect_response(Some("apply"), move |_, _| observed.set(true));
            dialog.response("cancel");
            assert!(!applied.get());
            let dialog = confirmation(&parent, "test@example.invalid", &record, choice);
            let applied = Rc::new(Cell::new(false));
            let observed = applied.clone();
            dialog.connect_response(Some("apply"), move |_, _| observed.set(true));
            dialog.response("apply");
            assert!(applied.get());
        }
        let engine = Arc::new(DriveSyncEngine::new());
        let account = AccountSummary {
            id: AccountId("ui-fixture".into()),
            provider_subject: "ui-fixture".into(),
            email: Some("test@example.invalid".into()),
        };
        let job = Controller::attach(
            engine.clone(),
            &drivesync_engine::RestoredAccount {
                account: account.clone(),
                folder: None,
                error: None,
            },
        );
        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let center = Rc::new(Center {
            engine,
            id: account.id,
            account: "test@example.invalid".into(),
            job,
            window: parent.downgrade(),
            list: list.clone(),
            status: gtk::Label::new(None),
            refresh: gtk::Button::with_label("Reload saved observations"),
            later: gtk::Button::with_label("Decide later"),
            pause: gtk::Button::with_label("Pause safely"),
            busy: Cell::new(false),
            loading: Cell::new(false),
        });
        let mut unknown = record.clone();
        unknown.remote_id = None;
        present_items(
            &list,
            &models::pending(&[record.clone(), unknown]),
            Some(&center),
        );
        fn expand(widget: &gtk::Widget, buttons: &mut Vec<gtk::Button>) {
            if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
                row.set_expanded(true)
            }
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button
                    .label()
                    .is_some_and(|label| label.starts_with("Keep "))
                {
                    buttons.push(button.clone())
                }
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                expand(&current, buttons);
                child = current.next_sibling()
            }
        }
        let mut buttons = vec![];
        expand(list.upcast_ref(), &mut buttons);
        assert_eq!(
            buttons.len(),
            3,
            "unsupported observation must not offer choices"
        );
        parent.set_content(Some(&crate::flow::scroll(&list, 760)));
        parent.present();
        settle();
        assert!(buttons[0].grab_focus());
        assert_eq!(buttons[0].accessible_role(), gtk::AccessibleRole::Button);
        capture(&parent, "choices-360");
        buttons[2].emit_clicked();
        settle();
        let dialog = gtk::Window::list_toplevels()
            .into_iter()
            .find_map(|window| window.downcast::<adw::MessageDialog>().ok())
            .expect("real choice opens confirmation");
        capture(&dialog, "keep-both-confirmation");
        dialog.response("cancel");
        assert!(!center.busy.get());
        assert!(!center.job.running());
        parent.close();
    }

    fn settle() {
        let frame = gtk::glib::MainLoop::new(None, false);
        let finish = frame.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            finish.quit()
        });
        frame.run();
    }
    fn capture(window: &impl IsA<gtk::Window>, name: &str) {
        let Ok(output) = std::env::var("DRIVESYNC_RESOLUTION_UI_OUTPUT") else {
            return;
        };
        std::fs::create_dir_all(&output).unwrap();
        let window = window.as_ref();
        settle();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        let node = snapshot.to_node().expect("real widget frame");
        window
            .renderer()
            .unwrap()
            .render_texture(&node, None)
            .save_to_png(std::path::Path::new(&output).join(format!("{name}.png")))
            .unwrap();
    }
}
