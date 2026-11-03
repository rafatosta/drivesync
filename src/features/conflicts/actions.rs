//! Explicit decisions bound to displayed evidence, with safe account-job exclusion.
use super::{models, present_items, ReviewKind};
use crate::features::synchronization::automatic::{Controller, ResolutionTask};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    AccountId, AccountSummary, ConflictRecord, DriveSyncEngine, ResolutionChoice, ResolutionRequest,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

pub(super) struct Center {
    engine: Arc<DriveSyncEngine>,
    kind: Option<ReviewKind>,
    id: AccountId,
    account: String,
    job: Controller,
    window: gtk::glib::WeakRef<adw::Window>,
    list: gtk::Box,
    navigation: adw::NavigationView,
    root_page: adw::NavigationPage,
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
    kind: Option<ReviewKind>,
) {
    let parent = button
        .root()
        .and_then(|root| root.downcast::<gtk::Window>().ok());
    let window = adw::Window::builder()
        .title(gettextrs::gettext(
            kind.map_or("Conflicts and problems", ReviewKind::title),
        ))
        .default_width(640)
        .default_height(560)
        .modal(true)
        .build();
    window.set_transient_for(parent.as_ref());
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let later = gtk::Button::with_label(&gettextrs::gettext("Decide later"));
    header.pack_end(&later);
    toolbar.add_top_bar(&header);
    let content = crate::ui::content();
    let identity = account.email.as_deref().unwrap_or(&account.id.0).to_owned();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        &gettextrs::gettext(kind.map_or("Conflicts and problems", ReviewKind::title)),
        &identity,
    )));
    let label = gtk::Label::builder()
        .label(gettextrs::gettext("Account: {account}").replace("{account}", &identity))
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
                .filter(|issue| match kind {
                    None => true,
                    Some(ReviewKind::Conflicts) => {
                        issue.error == drivesync_engine::EngineError::DownloadConflict
                    }
                    Some(ReviewKind::Problems) => {
                        issue.error != drivesync_engine::EngineError::DownloadConflict
                    }
                })
                .map(|issue| {
                    format!(
                        "{}: {}",
                        issue
                            .relative_path
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| gettextrs::gettext("Account")),
                        gettextrs::gettext(issue.error.to_string())
                    )
                })
                .collect::<Vec<_>>(),
            Err(error) => vec![gettextrs::gettext(error.to_string())],
        };
        if !errors.is_empty() {
            let label = gtk::Label::builder()
                .label(
                    gettextrs::gettext(
                        "Errors from the latest cycle (saved observations may be older):\n{errors}",
                    )
                    .replace("{errors}", &errors.join("\n")),
                )
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
    status.set_margin_start(18);
    status.set_margin_end(18);
    status.set_margin_bottom(12);
    let status_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .max_content_height(120)
        .propagate_natural_height(true)
        .child(&status)
        .build();
    toolbar.add_bottom_bar(&status_scroll);
    let refresh = gtk::Button::with_label(&gettextrs::gettext("Check current versions"));
    content.append(&refresh);
    let pause = gtk::Button::with_label(&gettextrs::gettext("Pause safely"));
    pause.set_visible(false);
    toolbar.add_bottom_bar(&pause);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.append(&list);
    let navigation = adw::NavigationView::new();
    let root_page = adw::NavigationPage::with_tag(
        &crate::ui::scroll(&content, 760),
        &gettextrs::gettext("Pending items"),
        "pending",
    );
    navigation.add(&root_page);
    toolbar.set_content(Some(&navigation));
    window.set_content(Some(&toolbar));
    let center = Rc::new(Center {
        kind,
        engine,
        id: account.id.clone(),
        account: identity,
        job,
        window: window.downgrade(),
        list,
        navigation,
        root_page,
        status,
        refresh,
        later,
        pause,
        busy: Cell::new(false),
        loading: Cell::new(false),
    });
    let close = center.clone();
    window.connect_close_request(move |_| {
        if close.busy.get() {close.job.pause();close.status.set_label(&gettextrs::gettext("Pausing at a safe boundary. Accepted effects remain saved and may require recovery."));gtk::glib::Propagation::Stop} else {gtk::glib::Propagation::Proceed}
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
            center.run(ResolutionTask::Recheck)
        }
    });
    let weak = Rc::downgrade(&center);
    center.pause.connect_clicked(move |_| {
        if let Some(center) = weak.upgrade() {
            center.job.pause();
            center.status.set_label(&gettextrs::gettext(
                "Pausing safely. This does not undo accepted effects; recovery may be required.",
            ));
        }
    });
    center.load(None);
    window.present();
}

impl Center {
    /// Pushes a page in the existing window; back never dispatches a decision.
    pub(super) fn push(&self, title: &str, content: &impl IsA<gtk::Widget>) {
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&crate::ui::scroll(content, 760)));
        self.navigation
            .push(&adw::NavigationPage::new(&toolbar, title));
    }

    /// Keeps the saved list alive while inspecting one observation.
    pub(super) fn detail(self: &Rc<Self>, item: &models::Item) {
        if self.busy.get() || self.loading.get() {
            return;
        }
        let content = crate::ui::content();
        let check = gtk::Button::with_label(&gettextrs::gettext("Check current versions"));
        let weak = Rc::downgrade(self);
        check.connect_clicked(move |_| {
            if let Some(center) = weak.upgrade() {
                center.run(ResolutionTask::Recheck);
            }
        });
        content.append(&check);
        if let Some(record) = &item.record {
            content.append(
                &gtk::Label::builder()
                    .label(gettextrs::gettext(record.diagnosis.description()))
                    .use_markup(false)
                    .wrap(true)
                    .xalign(0.0)
                    .build(),
            );
            let versions = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .homogeneous(true)
                .min_children_per_line(1)
                .max_children_per_line(2)
                .column_spacing(12)
                .row_spacing(12)
                .build();
            for (title, size) in [
                ("Local version", record.local_size),
                ("Google Drive version", record.remote_size),
            ] {
                let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
                card.add_css_class("card");
                let title = gtk::Label::builder()
                    .label(gettextrs::gettext(title))
                    .wrap(true)
                    .margin_top(12)
                    .margin_start(12)
                    .margin_end(12)
                    .build();
                card.append(&title);
                card.append(
                    &gtk::Label::builder()
                        .label(
                            size.map(|v| format!("{v} bytes"))
                                .unwrap_or_else(|| gettextrs::gettext("Unavailable")),
                        )
                        .margin_bottom(12)
                        .margin_start(12)
                        .margin_end(12)
                        .build(),
                );
                versions.insert(&card, -1);
            }
            content.append(&versions);
        }
        let details = adw::ExpanderRow::builder()
            .title(gettextrs::gettext("Observed versions and next steps"))
            .build();
        details.set_expanded(item.record.is_none());
        details.add_row(
            &adw::ActionRow::builder()
                .title(gettextrs::gettext("Saved observation"))
                .subtitle(&item.details)
                .subtitle_selectable(true)
                .use_markup(false)
                .subtitle_lines(0)
                .build(),
        );
        let group = adw::PreferencesGroup::new();
        group.add(&details);
        content.append(&group);
        if let Some(record) = &item.record {
            add_choices(&content, self, record);
        }
        self.push(&item.path, &content);
    }

    /// Reloading reads saved state only and never acknowledges pending evidence.
    fn load(self: &Rc<Self>, notice: Option<String>) {
        if self.busy.get() || self.loading.replace(true) {
            return;
        }
        self.refresh.set_sensitive(false);
        self.list.set_sensitive(false);
        self.status
            .set_label(&gettextrs::gettext("Loading saved observations…"));
        let engine = self.engine.clone();
        let id = self.id.clone();
        let center = self.clone();
        crate::core::worker::work(
            move || {
                let records = engine.saved_conflicts(&id)?;
                let progress = engine.saved_resolutions(&id)?;
                Ok::<_, drivesync_engine::EngineError>((records, progress))
            },
            move |result| {
                center.loading.set(false);
                center.navigation.set_sensitive(true);
                center.refresh.set_sensitive(true);
                center.list.set_sensitive(true);
                match result {
                Err(error)=>center.status.set_label(&gettextrs::gettext("Saved state could not be loaded: {error}. Retry checking current versions. Existing evidence has not been cleared.").replace("{error}", gettextrs::gettext(error.to_string()).trim_end_matches('.'))),
                Ok((records,progress))=>{
                    while let Some(child)=center.list.first_child(){center.list.remove(&child)}
                    let items=models::pending(&records).into_iter().filter(|item| center.kind.is_none_or(|kind| kind.includes(item.category))).collect::<Vec<_>>();
                    let message = gettextrs::gettext(if items.is_empty(){"No saved pending items. A version check does not transfer pending files."}else{"These are saved observations. Use Check current versions to refresh them. Decide later keeps unaccepted items pending."});
                    center.status.set_label(&notice.map(|notice|format!("{notice}\n\n{message}")).unwrap_or(message));
                    if !progress.is_empty() {
                        let group=adw::PreferencesGroup::builder().title(gettextrs::gettext("Accepted decisions awaiting recovery")).description(gettextrs::gettext("These choices are already saved. Paths below are planned or previously verified; current copy availability has not been checked.")).build();
                        for operation in progress {
                            let row=adw::ActionRow::builder().title(operation.source_path.display().to_string()).subtitle(format!("Choice: {:?}. Stage: {:?}. Target: {}\nPreservation locations: {}{}",operation.choice,operation.state,operation.target_path.display(),operation.preserved_paths.iter().map(|p|p.display().to_string()).collect::<Vec<_>>().join("; "),operation.additional_path.map(|p|format!("\nAdditional synchronized file: {}",p.display())).unwrap_or_default())).use_markup(false).subtitle_selectable(true).build();row.set_title_lines(0);row.set_subtitle_lines(0);group.add(&row);
                        }
                        let resume=gtk::Button::with_label(&gettextrs::gettext("Resume accepted decisions"));let weak=Rc::downgrade(&center);resume.connect_clicked(move |_|{if let Some(center)=weak.upgrade(){center.run(ResolutionTask::ResumeAccepted)}});group.add(&resume);center.list.append(&group);
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
        self.navigation.set_sensitive(false);
        self.list.set_sensitive(false);
        self.refresh.set_sensitive(false);
        self.later.set_sensitive(false);
        self.pause.set_visible(true);
        let recheck = matches!(task, ResolutionTask::Recheck);
        self.status.set_label(&gettextrs::gettext(if recheck {
            "Checking current local and Drive versions. No version will be selected."
        } else {
            "Revalidating the accepted decision and preserving versions. Pause requests stop at a safe boundary; they do not undo completed effects."
        }));
        let center = self.clone();
        if !self.job.run_resolution(self.engine.clone(),task,move |result| {
            center.busy.set(false);center.navigation.set_sensitive(true);center.later.set_sensitive(true);center.pause.set_visible(false);
            let notice=match result {
                Ok(_) if recheck => gettextrs::gettext("Current versions checked. Review the updated observations before choosing."),
                Ok(reports) => gettextrs::gettext("Verified {count} decision(s). Active files and preserved versions: {paths}")
                    .replace("{count}", &reports.len().to_string())
                    .replace("{paths}", &reports.iter().flat_map(|report| std::iter::once(&report.path).chain(report.additional_path.iter()).chain(report.preserved_paths.iter())).map(|p| p.display().to_string()).collect::<Vec<_>>().join("; ")),
                Err(error) => gettextrs::gettext("Operation not confirmed: {error}. Copies and accepted decisions are retained. Check current versions if evidence changed. Resume accepted decisions when recovery is pending.")
                    .replace("{error}", gettextrs::gettext(error.to_string()).trim_end_matches('.')),
            };
            center.navigation.pop_to_page(&center.root_page);
            center.load(Some(notice));
        }) {
            self.busy.set(false);self.navigation.set_sensitive(true);self.list.set_sensitive(true);self.refresh.set_sensitive(true);self.later.set_sensitive(true);self.pause.set_visible(false);
            self.status.set_label(&gettextrs::gettext("The account is busy or unavailable. Wait for its current operation to stop, then retry. No new decision was dispatched."));
        }
    }
}

fn choice_label(choice: ResolutionChoice) -> &'static str {
    match choice {
        ResolutionChoice::MergeText => "Apply reviewed text merge…",
        ResolutionChoice::KeepLocal => "Keep local version…",
        ResolutionChoice::KeepRemote => "Keep Drive version…",
        ResolutionChoice::KeepBoth => "Keep both versions…",
    }
}

/// Describes the exact engine effect before the user accepts a choice.
fn choice_effect(choice: ResolutionChoice) -> &'static str {
    match choice {
        ResolutionChoice::MergeText=>"The reviewed plain-text candidate replaces both active versions at the existing Drive ID. Base, local and Drive versions are preserved. The positional merge requires review of the whole candidate; it cannot establish semantic compatibility.",
        ResolutionChoice::KeepLocal=>"The computer version will replace the Drive version. Both observed versions will be preserved before replacement.",
        ResolutionChoice::KeepRemote=>"The observed Drive bytes will become the active local version. The displaced local version is preserved beside the file.",
        ResolutionChoice::KeepBoth=>"Both versions will be preserved under different synchronized names. The computer version keeps the original name.",
    }
}

pub(super) fn add_choices(row: &gtk::Box, center: &Rc<Center>, record: &ConflictRecord) {
    let buttons = gtk::Box::new(gtk::Orientation::Vertical, 6);
    buttons.set_margin_start(12);
    buttons.set_margin_end(12);
    buttons.set_margin_top(6);
    buttons.set_margin_bottom(12);
    let compare = gtk::Button::with_label(&gettextrs::gettext("Compare observed versions…"));
    let comparison_record = record.clone();
    let weak = Rc::downgrade(center);
    compare.connect_clicked(move |_| {
        let Some(center) = weak.upgrade() else { return; };
        if center.busy.get() || center.loading.get() || center.job.running() { return; }
        center.loading.set(true);
        center.navigation.set_sensitive(false);
        center.list.set_sensitive(false);
        center.refresh.set_sensitive(false);
        center.status.set_label(&gettextrs::gettext("Reading and verifying observed versions…"));
        let engine = center.engine.clone();
        let id = center.id.clone();
        let record = comparison_record.clone();
        let cancel = drivesync_engine::Cancellation::new();
        let weak_window = center.window.clone();
        let close_cancel = cancel.clone();
        let handler = weak_window.upgrade().map(|window| window.connect_close_request(move |_| {
            close_cancel.cancel(); gtk::glib::Propagation::Proceed
        }));
        crate::core::worker::work(move || engine.compare_conflict(&id, &record, &cancel), move |result| {
            if let (Some(window), Some(handler)) = (weak_window.upgrade(), handler) { window.disconnect(handler); }
            center.loading.set(false);
            center.navigation.set_sensitive(true);
            center.list.set_sensitive(true);
            center.refresh.set_sensitive(true);
            match result {
                Ok(comparison) => {
                    center.status.set_label(&gettextrs::gettext("Snapshots verified. No decision was accepted. Choices still require a live revalidation."));
                    if let Some(window) = center.window.upgrade().filter(|w| w.is_visible()) {
                        let content = super::comparison_page::content(&window, &center.account, &comparison);
                        center.push("Compare observed versions", &content);
                    }
                }
                Err(error) => center.status.set_label(&gettextrs::gettext("Comparison unavailable: {error}. Original versions and the pending item remain. Use Check current versions and review again.").replace("{error}", gettextrs::gettext(error.to_string()).trim_end_matches('.'))),
            }
        });
    });
    buttons.append(&compare);
    let merge = gtk::Button::with_label(&gettextrs::gettext("Review plain-text merge…"));
    let merge_record = record.clone();
    let weak = Rc::downgrade(center);
    merge.connect_clicked(move |_| {
        let Some(center) = weak.upgrade() else { return; };
        if center.busy.get() || center.loading.get() || center.job.running() { return; }
        center.loading.set(true);
        center.navigation.set_sensitive(false);
        center.list.set_sensitive(false);
        center.refresh.set_sensitive(false);
        center.status.set_label(&gettextrs::gettext("Verifying the retained common base and both observed versions…"));
        let engine = center.engine.clone();
        let id = center.id.clone();
        let record = merge_record.clone();
        let cancel = drivesync_engine::Cancellation::new();
        let weak_window = center.window.clone();
        let close_cancel = cancel.clone();
        let handler = weak_window.upgrade().map(|window| window.connect_close_request(move |_| { close_cancel.cancel(); gtk::glib::Propagation::Proceed }));
        crate::core::worker::work(move || engine.preview_text_merge(&id, &record, &cancel), {
            let record = merge_record.clone();
            move |result| {
                if let (Some(window), Some(handler)) = (weak_window.upgrade(), handler) { window.disconnect(handler); }
                center.loading.set(false);
                center.navigation.set_sensitive(true);
                center.list.set_sensitive(true);
                center.refresh.set_sensitive(true);
                match result {
                    Ok(candidate) => {
                        center.status.set_label(&gettextrs::gettext("Plain-text candidate verified for review. No decision accepted."));
                        if center.window.upgrade().is_some_and(|w| w.is_visible()) {
                            let (content, apply) = super::text_merge_page::page(&center.account, &record, &candidate);
                            let weak = Rc::downgrade(&center);
                            apply.connect_clicked(move |_| {
                                if let Some(center) = weak.upgrade() { center.run(ResolutionTask::Apply(Box::new(candidate.accept_reviewed()))); }
                            });
                            center.push("Review plain-text merge", &content);
                        }
                    }
                    Err(error) => center.status.set_label(&format!("Text merge unavailable: {error}. A retained verified common base and compatible plain text are required. Offline, overlap, unsupported content or stale versions keep the conflict pending. You can compare versions or decide later.")),
                }
            }
        });
    });
    if !matches!(
        record.diagnosis,
        drivesync_engine::ChangeKind::LocalIdentityChanged
            | drivesync_engine::ChangeKind::ConcurrentIdentityChanges
    ) {
        buttons.append(&merge);
    }
    let selection = Rc::new(Cell::new(None));
    let apply = gtk::Button::with_label(&gettextrs::gettext("Confirm choice"));
    apply.add_css_class("suggested-action");
    apply.set_sensitive(false);
    let mut first = None;
    for choice in [
        ResolutionChoice::KeepBoth,
        ResolutionChoice::KeepLocal,
        ResolutionChoice::KeepRemote,
    ] {
        let option = gtk::CheckButton::new();
        option.set_group(first.as_ref());
        if first.is_none() {
            first = Some(option.clone());
        }
        let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
        text.append(
            &gtk::Label::builder()
                .label(gettextrs::gettext(choice_label(choice)))
                .wrap(true)
                .xalign(0.0)
                .build(),
        );
        text.append(
            &gtk::Label::builder()
                .label(gettextrs::gettext(choice_effect(choice)))
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .xalign(0.0)
                .build(),
        );
        option.set_child(Some(&text));
        let selected = selection.clone();
        let weak = apply.downgrade();
        option.connect_toggled(move |button| {
            if button.is_active() {
                selected.set(Some(choice));
                if let Some(apply) = weak.upgrade() {
                    apply.set_sensitive(true);
                }
            }
        });
        if choice == ResolutionChoice::KeepBoth {
            option.set_active(true);
        }
        buttons.append(&option);
    }
    let record = record.clone();
    let weak = Rc::downgrade(center);
    apply.connect_clicked(move |_| {
        let Some(center) = weak.upgrade() else {
            return;
        };
        let Some(choice) = selection.get() else {
            return;
        };
        if let Ok(request) = ResolutionRequest::from_record(&record, choice) {
            center.run(ResolutionTask::Apply(Box::new(request)));
        }
    });
    buttons.append(&apply);
    row.append(&buttons);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises navigation, consent and job exclusion without remote mutations.
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn review_navigation_keeps_one_window_and_requires_explicit_confirmation() {
        if let Ok(locale) = std::env::var("DRIVESYNC_RESOLUTION_UI_LOCALE") {
            // This ignored test runs alone in a dedicated process before GTK.
            crate::core::i18n::init();
            gettextrs::bindtextdomain("drivesync", locale).unwrap();
            gettextrs::textdomain("drivesync").unwrap();
        }
        adw::init().unwrap();
        let window = adw::Window::builder()
            .default_width(360)
            .default_height(640)
            .build();
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
        let navigation = adw::NavigationView::new();
        navigation.set_animate_transitions(false);
        let root_page = adw::NavigationPage::new(&list, "Conflicts");
        navigation.add(&root_page);
        window.set_content(Some(&navigation));
        let center = Rc::new(Center {
            kind: None,
            engine,
            id: account.id,
            account: "test@example.invalid".into(),
            job,
            window: window.downgrade(),
            list: list.clone(),
            navigation: navigation.clone(),
            root_page: root_page.clone(),
            status: gtk::Label::new(None),
            refresh: gtk::Button::new(),
            later: gtk::Button::new(),
            pause: gtk::Button::new(),
            busy: Cell::new(false),
            loading: Cell::new(false),
        });
        let record = ConflictRecord {
            conflict_id: "a".repeat(64),
            remote_id: Some("file".into()),
            remote_parent_id: Some("root".into()),
            remote_name: Some("<report>&.txt".into()),
            relative_path: Some("<report>&.txt".into()),
            diagnosis: drivesync_engine::ChangeKind::LocalIdentityChanged,
            status: drivesync_engine::ConflictStatus::Pending,
            local_size: Some(10),
            local_modified_unix: None,
            remote_size: Some(20),
            local_fingerprint: Some(format!("{}:1:2", "b".repeat(64))),
            remote_version: Some(2),
            outcomes: vec![],
        };
        let items = models::pending(&[record]);
        present_items(&list, &items, Some(&center));
        window.present();
        let count = gtk::Window::list_toplevels().len();
        center.detail(&items[0]);
        assert_eq!(navigation.navigation_stack().n_items(), 2);
        let settle = gtk::glib::MainLoop::new(None, false);
        let quit = settle.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            quit.quit()
        });
        settle.run();
        assert!(window.width() <= 360, "detail must fit a narrow window");
        if let Ok(output) = std::env::var("DRIVESYNC_RESOLUTION_UI_OUTPUT") {
            std::fs::create_dir_all(&output).unwrap();
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(&window)).snapshot(
                &snapshot,
                window.width() as f64,
                window.height() as f64,
            );
            window
                .renderer()
                .unwrap()
                .render_texture(snapshot.to_node().unwrap(), None)
                .save_to_png(std::path::Path::new(&output).join("navigation-detail-360.png"))
                .unwrap();
        }

        assert_eq!(gtk::Window::list_toplevels().len(), count);
        fn collect(
            widget: &gtk::Widget,
            options: &mut Vec<gtk::CheckButton>,
            buttons: &mut Vec<gtk::Button>,
        ) {
            if let Some(button) = widget.downcast_ref::<gtk::CheckButton>() {
                options.push(button.clone());
            }
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                buttons.push(button.clone());
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                collect(&current, options, buttons);
                child = current.next_sibling();
            }
        }
        let mut options = vec![];
        let mut buttons = vec![];
        collect(
            navigation.visible_page().unwrap().upcast_ref(),
            &mut options,
            &mut buttons,
        );
        let apply = buttons
            .iter()
            .find(|b| b.label().as_deref() == Some(gettextrs::gettext("Confirm choice").as_str()))
            .unwrap();
        assert_eq!(options.len(), 3);
        assert!(options[0].is_active());
        assert!(options[1..].iter().all(|b| !b.is_active()));
        assert!(apply.is_sensitive());
        options[0].set_active(true);
        assert!(apply.is_sensitive());
        options[1].set_active(true);
        assert!(!options[0].is_active());
        assert!(!center.job.running(), "selection alone must not dispatch");
        center.busy.set(true);
        apply.emit_clicked();
        assert!(!center.job.running(), "a busy page cannot dispatch again");
        center.busy.set(false);
        assert!(navigation.pop());
        assert_eq!(navigation.visible_page().as_ref(), Some(&root_page));
        assert!(!center.job.running(), "back never accepts a choice");
        center.detail(&models::Item {
            record: None,
            ..items[0].clone()
        });
        let mut options = vec![];
        let mut buttons = vec![];
        collect(
            navigation.visible_page().unwrap().upcast_ref(),
            &mut options,
            &mut buttons,
        );
        assert!(
            options.is_empty(),
            "unsupported observations have no version choices"
        );
        assert_eq!(gtk::Window::list_toplevels().len(), count);
        window.close();
    }
}
