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

## API concreta da integração OAuth

- `add_account(&Cancellation, open_browser)` → `Result<AccountSummary, EngineError>`;
  chamada bloqueante em worker, com porta de navegador que retorna sucesso/falha.
- `list_accounts()` → perfis autenticados registrados neste processo.
- `verify_account(&AccountId)` → perfil confirmado via UserInfo, reutilizando ou
  renovando credencial persistida. Também deve rodar em worker.
- `AccountId` é uma chave textual `google:<sub>`, estável entre logins.
- `AccountSummary` contém ID, subject e e-mail verificado opcional; nenhum token.
- `EngineError::Authentication(AuthError)` contém somente erros tipados e textos
  sanitizados. A GUI não acessa os módulos privados.

`status()` continua `NotConfigured`: autenticação não configura uma raiz de
sincronização. As demais APIs conceituais acima continuam planejadas. A mudança
na fachada e o registro mínimo de accounts são necessários para a integração
Welcome → Account Information explicitamente solicitada.

## Restauração de login na abertura

`list_accounts()` lê perfis salvos sem verificar autenticação. `restore_accounts()`
retorna `Vec<RestoredAccount>` com perfil e `error: Option<AuthError>` por conta;
somente `None` confirma autenticação. Ambos são bloqueantes e exigem worker.
Falha global do índice retorna erro; `AccountStorage` identifica falhas do perfil
local, enquanto `SecureStorage` identifica falhas do cofre. Ver [ACCT-003](../engine/src/accounts/algorithms.md#acct-003--persistir-e-restaurar-perfis-na-abertura).
