# Fachada pública do DriveSyncEngine (contrato conceitual)

Esta API é independente de Rust/GTK. Os nomes ilustram operações, não são métodos já implementados.

- add_account() → AuthorizationFlowId ou AccountId
- list_accounts() → AccountSummary[]
- create_sync_root(account_id, local_path, remote_folder_id, policy) → SyncRootId
- start_sync(sync_root_id) → OperationId
- pause_sync(sync_root_id) → Result
- resume_sync(sync_root_id) → Result
- get_status(sync_root_id) → SyncStatus
- list_conflicts(sync_root_id) → ConflictSummary[]
- resolve_conflict(conflict_id, choice) → OperationId
- subscribe_events(sync_root_id) → EngineEvent stream

A fachada valida entradas e encaminha operações ao Coordinator/serviços; não implementa OAuth, planner ou transferências. O frontend nunca instancia módulos internos diretamente. Uma GUI em outro processo usará um adaptador IPC/D-Bus com a mesma semântica.
