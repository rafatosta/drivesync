# Detector Local — INTERFACES

## 1. Escopo e visibilidade

Contrato **interno de `detectors/local/`** consumido somente pelo Detector pai. O módulo Local implementa **observação física do filesystem** e produz **`LocalFile` definido por [Models](../../models/INTERFACES.md)**. Models não é uma dependência operacional: apenas structs/enums e invariantes comuns.

As assinaturas abaixo são **esboços contratuais de Rust**, não código compilado. O Local pode funcionar com sink fake, sem Detector pai real, Storage, Google API, Inventário ou `SyncItem`. A fachada não reexporta a implementação Local.

## 2. Tipos internos e modelo compartilhado

```rust
// Importar de models: NÃO redefinir estes tipos em detectors/local/.
use crate::models::{
    AccountId, SyncId, File, FileData, FileEvent, FileEventKind, FileLocation,
    FileScope, ObservationCoverage, LocalFile, LocalRelativePath, Timestamp,
};

pub(super) type LocalDetectorResult<T> = Result<T, LocalDetectorFailure>;

pub(super) enum LocalDetectorState {
    Stopped, Starting, Running, Degraded, Stopping, Failed,
}
pub(super) enum LocalScanCoverage { Complete, Partial, Unavailable }
pub(super) enum LocalStartMode { Initial, Resume }
pub(super) enum LocalScanCause { Startup, WatchEvent, Recovery, Manual }
pub(super) enum LocalScanScope {
    Full,
    Paths(Vec<LocalRelativePath>),
}
pub(super) enum LocalStopReason { UserRequest, Shutdown, Unavailable }
pub(super) enum LocalDetectorError {
    InvalidScope, InvalidState, RootUnavailable, PermissionDenied,
    PartialObservation, WatcherUnavailable, ModelInvalid,
    PreconditionFailed, Cancelled, NotificationFailed, Internal,
}
pub(super) struct LocalDetectorFailure {
    pub kind: LocalDetectorError,
    pub message: String, // sanitizada
    pub retryable: bool,
}
```

`AccountId`, `SyncId` e `Timestamp` serão importados de seus **proprietários efetivos**, eventualmente reexportados por Models. Sua localização em `crate::models` é ilustrativa até unificação dos tipos existentes. **Não criar definições novas para fazer o exemplo compilar.**

### Aviso individual

```rust
// A unidade interna é o File COMPLETO de Models, não um wrapper File.
// O Detector Local compõe o contexto; o coletor constrói LocalFile.
pub(super) trait LocalNotificationSink: Send + Sync {
    // Um aviso por chamada. Ok(()) = persistido de forma durável pelo pai.
    fn notify(&self, file: File) -> LocalDetectorResult<()>;
}
```

**Regras de encapsulamento:** o produtor emite **um File por chamada**. `File.data=Some(FileData::Local(local_file))` preserva integralmente o objeto entregue pelo coletor; `File.data=None` é válido para `Deleted` suspeito, `RecheckRequired` e `ScopeUnavailable` segundo os invariantes de Models. O Local constrói somente contexto do evento (`FileEventKind`, localizações anterior/atual, `FileScope`, horário e cobertura), sem inspecionar campos internos de `LocalFile`. Não fabrica identidade ou ação.

`Created/Modified/Renamed/Moved/MetadataChanged` requerem evidências confiáveis e metadados presentes. Para movimento/renomeação, preservar `previous/current` quando correlacionados com segurança; caso contrário, usar `RecheckRequired`. Watcher remove pode ser exclusão suspeita, nunca comando de apagar. Falhas em escopo são indisponibilidade, não exclusão.

### Porta estável de obtenção de LocalFile

O Detector conhece apenas **a solicitação de coleta e o resultado**, sem acessar a composição interna de `LocalFile`. Proposta de porta conceitual, independente da lista de campos do modelo:

```rust
pub(super) struct LocalMetadataRequest {
    pub relative_path: LocalRelativePath,
    // O contexto da raiz é provido na construção do coletor.
}
pub(super) struct LocalMetadataResult {
    pub file: Option<LocalFile>,
    pub coverage: LocalScanCoverage,
}
pub(super) trait LocalMetadataCollector: Send + Sync {
    fn collect(&self, request: LocalMetadataRequest)
        -> LocalDetectorResult<LocalMetadataResult>;
}
```

**A interface de coleta é estável mesmo que `LocalFile` ganhe ou perca campos.** `file=None` e cobertura/erros representam ausência de metadados confiáveis, **não exclusão comprovada**. Escolha de sincronicidade e ownership concretos fica para a etapa de código; não criar métodos adicionais por atributo.

O coletor implementa internamente as chamadas de sistema e **pode delegar a funções auxiliares** a aquisição de grupos mais caros ou específicos (ACLs, xattrs, flags, dados de symlink). O código do Detector Local não conhece nem enumera esses grupos e não constrói `LocalFile` por struct literal. Sua única função é solicitar a coleta e transportar **o objeto devolvido sem projetar/reduzir seus campos**.

**Separação:** a definição `LocalFile` pertence a `models/`; a coleta com I/O pertence ao adaptador local; watcher, varredura e avisos pertencem ao Detector Local. Nenhuma modificação de `LocalFile` deve exigir mudança na lógica do Detector. A porta está descrita conceitualmente, sem implementação nesta etapa.

## 3. Objetos internos de requisição e resultado

```rust
pub(super) struct LocalDetectorScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub local_root: std::path::PathBuf, // configuração operacional, não LocalFile
}
pub(super) struct LocalDetectorSettings {
    pub debounce_ms: Option<u64>,
}
pub(super) struct LocalNotificationHandle {
    /* porta tipada LocalNotificationSink, injetada pelo pai */
}
pub(super) struct LocalDetectorCreateRequest {
    pub scope: LocalDetectorScope,
    pub notifications: LocalNotificationHandle,
    pub settings: LocalDetectorSettings,
}
pub(super) struct LocalDetectorCreateResult {
    pub detector: LocalDetector,
    pub state: LocalDetectorState,
}
pub(super) struct LocalStartRequest {
    pub mode: LocalStartMode,
    pub requested_at: Timestamp,
}
pub(super) struct LocalStartResult {
    pub state: LocalDetectorState,
    pub scheduled_scan: bool,
}
pub(super) struct LocalScanRequest {
    pub scope: LocalScanScope,
    pub cause: LocalScanCause,
    pub request_id: Option<String>,
}
pub(super) struct LocalScanResult {
    pub coverage: LocalScanCoverage,
    pub observed_count: u64,
    pub emitted_count: u64,
}
pub(super) struct LocalStopRequest {
    pub reason: LocalStopReason,
    pub requested_at: Timestamp,
}
pub(super) struct LocalStopResult {
    pub state: LocalDetectorState,
    pub in_flight: u32,
}
pub(super) struct LocalStatusRequest {}
pub(super) struct LocalStatusResult {
    pub state: LocalDetectorState,
    pub last_observation_at: Option<Timestamp>,
    pub last_coverage: Option<LocalScanCoverage>,
    pub last_error: Option<LocalDetectorError>,
}
```

**Decisão:** `LocalScanResult` retorna **contagens e cobertura**, não uma segunda coleção de avisos, para evitar duas rotas concorrentes de entrega. Todos os avisos seguem **exclusivamente o sink**; o pai é responsável pela fila. `LocalFile` não é persistido pelo Local.

## 4. Assinaturas internas

```rust
pub(super) struct LocalDetector { /* estado interno privado */ }
impl LocalDetector {
    pub(super) fn new(request: LocalDetectorCreateRequest)
        -> LocalDetectorResult<LocalDetectorCreateResult>;
    pub(super) fn start(&self, request: LocalStartRequest)
        -> LocalDetectorResult<LocalStartResult>;
    pub(super) fn scan_once(&self, request: LocalScanRequest)
        -> LocalDetectorResult<LocalScanResult>;
    pub(super) fn stop(&self, request: LocalStopRequest)
        -> LocalDetectorResult<LocalStopResult>;
    pub(super) fn status(&self, request: LocalStatusRequest)
        -> LocalDetectorResult<LocalStatusResult>;
}
```

## 5. Contratos por método

### LOC-01 — `new`

**Finalidade:** configurar observador local isolado para `sync_id`, conta e raiz, com sink de saída.
**Parâmetros:** `scope` e `notifications` obrigatórios; `debounce_ms=None` usa política segura interna.
**Retorno:** instância `Stopped`, sem watcher ativo.
**Erros/limites:** rejeitar raiz inválida ou reservada; não consultar Inventário nem criar `LocalFile` fictício.

### LOC-02 — `start`

**Finalidade:** ativar watcher e varredura de recuperação conforme modo.
**Parâmetros:** `mode`, `requested_at` obrigatórios.
**Retorno:** estado e indicação de scan programado; não comprova varredura concluída.
**Erros/limites:** falha de watcher ou cobertura nunca significa exclusão; início repetido deve ter semântica estável e tipada.

### LOC-03 — `scan_once`

**Finalidade:** inspecionar `Full` ou `Paths` e emitir avisos de observação/reavaliação contendo `LocalFile` quando possível.
**Parâmetros:** caminhos relativos tipados, causa e correlação opcional.
**Retorno:** cobertura, quantidade observada e quantidade de avisos aceitos pelo sink.
**Erros/limites:** `Complete` exige inspeção integral do escopo; falha do sink não incrementa `emitted_count` como entregue e exige replay/recuperação; não atualiza baseline nem executa operações.

### LOC-04 — `stop`

**Finalidade:** interromper watcher, agendamento e leitura em andamento.
**Parâmetros:** motivo e instante.
**Retorno:** estado e trabalho em trânsito.
**Erros/limites:** parar não confirma eventos, não apaga filas e não altera modelo de arquivo.

### LOC-05 — `status`

**Finalidade:** obter diagnóstico interno do observador.
**Parâmetros:** request vazio.
**Retorno:** estado, última observação, cobertura e erro opcionais.
**Erros/limites:** consulta sem scan, I/O remoto ou acesso a banco.

## 6. Semântica de tipos e ausência

- `LocalScanScope::Paths` usa `LocalRelativePath`; nenhum caminho pode escapar da raiz. Lista vazia deve resultar em erro tipado ou sem trabalho **conforme decisão explícita antes do código**, nunca virar scan global acidental.
- `LocalFile.local_id=None` significa identidade local desconhecida; não usar inode como ID estável sem comprovação.
- `LocalFile.kind=MetadataValue::Known(LocalEntryKind::Unknown)` ou `NotCollected`/`Unavailable` não autoriza inferir tipo ou tamanho; hash derivado não é atributo nativo obrigatório.
- Nomes Unix inválidos em UTF-8 devem ser preservados segundo a representação definida em Models.
- `LocalScanCoverage::Complete` avalia **cobertura da leitura**, não `PresenceState::AbsentConfirmed` do Inventário.
- `debounce_ms`, `request_id`, últimos timestamps e erros opcionais não implicam comportamento não documentado quando ausentes.

## 7. Fronteiras e pendências

O Local **não importa `SyncItem` ou `SyncItemId`** nesta etapa. Não depende da implementação de Storage; o sink fake é suficiente para testes locais. O Detector pai persiste o `File` completo sem remontar metadados.

Pendências antes de implementar: ownership efetivo das identidades já existentes, representação concreta de `LocalRelativePath`, política de symlinks, semântica de cancelamento/concorrência, watcher/debounce e como transportar metadata observada no evento persistente sem perda de evidência. Os contratos de Detector pai e Storage devem aceitar e reentregar `File` integral.

**Estado:** contrato de projeto não compilado, sem testes executados.
