//! Real persisted decisions, account switching and adaptive GTK preview in an isolated profile.
use super::*;
use drivesync_engine::models::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn record() -> RecordedAnalysis {
    RecordedAnalysis {
        plan: None,
        id: AnalysisId(1),
        item_id: SyncItemId("item".into()),
        item_kind: ItemKind::File,
        analyzed_item_revision: 1,
        resulting_item_revision: 2,
        input_fingerprint: vec![0; 32],
        recorded_at: Timestamp { unix_millis: 1 },
        evidence: AnalysisEvidence {
            baseline_confirmed: false,
            local_scope_complete: false,
            remote_scope_complete: true,
            local_content: EvidenceRelation::Unknown,
            remote_content: EvidenceRelation::Unknown,
            current_content: EvidenceRelation::Unknown,
            local_parent: None,
            current_parents: EvidenceRelation::Unknown,
        },
        disposition: AnalysisDisposition::PendingOperations,
        findings: vec![],
        requirements: vec![],
        decisions: vec![AnalysisDecision {
            intent: AnalysisIntent {
                direction: OperationDirection::Download,
                kind: OperationKind::Create,
            },
            operation_id: OperationId(1),
        }],
        stale: false,
    }
}
#[test]
fn operation_categories_respect_conflicts_evidence_and_obsolete_decisions() {
    let mut r = record();
    assert_eq!(operation_category(&r), "Create file");
    assert_eq!(operation_direction(&r), Some("Download"));
    r.decisions[0].intent.direction = OperationDirection::Upload;
    assert_eq!(operation_category(&r), "Create file");
    assert_eq!(operation_direction(&r), Some("Upload"));
    r.decisions.push(AnalysisDecision {
        intent: AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Rename,
        },
        operation_id: OperationId(2),
    });
    assert_eq!(operation_category(&r), "Create file · Rename");
    assert_eq!(operation_direction(&r), Some("Upload + Download"));
    r.disposition = AnalysisDisposition::PendingUserDecision;
    assert_eq!(operation_category(&r), "Manual action");
    assert_eq!(operation_direction(&r), None);
    r.disposition = AnalysisDisposition::PendingEvidence;
    assert_eq!(operation_category(&r), "Create file · Rename");
    assert_eq!(operation_direction(&r), Some("Upload + Download"));
    r.decisions.clear();
    assert_eq!(operation_category(&r), "Awaiting evidence");
    r.stale = true;
    assert_eq!(operation_category(&r), "Awaiting analysis");
    r.stale = false;
    r.disposition = AnalysisDisposition::NoChanges;
    assert_eq!(operation_category(&r), "No action needed");
}
#[test]
fn operation_kinds_are_independent_of_direction_and_preserve_multiple_actions() {
    let mut r = record();
    for direction in [OperationDirection::Upload, OperationDirection::Download] {
        for (kind, expected) in [
            (OperationKind::Create, "Create file"),
            (OperationKind::UpdateContent, "Update content"),
            (OperationKind::Rename, "Rename"),
            (OperationKind::Move, "Move"),
        ] {
            r.decisions[0].intent = AnalysisIntent {
                direction: direction.clone(),
                kind,
            };
            assert_eq!(operation_category(&r), expected);
        }
    }
    r.item_kind = ItemKind::Directory;
    r.decisions[0].intent.kind = OperationKind::Create;
    assert_eq!(operation_category(&r), "Create folder");
    r.decisions.push(AnalysisDecision {
        intent: AnalysisIntent {
            direction: OperationDirection::Download,
            kind: OperationKind::Move,
        },
        operation_id: OperationId(2),
    });
    assert_eq!(operation_category(&r), "Create folder · Move");
}
#[test]
fn file_actions_and_missing_evidence_are_presented_without_execution_claims() {
    let mut r = record();
    assert_eq!(actions(&r), "Download new file");
    assert_eq!(status(&r), "Pending · not executed");
    r.decisions[0].intent.direction = OperationDirection::Upload;
    assert_eq!(actions(&r), "Upload new file to Drive");
    r.item_kind = ItemKind::Directory;
    assert_eq!(actions(&r), "Create folder in Drive");
    r.disposition = AnalysisDisposition::PendingUserDecision;
    assert!(actions(&r).starts_with("Resolve conflict"));
    assert_eq!(status(&r), "User decision required");
    r.decisions.clear();
    r.disposition = AnalysisDisposition::PendingEvidence;
    r.requirements = vec![
        AnalysisRequirement::ObserveRemote,
        AnalysisRequirement::VerifyContent,
    ];
    assert_eq!(actions(&r), "Observe remote state · Verify file contents");
    r.stale = true;
    assert_eq!(status(&r), "Outdated");
    r.stale = false;
    r.disposition = AnalysisDisposition::NoChanges;
    assert_eq!(actions(&r), "No action needed");
    assert_eq!(status(&r), "No changes");
}
#[test]
fn inspection_never_initializes_a_missing_inventory_even_if_requested() {
    let root = std::env::temp_dir().join(format!("analyzer-readonly-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let result = analyzer::read(
        OpenInventoryRequest {
            local_root: root.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        },
        None,
        false,
    );
    assert_eq!(result.err().unwrap().kind, InventoryError::NotFound);
    assert!(!root.join(".drivesync").exists());
    std::fs::remove_dir(root).unwrap();
}
fn seed(root: &Path, account: &str, count: usize) -> OpenInventoryRequest {
    std::fs::create_dir_all(root).unwrap();
    let request = OpenInventoryRequest {
        local_root: root.into(),
        expected: SyncIdentity {
            sync_id: SyncId(format!("sync-{account}")),
            account_id: AccountId(account.into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: true,
    };
    let inventory = InventoryManager::open(request.clone()).unwrap().inventory;
    let names = [
        "Relatório trimestral.pdf",
        "Budget <draft> & review.xlsx",
        "Fotos da viagem.zip",
        "Notas arquivadas.txt",
        "Projetos/planejamento do próximo semestre.docx",
    ];
    for index in 0..count {
        let name = names[index % names.len()];
        let style = index % names.len();
        let reference = RemoteFileRef {
            account_id: AccountId(account.into()),
            file_id: RemoteFileId(format!("remote-{index}")),
        };
        let file = RemoteFile {
            reference: reference.clone(),
            provider: ProviderId("google".into()),
            name: MetadataValue::Known(name.into()),
            kind: MetadataValue::Known(RemoteEntryKind::File),
            mime_type: MetadataValue::NotCollected,
            parent_ids: MetadataValue::Known(vec![RemoteFileId("root".into())]),
            trashed: MetadataValue::Known(false),
            version: MetadataValue::Known("1".into()),
            created_time: MetadataValue::NotCollected,
            modified_time: MetadataValue::NotCollected,
            size: MetadataValue::NotCollected,
            md5_checksum: MetadataValue::NotCollected,
            sha256_checksum: MetadataValue::NotCollected,
            provider_data: MetadataValue::NotCollected,
        };
        let item = inventory
            .record_remote(RecordRemoteObservation {
                selector: ObservationTarget::NewItem,
                expected_revision: None,
                observation: drivesync_engine::inventory::RemoteObservation {
                    observed: File {
                        id: FileId(format!("event-{index}")),
                        causal_order: None,
                        sync_id: request.expected.sync_id.clone(),
                        observed_at: Timestamp { unix_millis: 1 },
                        coverage: ObservationCoverage::Partial,
                        scope: FileScope::Remote(reference.clone()),
                        data: Some(FileData::Remote(file)),
                        event: FileEvent {
                            action: FileEventAction::StateChanged,
                            local: None,
                            local_presence: None,
                            remote: Some(drivesync_engine::models::RemoteObservation {
                                source: RemoteObservationSource::Changes,
                                removed: Some(false),
                                time: None,
                                change_type: None,
                                drive_id: None,
                            }),
                            uncertainty: None,
                            previous: None,
                            current: Some(FileLocation::Remote {
                                reference,
                                parent_id: Some(RemoteFileId("root".into())),
                                name: Some(name.into()),
                            }),
                        },
                    },
                    presence: PresenceState::Present,
                    proof: ObservationProof {
                        source: ObservationSource::RemoteChange,
                        observed_at: Timestamp { unix_millis: 1 },
                        scan_or_generation: None,
                        complete_scope: false,
                    },
                },
            })
            .unwrap()
            .item;
        let mut report = record();
        report.disposition = match style {
            1 => AnalysisDisposition::PendingUserDecision,
            2 | 4 => AnalysisDisposition::PendingEvidence,
            3 => AnalysisDisposition::NoChanges,
            _ => AnalysisDisposition::PendingOperations,
        };
        let intent = match style {
            0 => Some(OperationKind::Create),
            4 => Some(OperationKind::Rename),
            _ => None,
        };
        let assessment = NewAnalysisAssessment {
            plan: None,
            disposition: report.disposition,
            findings: vec![AnalysisFinding {
                origin: AnalysisOrigin::Conflicts,
                dimension: AnalysisDimension::Content,
                status: if style == 1 {
                    AnalysisFindingStatus::Divergent
                } else if style == 2 || style == 4 {
                    AnalysisFindingStatus::Unknown
                } else {
                    AnalysisFindingStatus::Compatible
                },
                reason: if style == 1 {
                    AnalysisReason::ConcurrentChanges
                } else if style == 2 || style == 4 {
                    AnalysisReason::InsufficientEvidence
                } else {
                    AnalysisReason::Equivalent
                },
            }],
            requirements: if style == 2 || style == 4 {
                vec![AnalysisRequirement::VerifyContent]
            } else {
                vec![]
            },
            intents: intent
                .map(|kind| AnalysisIntent {
                    direction: OperationDirection::Download,
                    kind,
                })
                .into_iter()
                .collect(),
        };
        let receipt = inventory
            .record_analysis(RecordAnalysisRequest {
                expected_sync_id: request.expected.sync_id.clone(),
                expected_item: item.clone(),
                evidence: report.evidence,
                assessment,
                recorded_at: Timestamp { unix_millis: 2 },
            })
            .unwrap()
            .analysis;
        if style == 3 {
            inventory
                .update_item_status(UpdateItemStatusRequest {
                    item_id: item.id,
                    expected_revision: receipt.resulting_item_revision,
                    change: ItemStatusChange {
                        status: SyncStatus::PendingReconciliation,
                        conflict_reason: FieldUpdate::Clear,
                    },
                })
                .unwrap();
        }
    }
    OpenInventoryRequest {
        create_if_missing: false,
        ..request
    }
}
fn categories(widget: &gtk::Widget) -> Vec<String> {
    let mut result = Vec::new();
    if widget.widget_name() == "analyzer-operation-category" {
        result.push(
            widget
                .downcast_ref::<gtk::Label>()
                .unwrap()
                .text()
                .to_string(),
        );
    }
    let mut child = widget.first_child();
    while let Some(node) = child {
        result.extend(categories(&node));
        child = node.next_sibling();
    }
    result
}
fn wait(view: &View) {
    let context = gtk::glib::MainContext::default();
    let end = Instant::now() + Duration::from_secs(10);
    while view.state.borrow().loading {
        assert!(Instant::now() < end, "analysis read timed out");
        context.iteration(true);
    }
    let loop_ = gtk::glib::MainLoop::new(None, false);
    let done = loop_.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || done.quit());
    loop_.run();
}
fn capture(window: &adw::ApplicationWindow, name: &str) {
    if let Some(path) = std::env::var_os("DRIVESYNC_UI_PREVIEW_DIR") {
        std::fs::create_dir_all(&path).unwrap();
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        let node = snapshot.to_node().expect("Analyzer renders");
        window
            .renderer()
            .unwrap()
            .render_texture(&node, None)
            .save_to_png(PathBuf::from(path).join(format!("{name}.png")))
            .unwrap();
    }
}
#[test]
#[ignore = "requires isolated GTK display and XDG profile"]
fn analyzer_page_shows_persisted_files_actions_and_account_isolation() {
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let base = PathBuf::from(&profile).join("analyzer-preview");
    let first = seed(&base.join("personal"), "personal", 5);
    let mut second = first.clone();
    second.local_root = base.join("work");
    second.expected.sync_id = SyncId("sync-work".into());
    second.expected.account_id = AccountId("work".into());
    second.create_if_missing = true;
    std::fs::create_dir_all(&second.local_root).unwrap();
    drop(InventoryManager::open(second.clone()).unwrap());
    second.create_if_missing = false;
    let (root, view) = create(&Context::default());
    view.loading_accounts.set(true);
    view.state.borrow_mut().scopes = vec![
        Scope {
            account_id: "personal".into(),
            label: "Personal · personal@example.invalid".into(),
            request: Ok(first.clone()),
        },
        Scope {
            account_id: "work".into(),
            label: "Work · work@example.invalid".into(),
            request: Ok(second),
        },
    ];
    view.accounts.set_model(Some(&gtk::StringList::new(&[
        "Personal · personal@example.invalid",
        "Work · work@example.invalid",
    ])));
    view.accounts.set_selected(0);
    view.loading_accounts.set(false);
    let app = adw::Application::builder()
        .application_id("org.example.AnalyzerPreview")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let window = adw::ApplicationWindow::builder()
        .application(&app)
        .title("Developer Console · Analyzer")
        .default_width(960)
        .default_height(860)
        .content(&root)
        .build();
    window.present();
    view.reset();
    wait(&view);
    assert_eq!(
        view.list.observe_children().n_items(),
        4,
        "{}",
        view.message.text()
    );
    assert!(view.range.text().contains("4 analyses"));
    let displayed = categories(view.list.upcast_ref());
    assert!(displayed.contains(&"Create file".to_string()));
    assert!(displayed.contains(&"Manual action".to_string()));
    assert!(displayed.contains(&"Awaiting evidence".to_string()));
    let first_row = view.list.first_child().unwrap();
    assert!(first_row.width() <= root.width());
    let texts = analyzer::read(first.clone(), None, false)
        .unwrap()
        .entries
        .into_iter()
        .map(|(a, i)| (file_name(&i), actions(&a)))
        .collect::<Vec<_>>();
    assert!(texts
        .iter()
        .any(|(name, action)| name == "Relatório trimestral.pdf" && action == "Download new file"));
    assert!(texts.iter().any(
        |(name, action)| name == "Budget <draft> & review.xlsx" && action == "Resolve conflict"
    ));
    capture(&window, "analyzer-wide");
    window.set_default_size(430, 860);
    wait(&view);
    assert!(root.width() <= 450);
    capture(&window, "analyzer-narrow");
    view.outdated.set_active(true);
    wait(&view);
    assert_eq!(view.list.observe_children().n_items(), 5);
    assert!(categories(view.list.upcast_ref()).contains(&"Awaiting analysis".to_string()));
    view.accounts.set_selected(1);
    wait(&view);
    assert_eq!(view.list.observe_children().n_items(), 0);
    assert!(view.message.text().starts_with("No analyses"));
    view.accounts.set_selected(0);
    view.outdated.set_active(false);
    wait(&view);
    assert_eq!(
        view.list.observe_children().n_items(),
        4,
        "{}",
        view.message.text()
    );
    // An unavailable account must clear the preceding account's decisions.
    view.state.borrow_mut().scopes[1].request = Err("Folder unavailable".into());
    view.accounts.set_selected(1);
    wait(&view);
    assert_eq!(view.list.observe_children().n_items(), 0);
    assert_eq!(view.message.text(), "Folder unavailable");
    window.close();
}

#[test]
fn decision_reads_use_bounded_filter_bound_pages() {
    let root = std::env::temp_dir().join(format!(
        "analyzer-pages-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let request = seed(&root, "paging", 130);
    let first = analyzer::read(request.clone(), None, false).unwrap();
    assert_eq!(first.entries.len(), 100);
    let cursor = first.next_cursor.unwrap();
    let last = analyzer::read(request.clone(), Some(cursor.clone()), false).unwrap();
    assert_eq!(last.entries.len(), 4);
    assert!(last.next_cursor.is_none());
    assert!(last
        .entries
        .iter()
        .all(|(record, _)| !record.stale
            && !first.entries.iter().any(|(old, _)| old.id == record.id)));
    assert_eq!(
        analyzer::read(request, Some(cursor), true)
            .err()
            .unwrap()
            .kind,
        InventoryError::InvalidCursor
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires isolated GTK display and XDG profile"]
fn application_startup_delivers_live_files_to_the_visible_analyzer_without_manual_refresh() {
    use std::os::unix::fs::PermissionsExt;
    let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile");
    assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    let base = PathBuf::from(profile).join("live-startup");
    std::fs::create_dir_all(base.join("accounts")).unwrap();
    std::fs::set_permissions(
        base.join("accounts"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let id = "live@example.invalid";
    let accounts = base.join("accounts/accounts.json");
    std::fs::write(&accounts,serde_json::to_vec(&serde_json::json!({"version":1,"application":"org.example.LiveAnalyzerUI","intent":null,"records":{(id):{"account":{"id":id,"provider":"google","provider_user_id":id,"email":id,"display_name":"Personal","state":"ReauthorizationRequired","created_at":{"unix_millis":0},"updated_at":{"unix_millis":0}},"key":format!("google:{id}:synthetic")}}})).unwrap()).unwrap();
    std::fs::set_permissions(&accounts, std::fs::Permissions::from_mode(0o600)).unwrap();
    let root = base.join("drive");
    std::fs::create_dir(&root).unwrap();
    let configuration = crate::services::configuration::Configuration {
        account_id: AccountId(id.into()),
        local_uri: gtk::gio::File::for_path(&root).uri().into(),
        remote_id: "root".into(),
        remote_path: "My Drive".into(),
    };
    gtk::glib::MainContext::default().block_on(crate::services::configuration::seed_for_test(
        &configuration,
    ));
    let context = Context::default();
    let app = adw::Application::builder()
        .application_id("org.example.LiveAnalyzerUI")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let navigation = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&navigation));
    let window = adw::ApplicationWindow::builder()
        .application(&app)
        .default_width(960)
        .default_height(860)
        .content(&toasts)
        .build();
    // Same restoration hook as the real application; no manual prepare/scan/analyze call.
    navigation.add(&crate::ui::dashboard::restoration::build(
        drivesync_engine::EngineConfig {
            data_directory: base.join("accounts"),
            application_id: "org.example.LiveAnalyzerUI".into(),
            google_oauth_file: None,
        },
        &context,
        &navigation,
        &toasts,
    ));
    window.present();
    fn until(test: impl Fn() -> bool) {
        let main = gtk::glib::MainContext::default();
        let end = Instant::now() + Duration::from_secs(20);
        while !test() {
            assert!(Instant::now() < end, "live startup timed out");
            while main.pending() {
                main.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    until(|| {
        context.detectors.borrow().get(id).is_some_and(|session| {
            session
                .orchestrator
                .detector_status()
                .is_ok_and(|s| s.delivery == drivesync_engine::detectors::DeliveryState::Enabled)
        })
    });
    let (page, view) = create(&context);
    let retained = view.clone();
    page.connect_map(move |_| retained.reload());
    window.set_content(Some(&page));
    until(|| {
        !view.loading_accounts.get()
            && !view.state.borrow().loading
            && view.accounts.model().is_some()
    });
    std::fs::create_dir(root.join("New folder")).unwrap();
    std::fs::write(
        root.join("New folder/Created after opening.txt"),
        b"local user content",
    )
    .unwrap();
    until(|| view.list.observe_children().n_items() == 2 && !view.state.borrow().loading);
    fn text(widget: &gtk::Widget) -> String {
        let mut result = widget
            .downcast_ref::<gtk::Label>()
            .map(|l| l.text().to_string())
            .unwrap_or_default();
        let mut child = widget.first_child();
        while let Some(node) = child {
            result.push_str(&text(&node));
            child = node.next_sibling();
        }
        result
    }
    assert!(text(view.list.upcast_ref()).contains("New folder/Created after opening.txt"));
    let rows = text(view.list.upcast_ref());
    assert_eq!(
        categories(view.list.upcast_ref()),
        vec!["Create folder", "Create file"]
    );
    assert!(!rows.contains("Awaiting evidence"));
    assert!(rows.contains("Needs evidence"));
    let owner = context
        .detectors
        .borrow()
        .get(id)
        .unwrap()
        .orchestrator
        .clone();
    assert!(owner.preparation_status().events_processed > 0);
    assert!(owner.diagnostics().confirmed_events.unwrap() > 0);
    assert_eq!(
        std::fs::read(root.join("New folder/Created after opening.txt")).unwrap(),
        b"local user content"
    );
    wait(&view);
    capture(&window, "analyzer-live-startup");
    fn first_expander(widget: &gtk::Widget) -> Option<gtk::Expander> {
        if let Some(expander) = widget.downcast_ref::<gtk::Expander>() {
            return Some(expander.clone());
        }
        let mut child = widget.first_child();
        while let Some(node) = child {
            if let Some(found) = first_expander(&node) {
                return Some(found);
            }
            child = node.next_sibling();
        }
        None
    }
    let details = first_expander(view.list.upcast_ref()).unwrap();
    assert!(!details.is_expanded());
    details.set_expanded(true);
    wait(&view);
    assert!(text(view.list.upcast_ref()).contains("Step 1:"));
    capture(&window, "analyzer-plan-details");
    details.set_expanded(false);

    // Select two identities explicitly through the real console dialog. A remote
    // item with a different name is intentionally used: names never choose pairs.
    until(|| owner.diagnostics().pending_events == Some(0));
    let scope = owner.detector().scope().clone();
    let opening = OpenInventoryRequest {
        local_root: root.clone(),
        expected: SyncIdentity {
            sync_id: scope.sync_id.clone(),
            account_id: scope.account_id.clone(),
            remote_root_id: scope.remote_root_id.clone(),
        },
        create_if_missing: false,
    };
    let inv = InventoryManager::open(opening.clone()).unwrap().inventory;
    let local = inv
        .get_item(GetItemRequest {
            selector: ItemSelector::LocalRelativePath(
                LocalRelativePath::new(b"New folder/Created after opening.txt".to_vec()).unwrap(),
            ),
        })
        .unwrap()
        .item
        .unwrap();
    let template = seed(&base.join("reference-metadata"), id, 1);
    let reference_inv = InventoryManager::open(template).unwrap().inventory;
    let file = reference_inv
        .list_items(ListItemsRequest {
            filter: Default::default(),
            pagination: Default::default(),
        })
        .unwrap()
        .page
        .entries
        .remove(0)
        .remote
        .unwrap();
    let reference = file.reference.clone();
    let name = match &file.name {
        MetadataValue::Known(name) => name.clone(),
        _ => unreachable!(),
    };
    let remote = inv
        .record_remote(RecordRemoteObservation {
            selector: ObservationTarget::NewItem,
            expected_revision: None,
            observation: drivesync_engine::inventory::RemoteObservation {
                observed: File {
                    id: FileId("user-choice-remote-observation".into()),
                    causal_order: None,
                    sync_id: scope.sync_id.clone(),
                    observed_at: Timestamp { unix_millis: 1 },
                    scope: FileScope::Remote(reference.clone()),
                    coverage: ObservationCoverage::Partial,
                    data: Some(FileData::Remote(file)),
                    event: FileEvent {
                        action: FileEventAction::StateChanged,
                        local: None,
                        local_presence: None,
                        remote: Some(drivesync_engine::models::RemoteObservation {
                            source: RemoteObservationSource::Changes,
                            removed: Some(false),
                            time: None,
                            change_type: None,
                            drive_id: None,
                        }),
                        uncertainty: None,
                        previous: None,
                        current: Some(FileLocation::Remote {
                            reference: reference.clone(),
                            parent_id: Some(RemoteFileId("root".into())),
                            name: Some(name),
                        }),
                    },
                },
                presence: PresenceState::Present,
                proof: ObservationProof {
                    source: ObservationSource::RemoteChange,
                    observed_at: Timestamp { unix_millis: 1 },
                    scan_or_generation: None,
                    complete_scope: false,
                },
            },
        })
        .unwrap()
        .item;
    assert_ne!(local.id, remote.id);
    fn find(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if widget.widget_name() == name {
            return Some(widget.clone());
        }
        let mut child = widget.first_child();
        while let Some(node) = child {
            if let Some(found) = find(&node, name) {
                return Some(found);
            }
            child = node.next_sibling();
        }
        None
    }
    let changed = Rc::new(Cell::new(false));
    let changed_flag = changed.clone();
    let dialog = super::super::association::present(
        window.upcast_ref(),
        context.clone(),
        analyzer::Scope {
            account_id: id.into(),
            label: "Personal · explicitly selected identities".into(),
            request: Ok(opening),
        },
        Rc::new(move || changed_flag.set(true)),
    );
    let local_choice = find(dialog.upcast_ref(), "association-local")
        .unwrap()
        .downcast::<gtk::DropDown>()
        .unwrap();
    let remote_choice = find(dialog.upcast_ref(), "association-remote")
        .unwrap()
        .downcast::<gtk::DropDown>()
        .unwrap();
    let confirm = find(dialog.upcast_ref(), "associate-confirm")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    until(|| remote_choice.model().is_some_and(|m| m.n_items() == 2));
    assert_eq!(local_choice.selected(), 0);
    assert_eq!(remote_choice.selected(), 0);
    assert!(!confirm.is_sensitive());
    let model = local_choice.model().unwrap();
    let selected = (0..model.n_items())
        .find(|n| {
            model
                .item(*n)
                .unwrap()
                .downcast::<gtk::StringObject>()
                .unwrap()
                .string()
                .contains(&local.id.0)
        })
        .unwrap();
    local_choice.set_selected(selected);
    assert!(!confirm.is_sensitive());
    remote_choice.set_selected(1);
    assert!(confirm.is_sensitive());
    wait(&view);
    if let Some(path) = std::env::var_os("DRIVESYNC_UI_PREVIEW_DIR") {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&dialog)).snapshot(
            &snapshot,
            dialog.width() as f64,
            dialog.height() as f64,
        );
        let node = snapshot.to_node().unwrap();
        dialog
            .renderer()
            .unwrap()
            .render_texture(&node, None)
            .save_to_png(PathBuf::from(path).join("analyzer-association.png"))
            .unwrap();
    }
    confirm.emit_clicked();
    until(|| changed.get());
    let linked = inv
        .get_item(GetItemRequest {
            selector: ItemSelector::Id(local.id.clone()),
        })
        .unwrap()
        .item
        .unwrap();
    assert_eq!(linked.remote_ref, Some(reference));
    assert!(linked.baseline.is_none());
    assert!(inv
        .get_association(GetAssociationRequest { item_id: local.id })
        .unwrap()
        .association
        .unwrap()
        .decision_id
        .starts_with("user-association-"));
    assert_eq!(
        std::fs::read(root.join("New folder/Created after opening.txt")).unwrap(),
        b"local user content"
    );
    dialog.close();

    gtk::glib::MainContext::default().block_on(crate::application::detectors::remove(
        &context,
        &AccountId(id.into()),
    ));
    assert_eq!(
        owner.preparation_status().state,
        drivesync_engine::orchestrator::PreparationState::Stopped
    );
    window.close();
}

/// Opt-in acceptance against a real account; no fake Provider and no Operator.
#[test]
#[ignore = "requires explicit live account, private GTK profile and Secret Service access"]
fn real_account_startup_observes_drive_and_local_file_in_analyzer() {
    use drivesync_engine::detectors::{DeliveryState, RemoteCoverage};
    use std::io::Write;
    let required = |name| std::env::var(name).unwrap_or_else(|_| panic!("missing {name}"));
    assert_eq!(required("DRIVESYNC_LIVE_OBSERVATION"), "1");
    assert!(required("XDG_DATA_HOME").starts_with("/tmp/drivesync-ui-test-real-"));
    let id = required("DRIVESYNC_LIVE_ACCOUNT");
    let root = PathBuf::from(required("DRIVESYNC_LIVE_ROOT"));
    let configuration = crate::services::configuration::Configuration {
        account_id: AccountId(id.clone()),
        local_uri: gtk::gio::File::for_path(&root).uri().into(),
        remote_id: required("DRIVESYNC_LIVE_REMOTE_ROOT"),
        remote_path: "Live acceptance root".into(),
    };
    adw::init().unwrap();
    gtk::Settings::default()
        .unwrap()
        .set_gtk_enable_animations(false);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    gtk::glib::MainContext::default().block_on(crate::services::configuration::seed_for_test(
        &configuration,
    ));
    let context = Context::default();
    let app = adw::Application::builder()
        .application_id("com.rtosta.DriveSync.LiveAcceptance")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let navigation = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&navigation));
    let window = adw::ApplicationWindow::builder()
        .application(&app)
        .default_width(1100)
        .default_height(900)
        .content(&toasts)
        .build();
    let config = drivesync_engine::EngineConfig {
        data_directory: PathBuf::from(required("DRIVESYNC_LIVE_ACCOUNTS")),
        application_id: required("DRIVESYNC_LIVE_APPLICATION"),
        google_oauth_file: std::env::var_os("DRIVESYNC_GOOGLE_OAUTH_FILE").map(PathBuf::from),
    };
    // Reject a multi-account profile before the restoration hook can start it.
    let engine = gtk::glib::MainContext::default()
        .block_on(drivesync_engine::Engine::open(config.clone()))
        .unwrap();
    let accounts = gtk::glib::MainContext::default()
        .block_on(engine.list_accounts(drivesync_engine::ListAccountsRequest { provider: None }))
        .unwrap()
        .accounts;
    assert_eq!(
        accounts.len(),
        1,
        "supply a private single-account metadata profile"
    );
    assert_eq!(accounts[0].id.0, id);
    *context.engine.borrow_mut() = Some(engine);
    navigation.add(&crate::ui::dashboard::restoration::build(
        config,
        &context,
        &navigation,
        &toasts,
    ));

    window.present();
    let until = |test: &dyn Fn() -> bool| {
        let main = gtk::glib::MainContext::default();
        let end = Instant::now() + Duration::from_secs(60);
        while !test() {
            assert!(Instant::now() < end, "live account acceptance timed out");
            while main.pending() {
                main.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    until(&|| {
        context.detectors.borrow().get(&id).is_some_and(|s| {
            s.orchestrator
                .detector_status()
                .is_ok_and(|s| s.delivery == DeliveryState::Enabled)
                && s.orchestrator.remote_diagnostics().is_some_and(|r| {
                    r.status.coverage == RemoteCoverage::Complete && r.status.last_error.is_none()
                })
        })
    });
    assert_eq!(
        context.detectors.borrow().len(),
        1,
        "only the selected test account may run"
    );
    let (page, view) = create(&context);
    let retained = view.clone();
    page.connect_map(move |_| retained.reload());
    window.set_content(Some(&page));
    until(&|| {
        !view.loading_accounts.get()
            && !view.state.borrow().loading
            && view.accounts.model().is_some()
    });
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let name = format!("drivesync-e2e-ui-{stamp}.txt");
    let file = root.join(&name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .unwrap()
        .write_all(b"real account GUI acceptance fixture\n")
        .unwrap();
    fn text(widget: &gtk::Widget) -> String {
        let mut result = widget
            .downcast_ref::<gtk::Label>()
            .map(|l| l.text().to_string())
            .unwrap_or_default();
        let mut child = widget.first_child();
        while let Some(node) = child {
            result.push_str(&text(&node));
            child = node.next_sibling();
        }
        result
    }
    until(&|| !view.state.borrow().loading && text(view.list.upcast_ref()).contains(&name));
    let rows = text(view.list.upcast_ref());
    assert!(rows.contains("Upload"));
    assert!(rows.contains("Download"));
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"real account GUI acceptance fixture\n"
    );
    capture(&window, "analyzer-real-account");
    let owner = context
        .detectors
        .borrow()
        .get(&id)
        .unwrap()
        .orchestrator
        .clone();
    assert!(owner.preparation_status().events_processed > 0);
    owner
        .stop_detector(drivesync_engine::detectors::DetectorStopMode::StopNow)
        .unwrap();
    window.close();
}
