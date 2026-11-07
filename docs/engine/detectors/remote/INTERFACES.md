# Detector Remoto — INTERFACES

## 1. Visibilidade e tipos

Contrato **interno** consumido pelo `Detector`, não reexportado ao Orquestrador. O RemoteDetector pode ter estruturas/algoritmos diferentes do Local. Não depende de uma implementação real do pai nem de `storage/`; recebe portas tipadas injetadas.

```rust
pub(super) type RemoteResult<T> = Result<T, RemoteFailure>;
pub(super) enum RemoteState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub(super) enum RemotePollMode { Initial, Incremental, Recovery }
pub(super) enum RemoteCoverage { Complete, Partial, Unavailable }
pub(super) enum RemoteError {
    InvalidScope, InvalidState, RemoteUnavailable, PermissionDenied,
    CursorExpired, PageIncomplete, SinkUnavailable, CheckpointUnavailable,
    Cancelled, Internal,
}
pub(super) struct RemoteFailure {
    pub kind: RemoteError,
    pub retryable: bool,
    pub message: String,
}
pub(super) struct RemoteObservation {
    pub sync_id: SyncId,
    pub remote_id: Option<String>,
    pub kind: RemoteObservationKind,
    pub observed_at: Timestamp,
    pub coverage: RemoteCoverage,
    pub scope_hint: Option<RemoteScopeHint>,
}
pub(super) enum RemoteObservationKind {
    Changed, RecheckRequired, Unavailable, CursorInvalid,
}
pub(super) trait RemoteEventSink: Send + Sync {
    // Sucesso significa evento aceito de forma DURÁVEL pelo pai,
    // não que já foi consumido pelo Orquestrador.
    fn emit(&self, event: RemoteObservation) -> RemoteResult<()>;
}
pub(super) trait RemoteCheckpoint: Send + Sync {
    fn read(&self, sync_id: SyncId) -> RemoteResult<Option<String>>;
    fn commit(&self, sync_id: SyncId, checkpoint: String) -> RemoteResult<()>;
}
```

A localização do checkpoint pertence a seu backend, não ao Detector Remoto. A garantia **não avançar após emissão não persistida** exige testar os limites por página e replays idempotentes. O sink do pai transforma a observação em evento persistido sem exigir que o Remoto conheça `EventStore`.

## 2. Requests/results

```rust
pub(super) struct RemoteCreateRequest {
    pub sync_id: SyncId,
    pub remote_root_id: String,
    pub session: GoogleSession,
    pub google_api: GoogleApiPort,
    pub sink: RemoteEventSinkPort,
    pub checkpoints: RemoteCheckpointPort,
    pub polling_interval: Option<Duration>,
}
pub(super) struct RemoteCreateResult { pub detector: RemoteDetector, pub state: RemoteState }
pub(super) struct RemoteStartRequest { pub mode: RemotePollMode }
pub(super) struct RemoteStartResult { pub state: RemoteState, pub scheduled: bool }
pub(super) struct RemotePollRequest {
    pub mode: RemotePollMode,
    pub page_budget: Option<u32>,
}
pub(super) struct RemotePollResult {
    pub coverage: RemoteCoverage,
    pub emitted: u64,
    pub has_more: bool,
    pub checkpoint_confirmed: Option<String>,
}
pub(super) struct RemoteStopRequest {}
pub(super) struct RemoteStopResult { pub state: RemoteState }
pub(super) struct RemoteStatusRequest {}
pub(super) struct RemoteStatusResult {
    pub state: RemoteState,
    pub last_observed_at: Option<Timestamp>,
    pub last_error: Option<RemoteError>,
}
pub(super) struct GoogleApiPort { /* interface do google_api */ }
pub(super) struct RemoteEventSinkPort { /* RemoteEventSink */ }
pub(super) struct RemoteCheckpointPort { /* RemoteCheckpoint */ }
```

As declarações de portas acima são **esboços tipados**, não API compilada; ao implementar, escolher ownership/async apropriados sem expor concretudes ao pai. `GoogleSession` deve ser do contrato `google_api/`.

## 3. Métodos internos

```rust
impl RemoteDetector {
    pub(super) fn new(request: RemoteCreateRequest) -> RemoteResult<RemoteCreateResult>;
    pub(super) fn start(&self, request: RemoteStartRequest) -> RemoteResult<RemoteStartResult>;
    pub(super) fn poll_once(&self, request: RemotePollRequest) -> RemoteResult<RemotePollResult>;
    pub(super) fn stop(&self, request: RemoteStopRequest) -> RemoteResult<RemoteStopResult>;
    pub(super) fn status(&self, request: RemoteStatusRequest) -> RemoteResult<RemoteStatusResult>;
}
```

## 4. Contrato por método

### REM-01 — `new`

**Finalidade:** criar detector remoto isolado com raiz, sessão e portas.
**Parâmetros:** sync_id/remote_root_id e APIs/sink/checkpoint obrigatórios; intervalo opcional significa padrão interno.
**Retorno:** instância Stopped, sem HTTP.
**Limites e erros:** identidade inválida ou dependência incompatível falha; não inicia OAuth por conta própria.

### REM-02 — `start`

**Finalidade:** iniciar scheduler remoto e programar leitura segura.
**Parâmetros:** modo Initial, Incremental ou Recovery.
**Retorno:** estado e agendamento, não leitura já concluída.
**Limites e erros:** não presume push; checkpoint inválido degrada observação e requer recuperação.

### REM-03 — `poll_once`

**Finalidade:** ler páginas e **emitir um sinal por chamada de `RemoteEventSink.emit`**, confirmando checkpoint depois da aceitação durável.
**Parâmetros:** modo da leitura e page_budget opcional; ausência de limite não significa página única.
**Retorno:** cobertura, número emitido, continuação e checkpoint efetivamente confirmado (opcional).
**Limites e erros:** falha do sink não avança cursor; `removed` não confirma exclusão; `has_more` implica cobertura parcial da rodada.

### REM-04 — `stop`

**Finalidade:** interromper polling sem apagar checkpoint.
**Parâmetros:** request vazio, identidade da instância.
**Retorno:** estado parado.
**Limites e erros:** não descarta eventos já recebidos pelo pai nem tenta fazer reconciliação física.

### REM-05 — `status`

**Finalidade:** expor diagnóstico interno para o Detector pai.
**Parâmetros:** consulta vazia.
**Retorno:** estado, último instante e erro opcionais.
**Limites e erros:** não inicia HTTP; erro sanitizado sem token/corpo de resposta.

## 5. Semântica dos opcionais e pendências

- `remote_id=None`: escopo ou mudança não atribuída com segurança; evento continua válido.
- `page_budget=None`: política interna; `Some(n)` limita esforço sem declarar completude antes da última página.
- `polling_interval=None`: intervalo padrão escolhido pelo Remoto, não imposto ao Local.
- `checkpoint_confirmed=None`: nenhum avanço confirmado.
- `last_observed_at=None`: ainda não houve leitura válida.

**Pendências:** ownership e assincronicidade reais; localização da persistência do checkpoint; cobertura de árvores, papéis de `changes` vs `files.list` e deduplicação de replay. Resolver dentro do módulo sem alterar interface pública do Detector nem obrigar implementar Local/Storage.
