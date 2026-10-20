# API pública DriveSyncEngine — contrato conceitual

A GUI usa chamadas Rust diretamente **no mesmo processo**. **Uma instância DriveSyncEngine gerencia N contas**, sem criar um motor, daemon ou processo por conta.

## Contas
- `add_account()` → AuthorizationFlowId / AccountId (fluxo OAuth)
- `list_accounts()` → AccountSummary[]
- `get_account_status(account_id)` → AccountSyncStatus
- `disconnect_account(account_id)` → resultado controlado (sem apagar arquivos)
- `pause_account(account_id)` / `resume_account(account_id)` → Result

## Raízes
- `create_sync_root(account_id, local_path, remote_folder_id, policy)` → SyncRootId
- `list_sync_roots(account_id)` → SyncRootSummary[]
- `start_sync(root_id)`, `pause_sync(root_id)`, `resume_sync(root_id)` → Result/OperationId
- `get_status(root_id)` → SyncStatus
- `get_overall_status()` → AggregatedSyncStatus (não ocultar contas com erro)
- `list_conflicts(root_id)` → ConflictSummary[]
- `resolve_conflict(conflict_id, choice)` → OperationId
- `subscribe_events(root_id?)` → EngineEvent stream, eventos sempre associados a AccountId + SyncRootId quando pertinentes

A fachada valida que cada raiz pertence à conta esperada. Uma sessão Auth não é compartilhada. A interface nunca acessa tokens, banco, fila ou adaptadores diretamente.

A **API D-Bus externa** do módulo `ipc` é uma fronteira opcional para a extensão Nautilus, não substitui a chamada direta GUI→Engine.

**Status:** interface conceitual, não implementada; métodos concretos dependem de revisão de contrato e testes.
