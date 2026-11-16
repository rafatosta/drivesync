//! Recovery contracts exercised with real, reopenable temporary SQLite databases.
use drivesync_engine::{inventory::*, models::*, operations::*};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "operator-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn open(&self) -> InventoryManager {
        InventoryManager::open(OpenInventoryRequest {
            local_root: self.0.clone(),
            expected: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            create_if_missing: true,
        })
        .unwrap()
        .inventory
    }
    fn intent(&self, key: &str) -> ExecutionIntent {
        let reference = RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("source".into()),
        };
        ExecutionIntent {
            context: ExecutionContext {
                id: ExecutionId::new(key.into()).unwrap(),
                source: Some(SourceEvidence::Remote {
                    metadata: Box::new(RemoteFile {
                        reference: reference.clone(),
                        provider: ProviderId("google".into()),
                        name: MetadataValue::Known("source".into()),
                        kind: MetadataValue::Known(RemoteEntryKind::File),
                        mime_type: MetadataValue::Known("application/octet-stream".into()),
                        parent_ids: MetadataValue::Known(vec![RemoteFileId("root".into())]),
                        trashed: MetadataValue::Known(false),
                        version: MetadataValue::Known("v1".into()),
                        created_time: MetadataValue::NotCollected,
                        modified_time: MetadataValue::NotCollected,
                        size: MetadataValue::Known(3),
                        md5_checksum: MetadataValue::NotCollected,
                        sha256_checksum: MetadataValue::NotCollected,
                        provider_data: MetadataValue::NotCollected,
                    }),
                    revision: None,
                }),
            },
            scope: ExecutionScope::from(&OperatorScope {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                local_root: self.0.clone(),
                remote_root_id: RemoteFileId("root".into()),
            }),
            command: OperationCommand::Download {
                source: ResourceRef::Remote {
                    provider_id: ProviderId("google".into()),
                    reference,
                },
                destination: DirectoryRef::LocalRoot,
                name: ItemName::Local("result.bin".into()),
            },
        }
    }
    fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.0.join(".drivesync/inventory.sqlite3")).unwrap()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn prepare(inv: &InventoryManager, intent: ExecutionIntent) -> ExecutionCheckpoint {
    inv.prepare_execution(PrepareExecutionRequest { intent })
        .unwrap()
}
fn save(
    inv: &InventoryManager,
    cp: &ExecutionCheckpoint,
    state: ExecutionState,
) -> InventoryResult<ExecutionCheckpoint> {
    inv.save_execution(SaveExecutionRequest {
        id: cp.intent.context.id.clone(),
        expected_revision: cp.revision,
        state,
    })
}
fn executing() -> ExecutionState {
    ExecutionState {
        phase: CheckpointPhase::Executing,
        effect: ExecutionEffect::Unknown,
        ..ExecutionState::prepared()
    }
}

#[test]
fn distinct_intentional_calls_are_not_deduplicated_by_command_or_filename() {
    let tree = Tree::new();
    let inv = tree.open();
    let first = prepare(&inv, tree.intent("first"));
    let second = prepare(&inv, tree.intent("second"));
    assert_eq!(first.intent.command, second.intent.command);
    assert_ne!(first.intent.context.id, second.intent.context.id);
    assert_eq!(
        tree.sql()
            .query_row("SELECT count(*) FROM execution_intents", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        tree.sql()
            .query_row("SELECT count(*) FROM operations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        tree.sql()
            .query_row("SELECT count(*) FROM baselines", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn reopen_and_exact_prepare_retry_recover_the_same_uncertain_execution() {
    let tree = Tree::new();
    let inv = tree.open();
    let intent = tree.intent("retry");
    let cp = prepare(&inv, intent.clone());
    let cp = save(&inv, &cp, executing()).unwrap();
    drop(inv);
    let reopened = tree.open();
    let recovered = prepare(&reopened, intent);
    assert_eq!(recovered, cp);
    assert_eq!(recovered.recovery_action(), RecoveryAction::ReconcileEffect);
    assert_eq!(
        reopened
            .get_execution(GetExecutionRequest {
                id: cp.intent.context.id.clone()
            })
            .unwrap(),
        recovered
    );
    assert!(!tree.0.join("result.bin").exists());
}
#[test]
fn an_execution_key_cannot_change_destination_source_version_or_scope() {
    let tree = Tree::new();
    let inv = tree.open();
    let intent = tree.intent("binding");
    prepare(&inv, intent.clone());
    let mut changed = intent.clone();
    let OperationCommand::Download { name, .. } = &mut changed.command else {
        unreachable!()
    };
    *name = ItemName::Local("other.bin".into());
    assert_eq!(
        inv.prepare_execution(PrepareExecutionRequest { intent: changed })
            .unwrap_err()
            .kind,
        InventoryError::AlreadyExists
    );
    let mut changed = intent.clone();
    let Some(SourceEvidence::Remote { metadata, .. }) = &mut changed.context.source else {
        unreachable!()
    };
    metadata.version = MetadataValue::Known("v2".into());
    assert_eq!(
        inv.prepare_execution(PrepareExecutionRequest { intent: changed })
            .unwrap_err()
            .kind,
        InventoryError::AlreadyExists
    );
    for field in 0..4 {
        let mut changed = intent.clone();
        match field {
            0 => changed.scope.sync_id.0 = "other".into(),
            1 => changed.scope.account_id.0 = "other".into(),
            2 => changed.scope.remote_root_id.0 = "other".into(),
            _ => changed.scope.local_root.push(b'x'),
        }
        assert_eq!(
            inv.prepare_execution(PrepareExecutionRequest { intent: changed })
                .unwrap_err()
                .kind,
            InventoryError::InvalidIdentity
        );
    }
    assert_eq!(prepare(&inv, intent).revision, 1);
}
#[test]
fn unknown_or_mismatched_source_evidence_never_prepares_an_execution() {
    let tree = Tree::new();
    let inv = tree.open();
    for fault in 0..6 {
        let mut intent = tree.intent("invalid");
        let Some(SourceEvidence::Remote { metadata, .. }) = &mut intent.context.source else {
            unreachable!()
        };
        match fault {
            0 => metadata.version = MetadataValue::NotCollected,
            1 => metadata.reference.file_id.0 = "different".into(),
            2 => metadata.provider.0 = "other".into(),
            3 => metadata.size = MetadataValue::Unavailable,
            4 => metadata.trashed = MetadataValue::Known(true),
            _ => intent.context.source = None,
        }
        assert_eq!(
            inv.prepare_execution(PrepareExecutionRequest { intent })
                .unwrap_err()
                .kind,
            InventoryError::InvalidArgument
        );
    }
    assert_eq!(
        tree.sql()
            .query_row("SELECT count(*) FROM execution_intents", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn stale_divergent_writers_fail_but_a_lost_save_response_is_idempotent() {
    let tree = Tree::new();
    let inv = tree.open();
    let other = tree.open();
    let cp = prepare(&inv, tree.intent("cas"));
    let next = save(&inv, &cp, executing()).unwrap();
    assert_eq!(save(&other, &cp, executing()).unwrap(), next);
    assert_eq!(
        save(&other, &cp, ExecutionState::prepared())
            .unwrap_err()
            .kind,
        InventoryError::VersionMismatch
    );
    assert_eq!(
        save(&other, &next, ExecutionState::prepared())
            .unwrap_err()
            .kind,
        InventoryError::InvalidTransition
    );
    let mut request = GetExecutionRequest {
        id: cp.intent.context.id,
    };
    request.id = ExecutionId::new("absent".into()).unwrap();
    assert_eq!(
        other.get_execution(request).unwrap_err().kind,
        InventoryError::NotFound
    );
}
#[test]
fn competing_connections_cannot_overwrite_each_others_recovery_facts() {
    let tree = Tree::new();
    let inv = tree.open();
    let other = tree.open();
    let cp = prepare(&inv, tree.intent("race"));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let first = cp.clone();
    let gate = barrier.clone();
    let writer = std::thread::spawn(move || {
        gate.wait();
        save(&inv, &first, executing())
    });
    barrier.wait();
    let ours = save(&other, &cp, ExecutionState::prepared());
    let theirs = writer.join().unwrap();
    assert_ne!(ours.is_ok(), theirs.is_ok());
    assert_eq!(
        ours.err().or_else(|| theirs.err()).unwrap().kind,
        InventoryError::VersionMismatch
    );
}
#[test]
fn completion_requires_observed_verification_and_keeps_the_published_identity() {
    let tree = Tree::new();
    let inv = tree.open();
    let cp = prepare(&inv, tree.intent("complete"));
    let cp = save(&inv, &cp, executing()).unwrap();
    let verifying = ExecutionState {
        phase: CheckpointPhase::Verifying,
        effect: ExecutionEffect::Applied,
        ..cp.state.clone()
    };
    let cp = save(&inv, &cp, verifying).unwrap();
    let item = ItemResult {
        resource: ResourceRef::Local {
            relative_path: LocalRelativePath::new(b"result.bin".to_vec()).unwrap(),
            local_id: Some(LocalFileId("actually-published".into())),
        },
        metadata: None,
        verification: VerificationStatus::Unverified,
    };
    let mut final_state = ExecutionState {
        phase: CheckpointPhase::Completed,
        result: Some(ExecutionResult::Transfer(TransferResult {
            item,
            bytes_transferred: 3,
            integrity: IntegrityStatus::NotAvailable,
        })),
        ..cp.state.clone()
    };
    assert_eq!(
        save(&inv, &cp, final_state.clone()).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
    let Some(ExecutionResult::Transfer(result)) = &mut final_state.result else {
        unreachable!()
    };
    result.item.verification = VerificationStatus::Verified;
    result.item.metadata = Some(FileData::Local(local_file(b"result.bin")));
    assert_eq!(
        save(&inv, &cp, final_state.clone()).unwrap_err().kind,
        InventoryError::InvalidIdentity
    );
    let Some(ExecutionResult::Transfer(result)) = &mut final_state.result else {
        unreachable!()
    };
    let Some(FileData::Local(file)) = &mut result.item.metadata else {
        unreachable!()
    };
    file.local_id = Some(LocalFileId("actually-published".into()));
    let done = save(&inv, &cp, final_state.clone()).unwrap();
    assert_eq!(done.recovery_action(), RecoveryAction::ReturnVerifiedResult);
    assert_eq!(save(&inv, &cp, final_state).unwrap(), done);
    assert_eq!(
        save(&inv, &done, done.state.clone()).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
    drop(inv);
    assert_eq!(prepare(&tree.open(), tree.intent("complete")), done);
}
#[test]
fn checkpoint_failure_rolls_back_the_intent_and_does_not_release_work() {
    let tree = Tree::new();
    let inv = tree.open();
    tree.sql().execute_batch("CREATE TRIGGER injected BEFORE INSERT ON execution_checkpoints BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(inv
        .prepare_execution(PrepareExecutionRequest {
            intent: tree.intent("rollback")
        })
        .is_err());
    assert_eq!(
        tree.sql()
            .query_row("SELECT count(*) FROM execution_intents", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    tree.sql().execute_batch("DROP TRIGGER injected").unwrap();
    assert_eq!(prepare(&inv, tree.intent("rollback")).revision, 1);
}
#[test]
fn corrupt_recovery_state_and_old_schema_are_preserved_and_rejected() {
    let tree = Tree::new();
    let inv = tree.open();
    let cp = prepare(&inv, tree.intent("corrupt"));
    tree.sql()
        .execute(
            "UPDATE execution_checkpoints SET confirmed_offset='not-a-number'",
            [],
        )
        .unwrap();
    assert_eq!(
        inv.get_execution(GetExecutionRequest {
            id: cp.intent.context.id.clone()
        })
        .unwrap_err()
        .kind,
        InventoryError::CorruptDatabase
    );
    drop(inv);
    tree.sql().pragma_update(None, "user_version", 6).unwrap();
    let path = tree.0.join(".drivesync/inventory.sqlite3");
    let before = std::fs::read(&path).unwrap();
    let result = InventoryManager::open(OpenInventoryRequest {
        local_root: tree.0.clone(),
        expected: SyncIdentity {
            sync_id: SyncId("sync".into()),
            account_id: AccountId("account".into()),
            remote_root_id: "root".into(),
        },
        create_if_missing: true,
    });
    assert!(matches!(
        result,
        Err(InventoryFailure {
            kind: InventoryError::UnsupportedSchema,
            ..
        })
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn journal_port_is_object_safe_and_sensitive_tokens_never_appear_in_debug() {
    let tree = Tree::new();
    let inv = tree.open();
    let journal: &dyn ExecutionJournal = &inv;
    let cp = journal
        .prepare_execution(PrepareExecutionRequest {
            intent: tree.intent("port"),
        })
        .unwrap();
    assert_eq!(
        journal
            .get_execution(GetExecutionRequest {
                id: cp.intent.context.id.clone()
            })
            .unwrap(),
        cp
    );
    let token = ResumeToken::new(b"sensitive-session-uri".to_vec()).unwrap();
    assert!(!format!("{token:?}").contains("sensitive-session"));
    assert!(ExecutionId::new("../escape".into()).is_err());
    assert!(serde_json::from_str::<ExecutionId>("\"\"").is_err());
    assert!(serde_json::from_str::<Sha256Digest>("\"md5\"").is_err());
    assert!(ResumeToken::new(vec![0; 8193]).is_err());
}

fn local_file(path: &[u8]) -> LocalFile {
    LocalFile {
        local_id: Some(LocalFileId("physical-source".into())),
        relative_path: LocalRelativePath::new(path.to_vec()).unwrap(),
        kind: MetadataValue::Known(LocalEntryKind::Regular),
        mode: MetadataValue::NotCollected,
        owner: LocalOwner {
            uid: MetadataValue::NotCollected,
            gid: MetadataValue::NotCollected,
        },
        times: LocalTimes {
            modified_at: MetadataValue::Known(LocalTimestamp::new(1, 2).unwrap()),
            changed_at: MetadataValue::Known(LocalTimestamp::new(3, 4).unwrap()),
            created_at: MetadataValue::NotCollected,
        },
        storage: LocalStorage {
            size_bytes: MetadataValue::Known(3),
            hard_link_count: MetadataValue::Known(1),
            inode: MetadataValue::Known(42),
            filesystem_device: MetadataValue::Known(DeviceNumber { major: 1, minor: 2 }),
        },
        flags: LocalFileFlags {
            statx_attributes: MetadataValue::NotCollected,
            statx_attributes_mask: MetadataValue::NotCollected,
        },
        symlink: LocalSymlink {
            target: MetadataValue::NotApplicable,
        },
    }
}
fn upload_intent(tree: &Tree, key: &str) -> ExecutionIntent {
    let mut intent = tree.intent(key);
    let file = local_file(b"source.bin");
    intent.command = OperationCommand::Upload {
        source: ResourceRef::Local {
            relative_path: file.relative_path.clone(),
            local_id: file.local_id.clone(),
        },
        destination: DirectoryRef::Remote {
            provider_id: ProviderId("google".into()),
            reference: RemoteFileRef {
                account_id: AccountId("account".into()),
                file_id: RemoteFileId("root".into()),
            },
        },
        name: ItemName::Remote("result.bin".into()),
    };
    intent.context.source = Some(SourceEvidence::Local {
        metadata: Box::new(file),
        sha256: Some(Sha256Digest::new("a".repeat(64)).unwrap()),
    });
    intent
}
#[test]
fn local_observation_is_bound_and_missing_physical_evidence_blocks_preparation() {
    let tree = Tree::new();
    let inv = tree.open();
    let intent = upload_intent(&tree, "local");
    prepare(&inv, intent.clone());
    let mut changed = intent.clone();
    let Some(SourceEvidence::Local { metadata, .. }) = &mut changed.context.source else {
        unreachable!()
    };
    metadata.times.modified_at = MetadataValue::Known(LocalTimestamp::new(50, 0).unwrap());
    assert_eq!(
        inv.prepare_execution(PrepareExecutionRequest { intent: changed })
            .unwrap_err()
            .kind,
        InventoryError::AlreadyExists
    );
    for fault in 0..5 {
        let mut changed = upload_intent(&tree, &format!("bad-local-{fault}"));
        let Some(SourceEvidence::Local { metadata, .. }) = &mut changed.context.source else {
            unreachable!()
        };
        match fault {
            0 => metadata.storage.inode = MetadataValue::NotCollected,
            1 => metadata.storage.hard_link_count = MetadataValue::Known(2),
            2 => metadata.times.changed_at = MetadataValue::Unavailable,
            3 => metadata.local_id = None,
            _ => metadata.relative_path = LocalRelativePath::new(b"other.bin".to_vec()).unwrap(),
        }
        assert_eq!(
            inv.prepare_execution(PrepareExecutionRequest { intent: changed })
                .unwrap_err()
                .kind,
            InventoryError::InvalidArgument
        );
    }
}
#[test]
fn upload_checkpoint_preserves_snapshot_reserved_id_session_and_server_offset() {
    let tree = Tree::new();
    let inv = tree.open();
    let cp = prepare(&inv, upload_intent(&tree, "upload"));
    assert_eq!(
        save(&inv, &cp, executing()).unwrap_err().kind,
        InventoryError::InvalidTransition
    );
    let reserved = ResourceRef::Remote {
        provider_id: ProviderId("google".into()),
        reference: RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("reserved-before-create".into()),
        },
    };
    let mut stage = local_file(b".drivesync/staging/upload.snapshot");
    stage.local_id = Some(LocalFileId("physical-snapshot".into()));
    let state = ExecutionState {
        staging: Some(StagingEvidence {
            metadata: Box::new(stage),
            sha256: Sha256Digest::new("a".repeat(64)).unwrap(),
        }),
        reserved_remote: Some(reserved),
        ..ExecutionState::prepared()
    };
    let cp = save(&inv, &cp, state).unwrap();
    let cp = save(
        &inv,
        &cp,
        ExecutionState {
            phase: CheckpointPhase::Executing,
            effect: ExecutionEffect::Unknown,
            ..cp.state.clone()
        },
    )
    .unwrap();
    let state = ExecutionState {
        resume_token: Some(ResumeToken::new(b"private-session".to_vec()).unwrap()),
        confirmed_offset: 2,
        ..cp.state.clone()
    };
    let cp = save(&inv, &cp, state).unwrap();
    drop(inv);
    let reopened = tree.open();
    let recovered = prepare(&reopened, upload_intent(&tree, "upload"));
    assert_eq!(recovered, cp);
    assert_eq!(
        recovered.state.resume_token.as_ref().unwrap().as_bytes(),
        b"private-session"
    );
    for fault in 0..5 {
        let mut bad = recovered.state.clone();
        match fault {
            0 => bad.confirmed_offset = 1,
            1 => bad.staging = None,
            2 => bad.reserved_remote = None,
            3 => bad.resume_token = Some(ResumeToken::new(b"other-session".to_vec()).unwrap()),
            _ => bad.confirmed_offset = 4,
        }
        assert!(save(&reopened, &recovered, bad).is_err());
    }
    assert!(!format!("{recovered:?}").contains("private-session"));
    assert_eq!(
        tree.sql()
            .query_row(
                "SELECT typeof(resume_token) FROM execution_checkpoints",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "blob"
    );
}
#[test]
fn raw_unix_root_bytes_roundtrip_without_lossy_serialization() {
    use std::os::unix::ffi::OsStringExt;
    let mut tree = Tree::new();
    let previous = tree.0.clone();
    let mut bytes = previous.clone().into_os_string().into_vec();
    bytes.extend_from_slice(b"-root-\xff");
    tree.0 = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    std::fs::rename(previous, &tree.0).unwrap();
    let inv = tree.open();
    let intent = tree.intent("unix");
    assert_eq!(
        serde_json::from_slice::<ExecutionIntent>(&serde_json::to_vec(&intent).unwrap()).unwrap(),
        intent
    );
    let cp = prepare(&inv, intent);
    drop(inv);
    assert_eq!(prepare(&tree.open(), tree.intent("unix")), cp);
}
#[test]
fn save_failure_preserves_the_previous_checkpoint_and_retry_uses_same_revision() {
    let tree = Tree::new();
    let inv = tree.open();
    let cp = prepare(&inv, tree.intent("save-rollback"));
    tree.sql().execute_batch("CREATE TRIGGER injected BEFORE UPDATE ON execution_checkpoints BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(save(&inv, &cp, executing()).is_err());
    assert_eq!(
        inv.get_execution(GetExecutionRequest {
            id: cp.intent.context.id.clone()
        })
        .unwrap(),
        cp
    );
    tree.sql().execute_batch("DROP TRIGGER injected").unwrap();
    assert_eq!(save(&inv, &cp, executing()).unwrap().revision, 2);
}
