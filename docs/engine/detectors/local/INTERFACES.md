# Detector Local — INTERFACES

## 1. Contrato e limites

Esta API é **exclusiva do Detector Local**. Não há obrigação de construir ou iniciar o Detector Remoto para utilizá-la. O consumidor principal é o Orquestrador; o Detector Local usa somente a API pública do Inventário para persistir observações e publica notificações de reavaliação.

Métodos recebem um objeto de entrada e devolvem `LocalDetectorResult<ObjetoDeResultado>`. Os tipos abaixo são o contrato conceitual para TDD e precisam ser reconciliados com os tipos definitivos do Inventário e do Orquestrador antes da implementação; **não são código Rust compilado**.

## 2. Tipos públicos

```rust
pub type LocalDetectorResult<T> = Result<T, LocalDetectorFailure>;

pub enum LocalDetectorState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub enum LocalScanCoverage { Complete, Partial, Unavailable }
pub enum LocalStartMode { Initial, Resume }
pub enum LocalScanCause { Startup, WatchEvent, Recovery, Manual }
pub enum LocalScanScope { Full, Paths(Vec<std::path::PathBuf>) }
pub enum LocalStopReason { UserRequest, Shutdown, Unavailable }
pub enum LocalNoticeReason { ObservedChange, RecheckRequired, ScopeUnavailable, MonitoringInterrupted }

pub struct LocalDetectorScope {
    pub sync_id: SyncId,          // contrato do Inventário
    pub account_id: AccountId,    // contrato do Inventário
    pub local_root: std::path::PathBuf,
}
pub struct LocalDetectorSettings {
    pub debounce_ms: Option<u64>,
}
pub struct LocalDetectorNotice {
    pub sync_id: SyncId,
    pub reason: LocalNoticeReason,
    pub affected_item_ids: Vec<SyncItemId>,
    pub observed_at: Timestamp,
    pub coverage: LocalScanCoverage,
}
pub trait LocalNotificationSink: Send + Sync {
    fn notify(&self, notice: LocalDetectorNotice) -> LocalDetectorResult<()>;
}

pub enum LocalDetectorError {
    InvalidScope, InvalidState, RootUnavailable, PermissionDenied,
    PartialObservation, WatcherUnavailable, InventoryUnavailable,
    PreconditionFailed, Cancelled, NotificationFailed, Internal,
}
pub struct LocalDetectorFailure {
    pub kind: LocalDetectorError,
    pub message: String, // sanitizada
    pub retryable: bool,
}
pub struct LocalDetectorStatus {
    pub state: LocalDetectorState,
    pub last_observation_at: Option<Timestamp>,
    pub last_coverage: Option<LocalScanCoverage>,
    pub last_error: Option<LocalDetectorError>,
}
```

### Objetos de requisição e resposta

```rust
// InventoryHandle e NotificationHandle são abstrações tipadas injetadas
// das APIs existentes, não wrappers de SQL nem implementação do Orquestrador.
// A forma concreta de ownership será fixada ao integrar os módulos.
pub struct LocalInventoryHandle { /* API do InventoryManager */ }
pub struct LocalNotificationHandle { /* LocalNotificationSink */ }

pub struct LocalDetectorCreateRequest {
    pub scope: LocalDetectorScope,
    pub inventory: LocalInventoryHandle,
    pub notifications: LocalNotificationHandle,
    pub settings: LocalDetectorSettings,
}
pub struct LocalDetectorCreateResult {
    pub detector: LocalDetector,
    pub state: LocalDetectorState,
}
pub struct LocalStartRequest {
    pub mode: LocalStartMode,
    pub requested_at: Timestamp,
}
pub struct LocalStartResult {
    pub state: LocalDetectorState,
    pub scheduled_scan: bool,
}
pub struct LocalScanRequest {
    pub scope: LocalScanScope,
    pub cause: LocalScanCause,
    pub request_id: Option<String>,
}
pub struct LocalScanResult {
    pub coverage: LocalScanCoverage,
    pub observed_count: u64,
    pub affected_items: Vec<SyncItemId>,
    pub notification: Option<LocalDetectorNotice>,
}
pub struct LocalStopRequest {
    pub reason: LocalStopReason,
    pub requested_at: Timestamp,
}
pub struct LocalStopResult {
    pub state: LocalDetectorState,
    pub in_flight: u32,
}
pub struct LocalStatusRequest {}
pub struct LocalStatusResult {
    pub status: LocalDetectorStatus,
}
```

## 3. Atributos opcionais: decisão de projeto

| Atributo | Por que é opcional | Semântica se ausente |
| --- | --- | --- |
| `debounce_ms` | Agrupamento de eventos é política de implementação | Usar valor interno seguro, não valor arbitrário de contrato |
| `request_id` | Nem toda varredura precisa de correlação externa | Não omitir observações nem prometer deduplicação só por essa ausência |
| `LocalScanResult.notification` | Uma leitura pode não produzir aviso novo | Não significa que não existam divergências anteriores |
| `LocalDetectorStatus.last_observation_at` | Detector pode ainda não ter observado | `None` não significa “agora” |
| `LocalDetectorStatus.last_coverage` | Nenhuma leitura concluída | `None` não deve ser interpretado como Complete |
| `LocalDetectorStatus.last_error` | Pode não haver falha conhecida | `None` não comprova disponibilidade atual |
| `affected_item_ids` vazio | Escopo mudou mas identidades ainda não são conhecidas | Reavaliar escopo, nunca descartar automaticamente |

`LocalDetectorScope.local_root` **não é opcional** para o Local. `LocalScanScope::Paths` não pode escapar da raiz sincronizada. `LocalScanCoverage::Complete` é relativa ao escopo pedido, não necessariamente à raiz inteira. Campo ausente, evento isolado ou cobertura incompleta não prova `PresenceState::AbsentConfirmed`.

## 4. Assinaturas públicas

```rust
pub struct LocalDetector { /* estado interno privado */ }

impl LocalDetector {
    pub fn new(request: LocalDetectorCreateRequest)
        -> LocalDetectorResult<LocalDetectorCreateResult>;
    pub fn start(&self, request: LocalStartRequest)
        -> LocalDetectorResult<LocalStartResult>;
    pub fn scan_once(&self, request: LocalScanRequest)
        -> LocalDetectorResult<LocalScanResult>;
    pub fn stop(&self, request: LocalStopRequest)
        -> LocalDetectorResult<LocalStopResult>;
    pub fn status(&self, request: LocalStatusRequest)
        -> LocalDetectorResult<LocalStatusResult>;
}
```

## 5. Contrato funcional de cada método

### LOC-01 — `new`

**Assinatura:** `new(LocalDetectorCreateRequest) -> LocalDetectorResult<LocalDetectorCreateResult>`.

**Finalidade:** Construir detector para uma raiz local, Inventário e destinatário de avisos, sem iniciar watcher.

**Parâmetros:** `scope.sync_id`, `account_id`, `local_root` exigidos; `inventory` é uma referência tipada ao Inventário; `notifications` é sink injetado; `settings` contém política local.

**Retorno:** Retorna instância em `Stopped` e o escopo validado.

**Limites e erros:** Não cria inventário global nem examina arquivos. Raiz reservada, associação inválida e dependências ausentes são erros.

### LOC-02 — `start`

**Assinatura:** `start(LocalStartRequest) -> LocalDetectorResult<LocalStartResult>`.

**Finalidade:** Iniciar observação contínua, preparando watcher e reconciliação por varredura.

**Parâmetros:** `mode` distingue início normal de retomada; `requested_at` registra a solicitação. Estado e raiz já pertencem à instância.

**Retorno:** Estado `Starting` ou `Running`, com indicação de varredura necessária.

**Limites e erros:** Não garante ausência de perda de evento; falha de watcher deve sinalizar estado degradado ou falha, não sucesso falso.

### LOC-03 — `scan_once`

**Assinatura:** `scan_once(LocalScanRequest) -> LocalDetectorResult<LocalScanResult>`.

**Finalidade:** Observar escopo completo ou caminhos afetados, registrar fatos locais e sinalizar reavaliações.

**Parâmetros:** `scope` define `Full` ou `Paths`; `cause` explica origem; `request_id` é opcional para correlação, não autoriza ignorar mudanças.

**Retorno:** Cobertura `Complete`, `Partial` ou `Unavailable`, contagem e IDs conhecidos; aviso opcional.

**Limites e erros:** Não transfere arquivos nem avança baseline. `Partial` não prova ausência em partes não lidas.

### LOC-04 — `stop`

**Assinatura:** `stop(LocalStopRequest) -> LocalDetectorResult<LocalStopResult>`.

**Finalidade:** Encerrar watcher e varreduras de forma coordenada.

**Parâmetros:** `reason` informa causa; `requested_at` situa a ordem.

**Retorno:** Estado após parada e número de observações ainda pendentes, se aplicável.

**Limites e erros:** Não apaga Inventário, não exclui arquivos e não afirma que a sincronização terminou.

### LOC-05 — `status`

**Assinatura:** `status(LocalStatusRequest) -> LocalDetectorResult<LocalStatusResult>`.

**Finalidade:** Consultar estado atual e último diagnóstico do detector.

**Parâmetros:** Request vazio: a instância já determina o escopo.

**Retorno:** Estado, última observação/cobertura e último erro, todos opcionais quando não ocorreram.

**Limites e erros:** Não dispara varredura e não faz I/O remoto.

## 6. Notificações e persistência

O `LocalNotificationSink` representa a entrada do Orquestrador: `notify` é uma operação de entrega de aviso, não de sincronização ou confirmação de item. Avisos podem ser repetidos, agregados ou perdidos; uma nova varredura restaura a observação depois de falha/reinício.

Para persistir fatos, chamar **`InventoryManager.record_local`** e consultas públicas necessárias, respeitando precondições de identidade e revisão. Não escrever SQL, não modificar cursor remoto e não alterar baseline/atividade/fila. Uma falha de persistência impede afirmar que a observação foi salva; falha de notificação posterior precisa ser diagnosticável.

## 7. Erros e dependências em aberto

`RootUnavailable`, `PermissionDenied`, `WatcherUnavailable`, `PartialObservation` e `Cancelled` **não** são prova de exclusão. Erros precisam ser tipados e sanitizados. A escolha de biblioteca watcher, tratamento de eventos durante scanner, tipos definitivos de `LocalFile`, forma concreta dos handles, semântica sync/async do start e política de symlinks devem ser fechadas antes de implementar os casos dependentes, **sem depender do Detector Remoto**.

**Estado:** proposta de contrato de projeto, ainda não compilada, nem validada em execução.
