# Detector — INTERFACES

## 1. Contrato público e visibilidade

`Detector` é a **única API exposta ao Orquestrador**. Local e Remoto emitem eventos **individuais** por portas internas; Storage persiste e reserva. O Detector entrega **lotes mistos de eventos locais/remotos** pela mesma interface, sem obrigar o Orquestrador a conhecer os filhos.

O código Rust apresentado é **contrato conceitual** sujeito a ajuste de ownership/assincronicidade antes da implementação, não API já compilada. Tipos de identidade devem ser reutilizados de seus módulos proprietários.

## 2. Tipos públicos

```rust
pub type DetectorResult<T> = Result<T, DetectorFailure>;

pub enum DetectorState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub enum DetectorStartMode { Initial, Restart }
pub enum DetectorStopMode { DrainInFlight, StopNow }
pub enum DetectorScanReason { Manual, Recovery, ConsistencyCheck }
pub enum DetectorScanScope { All, Items(Vec<SyncItemId>) }
pub enum DetectorEventKind { ChangeObserved, RecheckRequired, ScopeUnavailable }
pub enum DetectorCoverage { Complete, Partial, Unavailable }
pub enum EventDisposition { Processed, Discarded }
pub enum DetectorError {
    InvalidState, InvalidScope, InvalidIdentity, SourceUnavailable,
    StoreUnavailable, Backpressure, InvalidBatch, LeaseExpired,
    Cancelled, Internal,
}
pub struct DetectorFailure {
    pub kind: DetectorError,
    pub retryable: bool,
    pub message: String, // sanitizada
}
pub struct DetectorEvent {
    pub event_id: DetectionEventId,
    pub sync_id: SyncId,
    pub item_id: Option<SyncItemId>,
    pub kind: DetectorEventKind,
    pub observed_at: Timestamp,
    pub coverage: DetectorCoverage,
    pub scope_hint: Option<ScopeHint>,
}
pub struct EventBatch {
    pub batch_id: Option<BatchId>, // None somente com events vazio
    pub events: Vec<DetectorEvent>,
    pub first_observed_at: Option<Timestamp>,
    pub last_observed_at: Option<Timestamp>,
}
pub struct DetectorStatus {
    pub state: DetectorState,
    pub pending_events: u64,
    pub leased_events: u64,
    pub degraded_sources: u32,
    pub last_observation_at: Option<Timestamp>,
    pub last_error: Option<DetectorError>,
}
```

Eventos de fontes distintas conservam `event_id` e informação suficiente para reavaliação por item/escopo. O objeto público **não precisa expor qual mecanismo produziu** a alteração; se houver utilidade posterior em apresentar a origem, isso será uma decisão explícita e não exigirá duas APIs. `item_id=None` sinaliza reavaliação sem identidade estável.

## 3. Requests e resultados públicos

```rust
pub struct DetectorCreateRequest {
    pub sync_id: SyncId,
    pub event_available_sink: Option<EventAvailableSinkHandle>,
    // Fontes e Storage são construídos por composição privada.
}
pub struct DetectorCreateResult { pub detector: Detector, pub state: DetectorState }
pub struct DetectorStartRequest { pub mode: DetectorStartMode }
pub struct DetectorStartResult { pub state: DetectorState }
pub struct DetectorStopRequest { pub mode: DetectorStopMode }
pub struct DetectorStopResult { pub state: DetectorState, pub in_flight: u32 }
pub struct DetectorScanRequest {
    pub scope: DetectorScanScope,
    pub reason: DetectorScanReason,
    pub request_id: Option<String>,
}
pub struct DetectorScanResult { pub accepted: bool, pub correlation_id: Option<String> }
pub struct DetectorStatusRequest {}
pub struct DetectorStatusResult { pub status: DetectorStatus }
pub struct TakeEventsRequest { pub max_events: u32, pub lease_duration: Duration }
pub struct TakeEventsResult { pub batch: EventBatch }
pub struct AcknowledgeEventsRequest {
    pub batch_id: BatchId,
    pub disposition: EventDisposition,
}
pub struct AcknowledgeEventsResult { pub confirmed_count: u32 }
pub struct ReleaseEventsRequest { pub batch_id: BatchId }
pub struct ReleaseEventsResult { pub released_count: u32 }

pub trait EventAvailableSink: Send + Sync {
    fn notify_events_available(&self, sync_id: SyncId);
}
pub struct EventAvailableSinkHandle { /* porta tipada */ }
```

`event_available_sink=None`: Orquestrador pode consumir por consulta periódica sem receber avisos. Aviso de disponibilidade não confirma consumo. `max_events` e duração de lease devem ser positivos. `EventBatch.events=[]` implica `batch_id=None`, e instantes opcionais ausentes.

## 4. Assinaturas públicas

```rust
pub struct Detector { /* implementação privada */ }
impl Detector {
    pub fn new(request: DetectorCreateRequest) -> DetectorResult<DetectorCreateResult>;
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult>;
    pub fn stop(&self, request: DetectorStopRequest) -> DetectorResult<DetectorStopResult>;
    pub fn request_scan(&self, request: DetectorScanRequest) -> DetectorResult<DetectorScanResult>;
    pub fn status(&self, request: DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>;
    pub fn take_events(&self, request: TakeEventsRequest) -> DetectorResult<TakeEventsResult>;
    pub fn acknowledge(&self, request: AcknowledgeEventsRequest) -> DetectorResult<AcknowledgeEventsResult>;
    pub fn release_events(&self, request: ReleaseEventsRequest) -> DetectorResult<ReleaseEventsResult>;
}
```

## 5. Contrato funcional individual

### DET-01 — `new`

**Finalidade:** construir fachada de uma sincronização sem expor filhos.
**Parâmetros:** `sync_id` obrigatório; aviso de disponibilidade opcional.
**Retorno:** Detector Stopped.
**Limites e erros:** composição por fábrica privada; não exigir que consumidor crie Local/Remoto/Storage.

### DET-02 — `start`

**Finalidade:** iniciar os produtores com suas estratégias independentes.
**Parâmetros:** início inicial ou após parada.
**Retorno:** estado agregado; não significa leitura concluída.
**Limites e erros:** falha de uma origem pode resultar Degraded; não descarta pendências.

### DET-03 — `stop`

**Finalidade:** interromper produtores.
**Parâmetros:** `DrainInFlight` ou `StopNow`.
**Retorno:** estado e trabalhos ainda em andamento.
**Limites e erros:** nenhuma modalidade remove eventos já persistidos ou confirma consumo.

### DET-04 — `request_scan`

**Finalidade:** pedir observação adicional, sem ordenar como Local/Remoto a executam.
**Parâmetros:** `All` ou `Items` com IDs válidos; motivo; ID de correlação opcional.
**Retorno:** aceitação/agendamento.
**Limites e erros:** sem upload/download; não analisar baseline ou conflito.

### DET-05 — `status`

**Finalidade:** consultar saúde agregada e fila.
**Parâmetros:** objeto vazio.
**Retorno:** estado, quantidade pendente/reservada, erros opcionais.
**Limites e erros:** não reserva eventos.

### DET-06 — `take_events`

**Finalidade:** reservar **um lote unificado**, que pode conter eventos locais e remotos produzidos em paralelo.
**Parâmetros:** `max_events > 0` e lease válido.
**Retorno:** `EventBatch` com IDs estáveis, eventos individuais e intervalo do primeiro/último instante **observado**.
**Limites e erros:** leitura não apaga, não agrupa eventos conflitantes no mesmo ID e não resolve divergência. Ordem da fila representa **ordem durável de enfileiramento**, não causalidade entre as fontes. O mesmo item pode aparecer várias vezes no mesmo lote.

### DET-07 — `acknowledge`

**Finalidade:** confirmar lote como consumido ou explicitamente descartado pelo Orquestrador.
**Parâmetros:** `batch_id` válido; `Processed` ou `Discarded`.
**Retorno:** quantidade confirmada.
**Limites e erros:** lote não confirmado permanece recuperável; confirmação não significa arquivo sincronizado.

### DET-08 — `release_events`

**Finalidade:** devolver lote reservado para reentrega.
**Parâmetros:** `batch_id` válido.
**Retorno:** quantidade liberada.
**Limites e erros:** nenhuma perda ou confirmação implícita.

## 6. APIs internas consumidas

| Filho | Única dependência do Detector | O Detector não conhece |
| --- | --- | --- |
| [Local](local/INTERFACES.md) | Controle e sink interno de **um evento por emissão** | Watcher, varredura e tipos de filesystem |
| [Remoto](remote/INTERFACES.md) | Controle e sink interno de **um evento por emissão** | Polling, cursor e REST |
| [Storage](storage/INTERFACES.md) | `open`, `enqueue`, `claim_batch`, `acknowledge`, `release_batch`, `status` | Caminho, schema, SQL e SQLite |

Os adapters internos normalizam sinais para `DetectionEventRecord`. Sucesso de emissão só ocorre após `enqueue` durável. Os filhos **não precisam usar a mesma struct**, contanto que implementem seus contratos e a fachada converta sem perda de identidade/evidência.

## 7. Garantias e pendências

**Produtor–consumidor:** eventos acumulam até serem reservados e confirmados; crash após claim permite redelivery por lease. O Orquestrador pode receber simultaneamente sinais locais/remotos num mesmo lote e encaminhá-los à reconciliação, mas **não há resolução automática de conflitos no Detector**.

**Não há pausa pública**: `stop` interrompe produção, `start` retoma; descarte ocorre apenas em `acknowledge(Discarded)`. Fila sobrevive à parada. Nenhuma observação prova exclusão sem evidência completa.

**Decisões a fechar na implementação:** ownership Rust dos tipos comuns, fábrica interna das dependências, meios de sinalização, async e semântica de parada em trânsito, política de limites/backpressure e tipos temporais. Sem quebrar o isolamento dos submódulos.

**Estado:** documentação contratual, não código compilado ou testes executados.
