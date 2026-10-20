# API pública do DriveSyncEngine — contrato conceitual

Esta API interna é consumida **diretamente** pela interface GTK no mesmo processo, sem D-Bus ou daemon.

- `add_account()` → AuthorizationFlowId ou AccountId
- `list_accounts()` → AccountSummary[]
- `create_sync_root(account_id, local_path, remote_folder_id, policy)` → SyncRootId
- `start_sync(root_id)` → OperationId
- `pause_sync(root_id)` → Result
- `resume_sync(root_id)` → Result
- `get_status(root_id)` → SyncStatus
- `list_conflicts(root_id)` → ConflictSummary[]
- `resolve_conflict(conflict_id, choice)` → OperationId
- `subscribe_events(root_id)` → EngineEvent stream

A interface gráfica não importa módulos internos nem manipula diretamente SQLite, OAuth ou transferências. O `Coordinator` faz a orquestração via contratos.

**API externa é outra fronteira:** o módulo `ipc` oferece leitura de estado e sinais via D-Bus a componentes opcionais fora do Flatpak; não exige daemon. Ver [integração Nautilus](nautilus-integration.md).

Os métodos descritos são planejados; a sincronização real ainda não está implementada.
