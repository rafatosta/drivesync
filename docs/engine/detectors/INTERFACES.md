# Detectores — INTERFACES

## 1. Contrato público e limites

O componente reúne **Detector Local** e **Detector Remoto**, ambos com ciclo de vida controlado pelo Orquestrador e produção de notificações tipadas. O inventário dos arquivos, eventos e cursores pertence a `inventory/`; o acesso ao Google pertence a `google_api/`. Os detectores não executam sincronização física e não fazem análise de conflito.

A direção de notificação é **Detectores → Orquestrador**. O mecanismo de entrega é injetado por `DetectorNotificationSink`; sem acesso ao estado interno do Orquestrador. Os eventos são pistas para reavaliar, não comandos de upload/download.

## 2. Tipos públicos do componente

**Contrato conceitual Rust para TDD:** estas definições expressam as interfaces que deverão ser implementadas, não uma API já compilada. Os tipos de outros módulos permanecem sob responsabilidade de seus proprietários. Cada método próprio recebe um objeto de requisição e devolve um objeto de resultado.

```rust
pub type DetectorResult<T> = Result<T, DetectorFailure>;

pub struct DetectorScope {
    pub sync_id: SyncId,       // do Inventário
    pub account_id: AccountId, // do Inventário
    pub local_root: Option<std::path::PathBuf>,
    pub remote_root_id: Option<String>,
}
pub enum DetectorSide { Local, Remote }
pub enum DetectorState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub enum ScanCoverage { Complete, Partial, Unavailable }
pub enum StartMode { Full, ReconcileAfterRestart }
pub enum ScanCause { Startup, WatchEvent, Recovery, Manual }
pub enum LocalScanScope { Full, Paths(Vec<std::path::PathBuf>) }
pub enum RemoteScanMode { InitialInventory, Incremental, Recovery }
pub enum StopReason { UserRequest, Shutdown, Unavailable }
pub enum ChangeReason {
    ObservedChange, RecheckRequired, ScopeUnavailable,
    CursorExpired, MonitoringInterrupted,
}
pub struct DetectorNotice {
    pub sync_id: SyncId,
    pub side: DetectorSide,
    pub reason: ChangeReason,
    pub affected_item_ids: Vec<SyncItemId>,
    pub observed_at: Timestamp,
    pub coverage: ScanCoverage,
}
pub trait DetectorNotificationSink: Send + Sync {
    fn notify(&self, notice: DetectorNotice) -> DetectorResult<()>;
}

pub struct DetectorSettings {
    pub ignore_reserved_metadata: bool, // deve permanecer true para .drivesync/
    pub debounce_ms: Option<u64>,
    pub polling_interval_ms: Option<u64>,
}
pub struct DetectorStatus {
    pub state: DetectorState,
    pub last_observation_at: Option<Timestamp>,
    pub last_coverage: Option<ScanCoverage>,
    pub last_error: Option<DetectorError>,
}
pub enum DetectorError {
    InvalidScope, InvalidState, RootUnavailable, PermissionDenied,
    PartialObservation, WatcherUnavailable, CursorExpired,
    RemoteUnavailable, InventoryUnavailable, PreconditionFailed,
    Cancelled, NotificationFailed, Internal,
}
pub struct DetectorFailure {
    pub kind: DetectorError,
    pub message: String,    // sanitizada
    pub retryable: bool,
}
```

**Escopo:** `DetectorScope.local_root` é obrigatório somente no detector local; `remote_root_id` é obrigatório somente no remoto. A presença do outro campo não muda o lado nem autoriza varrer o outro ambiente. `DetectorSettings` não tem permissão para incluir `.drivesync/`.

**Cobertura:** `Complete` = escopo solicitado integralmente observado e validado; `Partial` = parte observada, sem inferência de ausência sobre lacunas; `Unavailable` = escopo não pôde ser observado. Isso não equivale a `PresenceState` de cada arquivo.

**Notificação:** `affected_item_ids` pode estar vazio quando a identidade ainda é desconhecida; significa reavaliar o escopo, não “nenhum arquivo mudou”. Não incluir nomes ou caminhos pessoais sem necessidade. Falha de entrega de aviso não desfaz registros já confirmados no Inventário, mas deve ser sinalizada para permitir recuperação.

### Entradas e saídas

```rust
pub struct LocalDetectorCreateRequest {
    pub scope: DetectorScope,
    pub inventory: InventoryHandle,
    pub notifications: NotificationHandle,
    pub settings: DetectorSettings,
}
pub struct LocalDetectorCreateResult { pub detector: LocalDetector, pub state: DetectorState }

pub struct RemoteDetectorCreateRequest {
    pub scope: DetectorScope,
    pub session: GoogleSession,
    pub google_api: GoogleApiHandle,
    pub inventory: InventoryHandle,
    pub notifications: NotificationHandle,
    pub settings: DetectorSettings,
}
pub struct RemoteDetectorCreateResult { pub detector: RemoteDetector, pub state: DetectorState }

// Handles são referências tipadas a contratos existentes, não detalhes de storage.
// A forma concreta (Arc<dyn Trait>, referência emprestada etc.) será definida
// na integração sem alterar a semântica destas interfaces.
pub struct InventoryHandle { /* API do InventoryManager */ }
pub struct GoogleApiHandle { /* API pública da Google API */ }
pub struct NotificationHandle { /* implementa DetectorNotificationSink */ }

pub struct DetectorStartRequest { pub startup: StartMode, pub requested_at: Timestamp }
pub struct DetectorStartResult { pub state: DetectorState, pub scheduled_scan: bool }
pub struct LocalScanRequest {
    pub scope: LocalScanScope,
    pub cause: ScanCause,
    pub request_id: Option<String>,
}
pub struct LocalScanResult {
    pub coverage: ScanCoverage,
    pub observed_count: u64,
    pub affected_items: Vec<SyncItemId>,
    pub notification: Option<DetectorNotice>,
}
pub struct RemotePollRequest {
    pub mode: RemoteScanMode,
    pub page_budget: Option<u32>,
    pub request_id: Option<String>,
}
pub struct RemotePollResult {
    pub coverage: ScanCoverage,
    pub observed_count: u64,
    pub events_recorded: u64,
    pub checkpoint: Option<String>, // cursor efetivamente persistido, se houver
    pub has_more: bool,
    pub notification: Option<DetectorNotice>,
}
pub struct DetectorStopRequest { pub reason: StopReason, pub requested_at: Timestamp }
pub struct DetectorStopResult { pub state: DetectorState, pub in_flight: u32 }
pub struct DetectorStatusRequest {}
pub struct DetectorStatusResult { pub status: DetectorStatus }
```

## 3. Obrigatoriedade e comportamento dos atributos

| Campo | Obrigatório? | Significado quando ausente |
| --- | --- | --- |
| `local_root` | Para Local | `None` é inválido para Local; permitido para Remoto |
| `remote_root_id` | Para Remoto | `None` é inválido para Remoto; permitido para Local |
| `debounce_ms` | Não | Utilizar valor interno padrão, sem prometer número não definido |
| `polling_interval_ms` | Não | Utilizar política interna segura, sem assumir push confiável |
| `request_id` | Não | Solicitação sem chave explícita de deduplicação; não assumir idempotência de efeito por isso |
| `page_budget` | Não | Política padrão de páginas/limite operacional; não sinaliza leitura completa se o serviço ainda tiver páginas |
| `notification` (resultado) | Não | Não houve notificação produzida para aquela chamada; não significa que não existam mudanças |
| `checkpoint` (remoto) | Não | Nenhum cursor novo foi confirmado; nunca inventar avanço |
| `last_observation_at`, `last_coverage`, `last_error` | Não | Ainda não houve leitura ou dado correspondente |
| `affected_item_ids` vazio | Vazio legítimo | Mudança de escopo não associada a item conhecido ou notificação geral |

**Fatos parciais:** a resposta da Google API pode omitir atributos; o detector não os limpa no Inventário. Uma observação local pode falhar em uma subpasta; `coverage=Partial` não se converte em ausência confirmada nessa subárvore. A política para symlinks e a definição final de `LocalFile` são dependências que exigem contrato específico.


## 4. Assinaturas públicas consolidadas

```rust
pub struct LocalDetector { /* privado */ }
pub struct RemoteDetector { /* privado */ }
impl LocalDetector {
    pub fn new(request: LocalDetectorCreateRequest) -> DetectorResult<LocalDetectorCreateResult>;
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult>;
    pub fn scan_once(&self, request: LocalScanRequest) -> DetectorResult<LocalScanResult>;
    pub fn stop(&self, request: DetectorStopRequest) -> DetectorResult<DetectorStopResult>;
    pub fn status(&self, request: DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>;
}
impl RemoteDetector {
    pub fn new(request: RemoteDetectorCreateRequest) -> DetectorResult<RemoteDetectorCreateResult>;
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult>;
    pub fn poll_once(&self, request: RemotePollRequest) -> DetectorResult<RemotePollResult>;
    pub fn stop(&self, request: DetectorStopRequest) -> DetectorResult<DetectorStopResult>;
    pub fn status(&self, request: DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>;
}
```

**Total: 10 operações públicas** (5 por detector). `DetectorNotificationSink.notify` é o contrato adicional de entrega, não uma operação de controle do detector. Operações potencialmente longas precisam preservar cancelamento, segurança e estados parciais; a escolha de runtime/async concreta será tomada na implementação.

## 5. Contrato funcional de cada método público

### DET-01 — `local_new`

**Assinatura:** `local_new(LocalDetectorCreateRequest) -> DetectorResult<LocalDetectorCreateResult>`.

**Finalidade:** Inicializa detector local para uma única sincronização, recebendo raiz, inventário e destino de notificações.

**Parâmetros:** `scope`: identidade e raiz; `inventory`: acesso por API pública; `notifications`: destinatário; `settings`: regras de varredura e exclusões internas. Não abre banco global.

**Retorno:** `detector`: instância criada; `state`: Stopped.

**Limites e erros:** Não inicia watcher nem varre automaticamente; identidade incorreta, raiz indevida ou dependência inválida resultam em erro.

### DET-02 — `local_start`

**Assinatura:** `local_start(DetectorStartRequest) -> DetectorResult<DetectorStartResult>`.

**Finalidade:** Inicia acompanhamento de filesystem e agenda varredura segura de inicialização ou retomada.

**Parâmetros:** `startup`: Full/ReconcileAfterRestart; `requested_at`: instante da solicitação, não prova de leitura.

**Retorno:** `state`: Starting/Running/Degraded; `scheduled_scan`: se há varredura necessária.

**Limites e erros:** Não presume monitoramento perfeito; falha em registrar watcher não autoriza ausência confirmada.

### DET-03 — `local_scan_once`

**Assinatura:** `local_scan_once(LocalScanRequest) -> DetectorResult<LocalScanResult>`.

**Finalidade:** Executa varredura explícita de uma raiz inteira ou de caminhos delimitados e registra observações pela API do Inventário.

**Parâmetros:** `scope`: Full/Paths; `cause`: Startup/WatchEvent/Recovery/Manual; `request_id`: deduplicação da solicitação, não do conteúdo.

**Retorno:** `coverage`: Complete/Partial/Unavailable; `observed_count`; `affected_items`; `notification`: indicação de reavaliação.

**Limites e erros:** Não faz upload, não altera baseline e não confirma exclusões após varredura parcial.

### DET-04 — `local_stop`

**Assinatura:** `local_stop(DetectorStopRequest) -> DetectorResult<DetectorStopResult>`.

**Finalidade:** Interrompe observação contínua com encerramento seguro de recursos.

**Parâmetros:** `reason`: UserRequest/Shutdown/Unavailable; `requested_at`: momento da ordem.

**Retorno:** `state`: Stopped ou erro explícito; `in_flight`: quantidade de leituras em encerramento.

**Limites e erros:** Não apaga inventário nem perde estado persistido. Não sinaliza sincronização concluída.

### DET-05 — `local_status`

**Assinatura:** `local_status(DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>`.

**Finalidade:** Consulta estado do acompanhamento local e última cobertura observada.

**Parâmetros:** Objeto vazio: o detector já possui seu escopo.

**Retorno:** `state`, `last_observation_at?`, `last_coverage?`, `last_error?`.

**Limites e erros:** Não desencadeia nova varredura nem valida automaticamente a disponibilidade atual do root.

### DET-06 — `remote_new`

**Assinatura:** `remote_new(RemoteDetectorCreateRequest) -> DetectorResult<RemoteDetectorCreateResult>`.

**Finalidade:** Prepara detector remoto com sessão Google, identidade, Inventário e notificações.

**Parâmetros:** `scope` inclui ID da raiz remota e sincronização; `session`: conta autorizada; `google_api`: contrato do provedor; `inventory` e `notifications`: APIs públicas.

**Retorno:** `detector`: instância; `state`: Stopped.

**Limites e erros:** Não inicia login, consulta remota ou escreve em banco diretamente.

### DET-07 — `remote_start`

**Assinatura:** `remote_start(DetectorStartRequest) -> DetectorResult<DetectorStartResult>`.

**Finalidade:** Inicia acompanhamento remoto com retomada de cursor ou inicialização de inventário.

**Parâmetros:** `startup`: modo de início; `requested_at`: instante. O cursor real vem do Inventário.

**Retorno:** `state` e necessidade de leitura inicial/incremental.

**Limites e erros:** Não promete push notification nem confirma estado remoto apenas por iniciar.

### DET-08 — `remote_poll_once`

**Assinatura:** `remote_poll_once(RemotePollRequest) -> DetectorResult<RemotePollResult>`.

**Finalidade:** Consulta mudanças ou árvore remota conforme modo solicitado, pagina até limite seguro e persiste eventos/cursor consistentemente.

**Parâmetros:** `mode`: InitialInventory/Incremental/Recovery; `page_budget?`: limite operacional opcional, que NÃO equivale a conclusão; `request_id`: rastreio; `scope` de leitura derivado do detector.

**Retorno:** `coverage`, `observed_count`, `events_recorded`, `checkpoint?`, `has_more`, `notification?`.

**Limites e erros:** `has_more=true` impede Complete; não reconhece eventos nem interpreta removed como exclusão comprovada.

### DET-09 — `remote_stop`

**Assinatura:** `remote_stop(DetectorStopRequest) -> DetectorResult<DetectorStopResult>`.

**Finalidade:** Encerra polling/watcher remoto sem modificar cursores válidos.

**Parâmetros:** `reason` e `requested_at`; idempotência do stop conforme estado.

**Retorno:** `state` e `in_flight`.

**Limites e erros:** Não revoga OAuth, não exclui cursor nem registra ausência remota.

### DET-10 — `remote_status`

**Assinatura:** `remote_status(DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>`.

**Finalidade:** Consulta status remoto, observação e diagnóstico de acesso.

**Parâmetros:** Request vazio; escopo na instância.

**Retorno:** `state`, `last_observation_at?`, `last_coverage?`, `last_error?`.

**Limites e erros:** Não realiza chamada Google nem reinicializa cursor.

## 6. Notificações e consistência

O destinatário de `DetectorNotificationSink` deve poder receber `DetectorNotice` repetida, agregada ou sem `SyncItemId` específico. Não depender de entrega exatamente uma vez; não usar callback como confirmação de baseline ou de evento remoto. Após reinício, revarrer pasta local e reconsultar eventos/cursor persistidos pelo Inventário.

O detector local usa `record_local` do Inventário e o remoto usa `record_remote`, `publish_remote_changes` e `get_remote_cursor`, sob suas precondições próprias. **Nenhuma API destes módulos é alterada** por este documento. Quando a identidade não puder ser inferida sem risco, notificar reavaliação de escopo e aguardar reconciliação segura; não criar vínculo ambíguo.

## 7. Erros e invariantes

- `RootUnavailable`, `WatcherUnavailable`, `RemoteUnavailable` e `CursorExpired` **não comprovam exclusão**.
- `InventoryUnavailable` impede declarar fatos como persistidos; `NotificationFailed` não autoriza refazer ações físicas (que não pertencem ao detector).
- `PartialObservation` significa cobertura insuficiente; é proibido declarar `ScanCoverage::Complete` após erro de paginação, cancelamento ou subtree ilegível.
- `InvalidScope` protege identidade da conta, raiz e sincronização; `InvalidState` protege transições start/stop inadequadas.
- `DetectorFailure.message` é sanitizada: não pode incluir tokens, nomes/caminhos de usuário desnecessários nem conteúdo dos arquivos.
- Uma observação só modifica o lado correspondente, **nunca baseline ou fila**, e eventos remotos devem ser reconhecidos somente por etapa posterior autorizada.

## 8. Dependências a fechar antes da implementação

A forma exata de `InventoryHandle`, `GoogleApiHandle` e `NotificationHandle`; contrato definitivo de `LocalFile`; comportamento de watcher Linux/Flatpak e a política de tratamento de symlinks; política de paginação/cobertura; significado preciso de `start` assíncrono/síncrono e encerramento/cancelamento. **Não definir números arbitrários nem criar nova API no Inventário para contornar pendências**. Essas questões devem ser resolvidas explicitamente antes de escrever testes dependentes delas.

## Documentação

[README.md](README.md) descreve as responsabilidades e cenários; [TESTS.md](TESTS.md) descreve a validação; [AGENTS.md](AGENTS.md) regula a implementação. Este documento é um contrato de projeto, não afirmação de código já compilado.
