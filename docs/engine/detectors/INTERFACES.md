# Detector — INTERFACES

## 1. Contrato público e visibilidade

**API disponível agora:** `drivesync_engine::detectors::LocalMetadataCollector`, com `new`, `scope` e `collect` da coleta local já implementada. Esta publicação parcial foi explicitamente solicitada; não implementa nem publica um Detector unificado provisório. Os exemplos de Detector/FileBatch abaixo continuam contrato futuro. A seção 8 descreve a superfície compilada.

`Detector` disponibiliza uma interface pública única para observar alterações, reservar eventos e confirmar sua entrega. As observações locais e remotas chegam individualmente e podem ser entregues em lotes mistos.

O código Rust apresentado é **contrato conceitual** sujeito a ajuste de ownership/assincronicidade antes da implementação, não API já compilada. Tipos de identidade devem ser reutilizados de seus módulos proprietários.

## 2. Tipos públicos

```rust
pub type DetectorResult<T> = Result<T, DetectorFailure>;

pub enum DetectorState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub enum DetectorStartMode { Initial, Restart }
pub enum DetectorStopMode { DrainInFlight, StopNow }
pub enum DeliveryState { Enabled, Paused }
pub enum DetectorActivity { Watching, Scanning, Polling, Recovering, Waiting, Unavailable, Inactive }
pub struct DetectorSourceStatus {
    pub state: DetectorState,
    pub activity: DetectorActivity,
    pub last_observation_at: Option<Timestamp>,
    pub last_error: Option<DetectorError>,
    pub observed_in_current_round: u64,
    pub emitted_in_current_round: u64,
}
pub enum DetectorScanReason { Manual, Recovery, ConsistencyCheck }
pub enum DetectorScanScope { All, Scopes(Vec<FileScope>) }
pub enum EventDisposition { Processed, Discarded }
pub enum DetectorError {
    InvalidState, InvalidScope, InvalidIdentity, SourceUnavailable,
    StoreUnavailable, Backpressure, DeliveryPaused, InvalidBatch, LeaseExpired,
    Cancelled, Internal,
}
pub struct DetectorFailure {
    pub kind: DetectorError,
    pub retryable: bool,
    pub message: String, // sanitizada
}
// File, FileScope, FileEvent, FileData, FileId e Timestamp vêm de models/.
// O lote e seu lease são responsabilidade exclusiva de detectors/.
pub struct FileBatch {
    pub batch_id: Option<BatchId>, // None somente quando files vazio
    pub files: Vec<File>,           // objetos COMPLETOS, sem projeção de campos
    pub first_observed_at: Option<Timestamp>,
    pub last_observed_at: Option<Timestamp>,
}
pub struct DetectorStatus {
    pub state: DetectorState,
    pub delivery: DeliveryState,
    pub local: DetectorSourceStatus,
    pub remote: DetectorSourceStatus,
    pub pending_events: u64,
    pub leased_events: u64,
    pub degraded_sources: u32,
    pub last_observation_at: Option<Timestamp>,
    pub last_error: Option<DetectorError>,
}
```

A fachada recebe e persiste **`models::File` integral**, incluindo `id`, `sync_id`, `event { kind, previous, current }`, `scope`, `coverage`, `observed_at` e `data: Option<FileData>`. Não mantém evento paralelo reduzido a IDs. A interface de entrega fornece `FileBatch.files` sem interpretar a origem da ocorrência. `File.data=None` é válido para remoção suspeita e reavaliação, **nunca exclusão confirmada**.

## 3. Requests e resultados públicos

```rust
pub struct DetectorCreateRequest {
    pub sync_id: SyncId,
    pub event_available_sink: Option<EventAvailableSinkHandle>,
    // Fontes e Storage são construídos por composição privada.
}
pub struct DetectorCreateResult { pub detector: Detector, pub state: DetectorState }
pub struct DetectorStartRequest {
    pub mode: DetectorStartMode,
    pub delivery: DeliveryState, // escolha inicial atômica, sem janela start/pause
}
pub struct DetectorStartResult { pub state: DetectorState }
pub struct DetectorPauseRequest {}
pub struct DetectorPauseResult { pub delivery: DeliveryState }
pub struct DetectorResumeRequest {}
pub struct DetectorResumeResult { pub delivery: DeliveryState }
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
pub struct TakeEventsResult { pub batch: FileBatch }
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

`event_available_sink=None`: eventos podem ser consultados periodicamente sem aviso de disponibilidade. Aviso de disponibilidade não confirma consumo. `max_events` e duração de lease devem ser positivos. `FileBatch.files=[]` implica `batch_id=None`, e instantes opcionais ausentes.

## 4. Assinaturas públicas

```rust
pub struct Detector { /* implementação privada */ }
impl Detector {
    pub fn new(request: DetectorCreateRequest) -> DetectorResult<DetectorCreateResult>;
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult>;
    pub fn pause(&self, request: DetectorPauseRequest) -> DetectorResult<DetectorPauseResult>;
    pub fn resume(&self, request: DetectorResumeRequest) -> DetectorResult<DetectorResumeResult>;
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

**Finalidade:** solicitar o início de **Local e Remoto juntos**, com estratégias internas independentes e entrega inicial definida atomicamente.
**Parâmetros:** início inicial ou após parada, e entrega `Enabled` ou `Paused`.
**Retorno:** estado agregado; não significa leitura concluída.
**Limites e erros:** ambos recebem solicitação de início mesmo quando um falha; estado agregado `Degraded`, com erros individuais preservados. Não existe comando público para iniciar apenas uma origem. Aceite de início não comprova observação concluída.

### DET-03 — `pause`

**Finalidade:** suspender **apenas a entrega externa**; Local e Remoto continuam observando e gravando eventos duravelmente no Storage.
**Parâmetros:** request vazio.
**Retorno:** entrega `Paused`.
**Limites e erros:** novos `take_events` devem retornar `DeliveryPaused` sem reservar eventos; lotes já reservados permanecem válidos para `acknowledge`, `release_events` ou expiração. Chamada repetida idempotente.

### DET-04 — `resume`

**Finalidade:** liberar novamente a entrega dos eventos persistidos.
**Parâmetros:** request vazio.
**Retorno:** entrega `Enabled`.
**Limites e erros:** não inicia fontes, não reconcilia o histórico e não compensa períodos de observação interrompida. Chamada repetida idempotente; requer serviço em estado operacional apropriado.

### DET-05 — `stop`

**Finalidade:** interromper efetivamente **as duas fontes** de observação e suspender a entrega, preservando os dados persistidos.
**Parâmetros:** `DrainInFlight` ou `StopNow`.
**Retorno:** estado e trabalhos ainda em andamento.
**Limites e erros:** `DrainInFlight` e `StopNow` diferem na finalização segura de observações em trânsito; retorno pode indicar `Stopping` até encerramento. Não remover eventos ou checkpoints persistidos, nem confirmar lotes. **O Orquestrador decide como lidar com mudanças ocorridas durante o intervalo sem detecção**; não delegar ao Detector Scanner, análise ou reconciliação automática.

### DET-06 — `request_scan`

**Finalidade:** pedir observação adicional, sem ordenar como Local/Remoto a executam.
**Parâmetros:** `All` ou `Scopes` com escopos `FileScope` válidos; motivo; ID de correlação opcional.
**Retorno:** aceitação/agendamento.
**Limites e erros:** sem upload/download; não analisar baseline ou conflito.

### DET-07 — `status`

**Finalidade:** consultar saúde agregada e fila.
**Parâmetros:** objeto vazio.
**Retorno:** estado agregado, permissão de entrega, estado/atividade/erro de cada origem e contadores correntes, mais estatísticas de fila.
**Limites e erros:** não reserva eventos.

### DET-08 — `take_events`

**Finalidade:** reservar **um lote unificado**, que pode conter eventos locais e remotos produzidos em paralelo.
**Parâmetros:** `max_events > 0` e lease válido.
**Retorno:** `FileBatch` com `File` completos, IDs estáveis, eventos individuais e intervalo do primeiro/último instante **observado**.
**Limites e erros:** quando entrega estiver `Paused`, retornar `DeliveryPaused` **sem fazer claim no Storage**; serializar pausa e reserva para que nenhuma nova reserva ocorra após pausa confirmada. Lotes anteriores seguem válidos. Leitura não apaga, não agrupa eventos conflitantes no mesmo ID e não resolve divergência. Ordem da fila representa **ordem durável de enfileiramento**, não causalidade entre as fontes. O mesmo item pode aparecer várias vezes no mesmo lote.

### DET-09 — `acknowledge`

**Finalidade:** confirmar lote como consumido ou explicitamente descartado.
**Parâmetros:** `batch_id` válido; `Processed` ou `Discarded`.
**Retorno:** quantidade confirmada.
**Limites e erros:** lote não confirmado permanece recuperável; confirmação não significa arquivo sincronizado.

### DET-10 — `release_events`

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

Os produtores entregam `models::File` por sinks internos, sem criar uma segunda representação de evento. O pai valida `sync_id`, persiste **o File integral** via Storage e somente então confirma o sink. O Storage devolve os mesmos objetos, não recriações por nome ou ID. Reentrega mantém `File.id` e conteúdo. O Detector não interpreta atributos de `LocalFile`/`RemoteFile`.

## 7. Garantias e pendências

**Produtor–consumidor:** eventos acumulam até serem reservados e confirmados; crash após claim permite redelivery por lease. Um lote pode conter simultaneamente sinais locais e remotos, sem alterar sua identidade ou conteúdo.

**Separação operacional:** `pause/resume` controlam entrega, não observação; `stop` encerra observação e suspende entrega; `start` inicia sempre ambas as fontes. A fila sobrevive à pausa e à parada, e apenas `acknowledge(Discarded)` pode descartar explicitamente um lote. O Orquestrador assume as consequências da escolha de interromper detecção. `DetectorState`, `DeliveryState` e estados de origem são independentes. Nenhuma observação prova exclusão sem evidência completa.

**Integração obrigatória com Models:** `FileBatch` pertence a Detector, `File`/`FileData`/`FileEvent`/`FileScope` a Models. `FileEventKind` possui Created, Modified, Deleted, Renamed, Moved, MetadataChanged, RecheckRequired, ScopeUnavailable. `File.event.previous/current` são localizações, não cópias de metadados. Nenhum `SyncItemId` é necessário para transportar uma ocorrência. Não acrescentar/remover campos no Detector quando `LocalFile`/`RemoteFile` evoluir: seus coletores/adaptadores especializados tratam isso. O evento representa uma observação, não uma ação executada.

**Decisões a fechar na implementação:** ownership Rust dos tipos comuns, fábrica interna das dependências, meios de sinalização, async e semântica de parada em trânsito, política de limites/backpressure e tipos temporais. Sem quebrar o isolamento dos submódulos.

**Estado:** documentação contratual, não código compilado ou testes executados.


## 8. API pública compilada — coleta local

O namespace `drivesync_engine::detectors` é público. O tipo `LocalMetadataCollector` é uma reexportação com nome público do coletor concreto existente; o caminho e os módulos de implementação permanecem privados. Não há wrapper que reconstrua LocalFile, duplicação de coleta ou necessidade de importar a trait interna.

### Tipos exportados

| Tipo público em detectors | Finalidade |
| --- | --- |
| LocalMetadataCollector | Coletor concreto, com campos privados, somente new/scope/collect públicos |
| LocalDetectorScope | Conta, SyncId e raiz absoluta usados na construção |
| LocalMetadataRequest | LocalRelativePath validado e token de cancelamento da chamada |
| LocalMetadataResult | LocalFile opcional integral e cobertura da coleta daquela entrada |
| LocalCancellation | new/default, clone, cancel e is_cancelled; cada new/default cria token independente |
| LocalScanCoverage | Complete/Partial/Unavailable; não comprova ausência nem percurso recursivo |
| LocalDetectorError, LocalDetectorFailure, LocalDetectorResult | Erros tipados e mensagem sanitizada; variantes compartilhadas não habilitam métodos ainda não implementados |
| LocalScanResult | Tipo nomeável de progress em LocalDetectorFailure; collect sempre retorna progress=None e não executa scan |

LocalFile, LocalRelativePath, AccountId e SyncId continuam em `drivesync_engine::models`; não são redefinidos ou copiados na fachada. Os nomes operacionais existentes foram preservados para não criar um segundo contrato de dados equivalente.

### `LocalMetadataCollector::new`

**Assinatura:** `pub fn new(scope: LocalDetectorScope) -> LocalDetectorResult<Self>`.

**Finalidade:** construir o coletor vinculado à raiz, sem iniciar watcher ou sincronização.

**Parâmetros:** conta/SyncId não vazios e raiz absoluta válida. Scope não valida associação consultando Accounts; configura a vinculação fornecida pelo consumidor.

**Retorno:** coletor utilizável externamente sem traits internas, preservando integralmente as regras físicas de [Local § 11](local/INTERFACES.md#11-adaptador-físico-de-coleta--etapa-de-implementação).

**Limites e erros:** construção é síncrona e abre descritores; executar fora da thread da GUI. InvalidScope, RootUnavailable, PermissionDenied ou ResourceLimit são sanitizados. Não cria raiz, arquivo, banco, sessão Google ou fila.

### `LocalMetadataCollector::scope`

**Assinatura:** `pub fn scope(&self) -> LocalDetectorScope`.

**Finalidade:** consultar a configuração original de conta, sincronização e raiz.

**Parâmetros:** nenhum.

**Retorno:** cópia do escopo, sem I/O, inclusive quando a raiz foi removida ou substituída.

**Limites e erros:** não revalida a raiz nem consulta Accounts; sem erro ou efeito externo. Alterar a cópia não altera o coletor.

### `LocalMetadataCollector::collect`

**Assinatura:** `pub fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult>`.

**Finalidade:** obter metadados selecionados de uma entrada abaixo da raiz, sem leitura de conteúdo ou acompanhamento contínuo.

**Parâmetros:** caminho de entrada já validado por Models e token criado/controlado pelo consumidor; cancel é irreversível e compartilhado pelos clones daquele token. O cancelamento não remove dados nem desfaz observações anteriores.

**Retorno:** mesmo objeto LocalFile produzido pelo adaptador e cobertura explícita; ausência é inconclusiva, não prova de exclusão. O token não é acionado pelo coletor. Chamadas independentes podem ocorrer concorrentemente.

**Limites e erros:** síncrono, sem tocar GUI. Mantém proteção de raiz/ancestrais, paths reservados, máscaras statx, fallback e cancelamento já testados no Local; o contrato físico não foi alterado pela publicação. Não emite File ou evento, não percorre filhos de diretório, não executa upload/download. Falhas seguem os erros tipados existentes, com progress=None.

### `LocalCancellation::new`, `default`, `cancel` e `is_cancelled`

**Assinaturas:** `pub fn new() -> Self`, `Default::default() -> Self`, `pub fn cancel(&self)` e `pub fn is_cancelled(&self) -> bool`.

**Finalidade:** permitir que o consumidor controle uma ou mais coletas cooperativas pelo mesmo token clonado.

**Parâmetros:** nenhum; cada operação usa o próprio token. Clone compartilha estado; new/default criam outro estado independente.

**Retorno:** new/default produzem token não cancelado, cancel retorna unidade, is_cancelled devolve o estado atual sem I/O.

**Limites e erros:** cancel é idempotente e não cancela syscall já iniciada de forma insegura. As consultas seguintes respeitam o sinal em pontos seguros. Nenhuma operação acessa filesystem ou tem erro; não há efeito global sobre outros tokens/coletores.

### Encapsulamento e limites desta publicação

Não são públicos `detectors::local`, seus módulos collector/filesystem/metadata/models, a trait interna de composição, MetadataReader, os descritores, hooks de corrida ou injeção de syscalls. O escopo externo escolhe somente raiz e requisições da coleta real; não injeta backend nem eventos.

Não existem Detector::new/start/pause/resume/stop/request_scan/status/take_events/acknowledge/release_events públicos nesta entrega. Isso evita expor métodos que apenas falham ou tipos remotos fictícios. Não há integração nova com a aplicação. A interface futura unificada continua sendo o contrato das seções anteriores, sem concessão de start somente local.
