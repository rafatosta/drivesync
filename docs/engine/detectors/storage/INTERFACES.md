# Detector Storage — INTERFACES

## 1. Fronteira interna e tipos

API **interna ao pacote Detector**, consumível pelo pai e substituível por fake nos testes. Não reexportar `EventStore`, estruturas, SQLite ou detalhes de lease ao restante da engine. Os nomes abaixo representam contratos de projeto, ainda não compilados.

```rust
pub(super) type StoreResult<T> = Result<T, StoreFailure>;

pub(super) struct StoreIdentity { pub sync_id: SyncId }
pub(super) struct DetectionEventRecord {
    pub event_id: DetectionEventId,
    pub sync_id: SyncId,
    pub item_id: Option<SyncItemId>,
    pub kind: DetectionKind,
    pub observed_at: Timestamp,
    pub scope_hint: Option<ScopeHint>,
}
pub(super) enum DetectionKind { ChangeObserved, RecheckRequired, ScopeUnavailable }
pub(super) enum DeliveryDisposition { Processed, Discarded }
pub(super) enum StoreError {
    IdentityMismatch, Unavailable, Corrupt, SchemaUnsupported,
    Conflict, LeaseExpired, InvalidLease, InvalidRequest, CapacityExceeded, Internal,
}
pub(super) struct StoreFailure {
    pub kind: StoreError,
    pub retryable: bool,
    pub message: String,
}
pub(super) struct StoreStats {
    pub pending: u64,
    pub leased: u64,
    pub total_confirmed: u64,
}
```

Os tipos `SyncId` e `SyncItemId` vêm dos contratos proprietários da engine. `ScopeHint` é tipado e limitado à localização de reavaliação; não é URL secreta nem nome de arquivo obrigatório.

## 2. Requests e resultados

```rust
pub(super) struct StoreOpenRequest {
    pub identity: StoreIdentity,
    pub location: StoreLocation, // fornecida pela fábrica privada, não pelo Detector público
}
pub(super) struct StoreOpenResult { pub store: EventStoreHandle }
pub(super) struct EnqueueEventRequest { pub event: DetectionEventRecord }
pub(super) struct EnqueueEventResult { pub event_id: DetectionEventId, pub inserted: bool }
pub(super) struct ClaimBatchRequest {
    pub max_events: u32,        // > 0
    pub lease_duration: Duration,
}
pub(super) struct ClaimBatchResult {
    pub lease_id: Option<LeaseId>, // None se não há eventos
    pub events: Vec<DetectionEventRecord>,
    pub first_observed_at: Option<Timestamp>,
    pub last_observed_at: Option<Timestamp>,
}
pub(super) struct AcknowledgeRequest {
    pub lease_id: LeaseId,
    pub disposition: DeliveryDisposition,
}
pub(super) struct AcknowledgeResult { pub confirmed_count: u32 }
pub(super) struct ReleaseBatchRequest { pub lease_id: LeaseId }
pub(super) struct ReleaseBatchResult { pub released_count: u32 }
pub(super) struct StoreStatusRequest {}
pub(super) struct StoreStatusResult { pub stats: StoreStats }

pub(super) trait EventStore: Send + Sync {
    fn enqueue(&self, request: EnqueueEventRequest) -> StoreResult<EnqueueEventResult>;
    fn claim_batch(&self, request: ClaimBatchRequest) -> StoreResult<ClaimBatchResult>;
    fn acknowledge(&self, request: AcknowledgeRequest) -> StoreResult<AcknowledgeResult>;
    fn release_batch(&self, request: ReleaseBatchRequest) -> StoreResult<ReleaseBatchResult>;
    fn status(&self, request: StoreStatusRequest) -> StoreResult<StoreStatusResult>;
}
pub(super) fn open(request: StoreOpenRequest) -> StoreResult<StoreOpenResult>;
```

`StoreLocation`, `EventStoreHandle`, IDs e relógio de leases serão definidos com ownership concreto na implementação. **Não alterar a API externa do Detector por conveniência de armazenamento.**

## 3. Contrato funcional de cada método

### STO-01 — `open`

**Finalidade:** abrir armazenamento da sincronização, validando identidade e integridade.
**Parâmetros:** `identity.sync_id` obrigatório; localização interna não vazia e segura.
**Retorno:** handle encapsulado para a fila dessa sincronização.
**Limites e erros:** formato inválido/corrupção/identidade divergente falham; não substituir banco existente nem acessar Inventário.

### STO-02 — `enqueue`

**Finalidade:** gravar evento único e durável.
**Parâmetros:** `event_id`, `sync_id`, tipo e tempo obrigatórios; item e escopo são opcionais conforme conhecimento do produtor.
**Retorno:** ID e `inserted` (false somente para repetição comprovadamente idêntica pelo mesmo ID).
**Limites e erros:** IDs repetidos com conteúdo divergente falham; não coalescer alterações diferentes nem gerar operação de sincronização.

### STO-03 — `claim_batch`

**Finalidade:** reservar atomicamente até `max_events` eventos pendentes para entrega.
**Parâmetros:** limite positivo e prazo de lease válido; nenhum filtro de tecnologia Local/Remote.
**Retorno:** lote tipado em ordem determinística, `lease_id` opcional para fila vazia e limites temporais opcionais.
**Limites e erros:** sem lease para fila vazia; lease válido impede claim concorrente; não exclui eventos por leitura.

### STO-04 — `acknowledge`

**Finalidade:** confirmar que consumidor tratou ou descartou **explicitamente** o lote reservado.
**Parâmetros:** lease válido e disposição `Processed` ou `Discarded` obrigatórios.
**Retorno:** quantidade confirmada, com proteção de idempotência para repetição da mesma confirmação.
**Limites e erros:** lease errado/expirado não apaga registros; não interpretar confirmação como baseline sincronizado.

### STO-05 — `release_batch`

**Finalidade:** devolver lote reservado sem confirmação para consumo futuro.
**Parâmetros:** lease válido.
**Retorno:** quantidade liberada; chamada repetida não deve causar perda.
**Limites e erros:** não altera payload nem converte evento em descartado.

### STO-06 — `status`

**Finalidade:** consultar estado da fila sem reservar eventos.
**Parâmetros:** request vazio; identidade já pertence ao handle.
**Retorno:** contagens pendentes/reservadas/confirmadas.
**Limites e erros:** consulta sem efeitos colaterais de consumo; pode retornar erro de indisponibilidade.

## 4. Erros, estados e ausência

`item_id=None` significa identidade ainda desconhecida; não significa exclusão. `scope_hint=None` significa evento suficiente por outros atributos ou escopo global a reavaliar. `lease_id=None` só é válido quando nenhum evento foi reservado. `ClaimBatchResult.events=[]` é fila vazia, não prova de que nenhuma mudança ocorreu durante a aplicação parada.

**Regra de contrato:** a confirmação é feita por lote sob `LeaseId` e todos os estados são transacionais. Lease expirado deve tornar evento elegível novamente, sem perder sua ordem original. Recuperação física (SQLite, journal, WAL, índice, FKs) é privada.

## 5. Pendências concretas

Definir serialização mínima tipada do payload, ownership/traits concretos, política de retenção e lease expirado, mecanismo de tempo para testes, instalação da base quando pasta sincronizada não estiver acessível e proteção contra processos simultâneos. Resolver antes de implementar casos dependentes; não adicionar métodos públicos ao Orquestrador para contornar detalhes internos.
