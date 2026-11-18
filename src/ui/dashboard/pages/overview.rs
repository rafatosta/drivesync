//! Adaptive GNOME account navigation; account settings remain in Preferences.
use super::sidebar::AccountSidebar;
use crate::application::Context;
use crate::ui::components::alerts::{AlertDetails, AlertItem, Alerts, ProblemCategory};
use crate::ui::components::sync_status::{SyncActivity, SyncStatus, SyncStatusData};
use adw::{gtk, prelude::*};
use drivesync_engine::{
    ListAccountsRequest, SynchronizationAction, SynchronizationActivity, SynchronizationIssueKind,
    SynchronizationProgressUnit, SynchronizationState, SynchronizationStatus,
    SynchronizationWaitReason, SynchronizationWorkDetail,
};
use gettextrs::gettext;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(crate) fn build(context: &Context, navigation: &adw::NavigationView) -> adw::NavigationPage {
    build_with_accounts(context, navigation, Vec::new())
}
fn build_with_accounts(
    context: &Context,
    navigation: &adw::NavigationView,
    accounts: Vec<drivesync_engine::Account>,
) -> adw::NavigationPage {
    let split = adw::OverlaySplitView::builder()
        .min_sidebar_width(240.0)
        .max_sidebar_width(320.0)
        .sidebar_width_fraction(0.3)
        .build();
    split.set_widget_name("dashboard-split");
    let feedback = gtk::Label::builder()
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    feedback.set_widget_name("dashboard-accounts-feedback");
    let sidebar = AccountSidebar::new().with_menu({
        let context = context.clone();
        let navigation = navigation.downgrade();
        let feedback = feedback.downgrade();
        move |account| super::account_menu::build(&context, &navigation, &feedback, account)
    });
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.set_title_widget(Some(&adw::WindowTitle::new("DriveSync", "")));
    header.pack_start(&crate::ui::components::main_menu::add_account_button());
    header.pack_end(&crate::ui::components::main_menu::button());
    let developer_console = gtk::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text("Developer Console (F12)")
        .action_name("win.developer-console")
        .build();
    header.pack_end(&developer_console);
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&sidebar.heading);
    body.append(&sidebar.list);
    body.append(&feedback);
    toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build(),
    ));
    let version = gtk::Label::builder()
        .label(crate::core::build_info::VERSION)
        .tooltip_text(format!(
            "Source revision: {}",
            crate::core::build_info::REVISION
        ))
        .css_classes(["dim-label", "caption"])
        .selectable(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    version.set_widget_name("dashboard-build-version");
    toolbar.add_bottom_bar(&version);
    split.set_sidebar(Some(&adw::NavigationPage::new(&toolbar, "DriveSync")));

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    let title = adw::WindowTitle::new("DriveSync", "");
    title.set_widget_name("dashboard-account-title");
    header.set_title_widget(Some(&title));
    let sidebar_toggle = gtk::ToggleButton::builder()
        .icon_name("adw-sidebar-symbolic")
        .tooltip_text(gettext("Toggle sidebar"))
        .css_classes(["flat"])
        .build();
    sidebar_toggle.set_widget_name("dashboard-sidebar-toggle");
    split
        .bind_property("show-sidebar", &sidebar_toggle, "active")
        .bidirectional()
        .sync_create()
        .build();
    header.pack_start(&sidebar_toggle);
    toolbar.add_top_bar(&header);
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty.set_widget_name("dashboard-content");
    empty.set_margin_top(24);
    empty.set_margin_bottom(24);
    empty.set_margin_start(24);
    empty.set_margin_end(24);
    let status = SyncStatus::new();
    status.widget().set_visible(false);
    empty.append(status.widget());
    let alerts = Rc::new(Alerts::new(navigation, |_, _| {}));
    empty.append(alerts.widget());
    toolbar.set_content(Some(&empty));
    split.set_content(Some(&adw::NavigationPage::new(&toolbar, "DriveSync")));
    let status_subscription = Rc::new(RefCell::new(None::<gtk::glib::JoinHandle<()>>));
    let visible_alerts = Rc::new(RefCell::new(Vec::<AlertItem>::new()));
    sidebar.on_selected({
        let title = title.downgrade();
        let status = status.clone();
        let status_subscription = status_subscription.clone();
        let alerts = alerts.clone();
        let visible_alerts = visible_alerts.clone();
        let context = context.clone();
        move |account| {
            if let Some(title) = title.upgrade() {
                title.set_title(
                    account
                        .as_ref()
                        .and_then(|account| {
                            account.display_name.as_deref().or(account.email.as_deref())
                        })
                        .unwrap_or("DriveSync"),
                );
                title.set_subtitle(
                    account
                        .as_ref()
                        .and_then(|account| account.email.as_deref())
                        .unwrap_or(""),
                );
            }
            if let Some(subscription) = status_subscription.borrow_mut().take() {
                subscription.abort();
            }
            let Some(account) = account else {
                status.widget().set_visible(false);
                visible_alerts.borrow_mut().clear();
                alerts.update(&[]);
                return;
            };
            status.update(&SyncStatusData::default());
            visible_alerts.borrow_mut().clear();
            alerts.update(&[]);
            let Some(engine) = context.engine.borrow().clone() else {
                return;
            };
            let mut subscription = engine.observe_synchronization(&account.id);
            let account_label = account
                .display_name
                .as_deref()
                .or(account.email.as_deref())
                .unwrap_or("DriveSync")
                .to_owned();
            let snapshot = subscription.snapshot();
            status.update(&presentation_status(&snapshot));
            let initial_alerts = issue_alerts(&snapshot, &account.id.0, &account_label);
            *visible_alerts.borrow_mut() = initial_alerts.clone();
            alerts.update(&initial_alerts);
            let status = status.clone();
            let alerts = alerts.clone();
            let visible_alerts = visible_alerts.clone();
            *status_subscription.borrow_mut() = Some(gtk::glib::spawn_future_local(async move {
                while subscription.changed().await {
                    let snapshot = subscription.snapshot();
                    status.update(&presentation_status(&snapshot));
                    let next_alerts = issue_alerts(&snapshot, &account.id.0, &account_label);
                    if *visible_alerts.borrow() != next_alerts {
                        *visible_alerts.borrow_mut() = next_alerts.clone();
                        alerts.update(&next_alerts);
                    }
                }
            }));
        }
    });
    sidebar.set_accounts(accounts);
    let weak_split = split.downgrade();
    sidebar.list.connect_row_activated(move |_, _| {
        if let Some(split) = weak_split.upgrade() {
            if split.is_collapsed() {
                split.set_show_sidebar(false);
            }
        }
    });

    let generation = Rc::new(Cell::new(0_u64));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let sidebar = sidebar.downgrade();
        let feedback = feedback.downgrade();
        move || {
            let Some(sidebar) = sidebar.upgrade() else {
                return;
            };
            let Some(engine) = context.engine.borrow().clone() else {
                return;
            };
            let request = generation.get().wrapping_add(1);
            generation.set(request);
            let generation = generation.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                let result = async {
                    let result = engine.list_accounts(ListAccountsRequest { provider: None }).await
                        .map_err(|error| crate::core::messages::engine_error(&error))?;
                    let mut accounts = Vec::new();
                    for account in result.accounts {
                        match crate::services::configuration::load(&account.id).await {
                            Ok(Some(_)) => accounts.push(account),
                            Ok(None) => (),
                            Err(_) => return Err(gettext("Saved configuration could not be read. Your account has been preserved.")),
                        }
                    }
                    Ok(accounts)
                }.await;
                if generation.get() != request {
                    return;
                }
                let Some(feedback) = feedback.upgrade() else {
                    return;
                };
                match result {
                    Ok(accounts) => {
                        sidebar.set_accounts(accounts);
                        feedback.set_label("");
                    }
                    Err(message) => feedback.set_label(&message),
                }
            });
        }
    });
    let on_map = refresh.clone();
    sidebar.list.connect_map(move |_| on_map());
    let group = gtk::gio::SimpleActionGroup::new();
    let action = gtk::gio::SimpleAction::new("refresh", None);
    action.connect_activate(move |_, _| refresh());
    group.add_action(&action);
    navigation.insert_action_group("dashboard", Some(&group));

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    let bin = adw::BreakpointBin::new();
    bin.set_size_request(320, 320);
    bin.set_child(Some(&split));
    bin.add_breakpoint(breakpoint);
    let page = adw::NavigationPage::new(&bin, "DriveSync");
    page.set_widget_name("drivesync-dashboard");
    page
}

/// Converts the engine's account snapshot into the reusable GTK presentation model.
fn presentation_status(status: &SynchronizationStatus) -> SyncStatusData {
    use SynchronizationActivity as EngineActivity;
    let activity = match status.state {
        SynchronizationState::Waiting => SyncActivity::Waiting,
        SynchronizationState::Stopped => SyncActivity::Stopped,
        SynchronizationState::Failed => SyncActivity::Failed,
        SynchronizationState::Idle => SyncActivity::Idle,
        SynchronizationState::Running => match status.work.as_ref().map(|work| work.action) {
            Some(SynchronizationAction::Upload) => SyncActivity::Upload,
            Some(SynchronizationAction::Download) => SyncActivity::Download,
            Some(SynchronizationAction::ObserveLocal) => SyncActivity::Scanning,
            Some(SynchronizationAction::ObserveRemote) => SyncActivity::CheckingRemote,
            Some(SynchronizationAction::Rename) => SyncActivity::Renaming,
            Some(SynchronizationAction::Move | SynchronizationAction::Restore) => {
                SyncActivity::Moving
            }
            Some(SynchronizationAction::Trash) => SyncActivity::Deleting,
            Some(SynchronizationAction::Analyze | SynchronizationAction::Reconcile) => {
                SyncActivity::Reconciling
            }
            Some(
                SynchronizationAction::CreateFile
                | SynchronizationAction::CreateDirectory
                | SynchronizationAction::UpdateContent
                | SynchronizationAction::Verify,
            ) => SyncActivity::Reconciling,
            None => match status.activity {
                EngineActivity::Idle => SyncActivity::Idle,
                EngineActivity::Starting | EngineActivity::Scanning => SyncActivity::Scanning,
                EngineActivity::Analyzing
                | EngineActivity::ProcessingEvents
                | EngineActivity::Checking
                | EngineActivity::Applying
                | EngineActivity::Verifying => SyncActivity::Reconciling,
                EngineActivity::Uploading => SyncActivity::Upload,
                EngineActivity::Downloading => SyncActivity::Download,
                EngineActivity::Waiting => SyncActivity::Waiting,
                EngineActivity::Stopped => SyncActivity::Stopped,
                EngineActivity::Failed => SyncActivity::Failed,
            },
        },
    };
    let (transferred_bytes, total_bytes) = match status.progress {
        Some(progress) if progress.unit == SynchronizationProgressUnit::Bytes => {
            (Some(progress.completed), progress.total)
        }
        _ => (None, None),
    };
    SyncStatusData {
        activity,
        detail: work_detail(status),
        transferred_bytes,
        total_bytes,
        problems: status.issue.as_ref().map_or(0, |issue| issue.count),
        ..Default::default()
    }
}

fn work_detail(status: &SynchronizationStatus) -> Option<String> {
    let work = status.work.as_ref()?;
    let name = work.item.as_ref().and_then(item_name);
    let action = match work.action {
        SynchronizationAction::ObserveLocal => "Checking local files",
        SynchronizationAction::ObserveRemote => "Checking cloud changes",
        SynchronizationAction::Analyze => "Analyzing",
        SynchronizationAction::Reconcile => "Syncing changes",
        SynchronizationAction::Upload => "Uploading",
        SynchronizationAction::Download => "Downloading",
        SynchronizationAction::CreateFile => "Creating file",
        SynchronizationAction::CreateDirectory => "Creating folder",
        SynchronizationAction::UpdateContent => "Updating file",
        SynchronizationAction::Rename => "Renaming",
        SynchronizationAction::Move => "Moving",
        SynchronizationAction::Trash => "Moving to trash",
        SynchronizationAction::Restore => "Restoring",
        SynchronizationAction::Verify => "Verifying",
    };
    let phase = match work.detail {
        SynchronizationWorkDetail::Preparation => None,
        SynchronizationWorkDetail::Analysis(phase) => Some(match phase {
            drivesync_engine::analyzer::AnalysisPhase::Validating => "Checking file information",
            drivesync_engine::analyzer::AnalysisPhase::Reconciling => "Reconciling file versions",
            drivesync_engine::analyzer::AnalysisPhase::EvaluatingConflict => {
                "Checking for conflicts"
            }
            drivesync_engine::analyzer::AnalysisPhase::Planning => "Planning changes",
            drivesync_engine::analyzer::AnalysisPhase::Recording => "Recording changes",
            drivesync_engine::analyzer::AnalysisPhase::Executing => "Applying changes",
            drivesync_engine::analyzer::AnalysisPhase::Verifying => "Verifying changes",
        }),
        SynchronizationWorkDetail::Creation(stage) => Some(match stage {
            drivesync_engine::analyzer::CreationStage::Checking => "Checking prerequisites",
            drivesync_engine::analyzer::CreationStage::Blocked => "Waiting for a prerequisite",
            drivesync_engine::analyzer::CreationStage::Executing => "Executing operation",
            drivesync_engine::analyzer::CreationStage::Verifying => "Verifying result",
            drivesync_engine::analyzer::CreationStage::Completed => "Operation completed",
            drivesync_engine::analyzer::CreationStage::Failed => "Operation failed",
        }),
        SynchronizationWorkDetail::Transfer(phase) => Some(match phase {
            drivesync_engine::operations::ExecutionPhase::Preparing => "Preparing transfer",
            drivesync_engine::operations::ExecutionPhase::Transferring => "Transferring file",
            drivesync_engine::operations::ExecutionPhase::Applying => "Saving file",
            drivesync_engine::operations::ExecutionPhase::Verifying => "Verifying file",
            drivesync_engine::operations::ExecutionPhase::Finalizing => "Finishing transfer",
        }),
        SynchronizationWorkDetail::Waiting(reason) => Some(match reason {
            SynchronizationWaitReason::ForChanges => "Waiting for file changes",
            SynchronizationWaitReason::RetryBackoff => "Waiting before retrying",
            SynchronizationWaitReason::SourceRecovery => "Waiting for the source to recover",
            SynchronizationWaitReason::IncompleteEventBatch => "Waiting to retry detected changes",
        }),
    };

    let mut detail = match name {
        Some(name) => gettext("{action}: {file}")
            .replace("{action}", &gettext(action))
            .replace("{file}", &name),
        None => gettext(action),
    };
    if let Some(phase) = phase {
        detail.push_str(" · ");
        detail.push_str(&gettext(phase));
    }
    Some(detail)
}

fn issue_alerts(status: &SynchronizationStatus, account_id: &str, account: &str) -> Vec<AlertItem> {
    let Some(issue) = &status.issue else {
        return Vec::new();
    };
    let category = ProblemCategory::Synchronization;
    let title = issue
        .item
        .as_ref()
        .and_then(item_name)
        .unwrap_or_else(|| category.title());
    let detail = issue_detail(issue);
    vec![AlertItem {
        id: format!("{account_id}/synchronization-issue"),
        account: account.to_owned(),
        title,
        detail,
        details: AlertDetails::Problem(category),
    }]
}

fn issue_detail(issue: &drivesync_engine::SynchronizationIssue) -> String {
    let message = match &issue.kind {
        SynchronizationIssueKind::Detector(_) => "A synchronization source needs attention.",
        SynchronizationIssueKind::LocalSource(_) => "The local folder could not be read.",
        SynchronizationIssueKind::RemoteSource(
            drivesync_engine::detectors::RemoteError::AuthorizationRequired,
        ) => "Reconnect the account to access the cloud source.",
        SynchronizationIssueKind::RemoteSource(
            drivesync_engine::detectors::RemoteError::PermissionDenied,
        ) => "The account does not have permission to access this item.",
        SynchronizationIssueKind::RemoteSource(_) => "The cloud source is unavailable.",
        SynchronizationIssueKind::Inventory(_) => {
            "Synchronization data could not be read or saved."
        }
        SynchronizationIssueKind::Queue(_) => "A detected change could not be saved.",
        SynchronizationIssueKind::Analyzer(_) => "A file could not be analyzed.",
        SynchronizationIssueKind::Operation(
            drivesync_engine::operations::OperatorError::UnsupportedOperation,
        ) => "This file operation is not supported by the provider.",
        SynchronizationIssueKind::Operation(
            drivesync_engine::operations::OperatorError::PermissionDenied,
        ) => "Permission is required to complete this file operation.",
        SynchronizationIssueKind::Operation(
            drivesync_engine::operations::OperatorError::InsufficientSpace,
        ) => "There is not enough storage to complete this file operation.",
        SynchronizationIssueKind::Operation(
            drivesync_engine::operations::OperatorError::Network,
        ) => "A network problem prevented this file operation.",
        SynchronizationIssueKind::Operation(_) => "A file operation could not be completed.",
        SynchronizationIssueKind::Internal => "Synchronization stopped unexpectedly.",
    };
    let mut detail = gettext(message);
    if issue.count > 1 {
        detail.push(' ');
        detail.push_str(
            &gettext("This issue affects {count} items.")
                .replace("{count}", &issue.count.to_string()),
        );
    }
    if issue.retryable {
        detail.push(' ');
        detail.push_str(&gettext("Will retry automatically."));
    }
    detail
}

fn item_name(item: &drivesync_engine::SynchronizationItem) -> Option<String> {
    item.local_name
        .as_ref()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| item.remote_name.clone())
}

#[cfg(test)]
pub(crate) fn fixture(
    navigation: &adw::NavigationView,
    account: &drivesync_engine::Account,
) -> adw::NavigationPage {
    super::sidebar::exercise(account);
    let mut second = account.clone();
    second.id = drivesync_engine::AccountId("sidebar-work".into());
    second.display_name = Some("Work".into());
    second.email = Some("work@example.invalid".into());
    build_with_accounts(
        &Context::default(),
        navigation,
        vec![account.clone(), second],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{
        SynchronizationIssue, SynchronizationIssueKind, SynchronizationItem,
        SynchronizationLocation, SynchronizationProgress, SynchronizationProgressUnit,
        SynchronizationWork,
    };

    #[test]
    fn download_work_keeps_its_filename_during_verification() {
        let mut status = SynchronizationStatus {
            state: SynchronizationState::Running,
            activity: SynchronizationActivity::Verifying,
            work: Some(SynchronizationWork {
                action: SynchronizationAction::Download,
                item: Some(SynchronizationItem {
                    id: None,
                    kind: None,
                    local_name: Some("teste-2048mb.bin".into()),
                    remote_name: None,
                    local_path: None,
                    remote_reference: None,
                }),
                location: Some(SynchronizationLocation::LocalRoot),
                detail: SynchronizationWorkDetail::Transfer(
                    drivesync_engine::operations::ExecutionPhase::Verifying,
                ),
                event: None,
            }),
            progress: Some(SynchronizationProgress {
                completed: 5,
                total: Some(100),
                unit: SynchronizationProgressUnit::Bytes,
                phase: Some(drivesync_engine::operations::ExecutionPhase::Verifying),
            }),
            ..Default::default()
        };

        let presentation = presentation_status(&status);
        assert_eq!(presentation.activity, SyncActivity::Download);
        assert_eq!(presentation.transferred_bytes, Some(5));
        assert!(presentation
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("teste-2048mb.bin")));

        status.state = SynchronizationState::Waiting;
        status.activity = SynchronizationActivity::Waiting;
        status.work.as_mut().unwrap().detail =
            SynchronizationWorkDetail::Waiting(SynchronizationWaitReason::RetryBackoff);
        assert_eq!(presentation_status(&status).activity, SyncActivity::Waiting);
    }

    #[test]
    fn operation_issue_is_a_separate_problem_item_while_download_stays_active() {
        let item = SynchronizationItem {
            id: None,
            kind: None,
            local_name: Some("arquivo-sem-extensao".into()),
            remote_name: None,
            local_path: None,
            remote_reference: None,
        };
        let status = SynchronizationStatus {
            state: SynchronizationState::Running,
            activity: SynchronizationActivity::Downloading,
            work: Some(SynchronizationWork {
                action: SynchronizationAction::Download,
                item: Some(item.clone()),
                location: Some(SynchronizationLocation::LocalRoot),
                detail: SynchronizationWorkDetail::Transfer(
                    drivesync_engine::operations::ExecutionPhase::Transferring,
                ),
                event: None,
            }),
            issue: Some(SynchronizationIssue {
                kind: SynchronizationIssueKind::Operation(
                    drivesync_engine::operations::OperatorError::UnsupportedOperation,
                ),
                retryable: false,
                item: Some(item),
                count: 1,
            }),
            ..Default::default()
        };

        let main_card = presentation_status(&status);
        assert_eq!(main_card.activity, SyncActivity::Download);
        assert!(main_card
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("arquivo-sem-extensao")));
        assert_eq!(main_card.problems, 1);

        let problems = issue_alerts(&status, "account-id", "Personal account");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].title, "arquivo-sem-extensao");
        assert!(problems[0].detail.contains("not supported"));
        assert!(matches!(
            problems[0].details,
            AlertDetails::Problem(ProblemCategory::Synchronization)
        ));
    }
}
