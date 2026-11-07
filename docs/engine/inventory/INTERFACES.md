# Inventário — INTERFACES

## 1. Objetivo e princípio das interfaces

**Contrato Rust inicial para implementação orientada a testes (TDD).** O Inventário é proprietário do `SyncItem` e de seu banco por pasta sincronizada, descrito em [README.md](README.md). Todos os consumidores acessam os dados por **métodos públicos com objetos de entrada e objetos de resultado**; nenhum consumidor recebe `Connection`, `Transaction`, `Row`, SQL, nome de tabela ou estrutura de armazenamento.

**Regra principal:** quando três informações pertencem à mesma ação, elas formam um **objeto de requisição**; não são três argumentos independentes. Adicionar um campo opcional ao objeto não exige alterar a assinatura do método. Usar tipos semanticamente relacionados, e não um único objeto genérico com dezenas de propriedades desconexas.

```rust
// Evitar:
fn record_local(&self, item_id: SyncItemId, file: LocalFile, time: Timestamp, revision: u64);

// Contrato escolhido:
fn record_local(&self, request: RecordLocalObservation) -> InventoryResult<RecordObservationResult>;
```

Essa convenção é aplicada **inclusive a buscas**, filtros, cursores, eventos, fila e histórico. Métodos só recebem `&self` (ou `self` em métodos de construção) e **um objeto de requisição**. Consultas sem parâmetros usam um objeto vazio nomeado, como `GetSyncInfoRequest`. Respostas utilizam **objetos nomeados**, exceto erros retornados por `InventoryResult<T>`. Não retornar `bool` ou tuplas com significados ambíguos.

### Dependências externas e limites

- `GoogleFile` é importado do contrato público de `google_api/`; o Inventário não o redefine.
- `LocalFile` será importado do componente responsável pelos metadados locais. Seu contrato completo permanece **em aberto**; esta especificação depende apenas dele como **tipo** e de uma identidade local informada separadamente.
- As estruturas de consulta representam **modelos de domínio**. Persistir ou reconstruir um `GoogleFile` completo dependerá do mapeamento dos campos e relacionamentos documentados. Atributos não persistidos não podem ser fabricados ao reconstruir o objeto.
- O Inventário não conhece autenticação, GUI, regras de conflito ou operadores físicos.
- Esta API trata **um inventário por instância**, validado por `sync_id`. Não administra contas, conexões globais, GSettings ou bancos de outros módulos.

## 2. Tipos fundamentais

Os tipos abaixo são **propostas públicas Rust**. Os snippets expressam a forma e semântica do contrato; os imports definitivos dos tipos externos e detalhes de ownership serão fixados ao integrar os módulos.

```rust
pub type InventoryResult<T> = Result<T, InventoryFailure>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncItemId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalFileId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StoredGoogleFileId(pub String); // PK local, NÃO é o ID do provedor
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RemoteEventId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivityId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CorpusId(pub String);

pub struct Timestamp(pub String); // RFC 3339 validado na fronteira da API
pub struct Pagination {
    pub limit: u32,
    pub after: Option<String>, // cursor opaco emitido pela mesma consulta
}
pub struct Page<T> {
    pub entries: Vec<T>,
    pub next_cursor: Option<String>,
}
pub enum PresenceState { Unknown, Present, AbsentConfirmed }
pub enum SyncStatus {
    PendingReconciliation, PendingUpload, PendingDownload,
    Synced, Conflict, Error,
}
pub enum ObservationSource { LocalScan, LocalEvent, RemoteListing, RemoteChange }
pub struct ObservationProof {
    pub source: ObservationSource,
    pub observed_at: Timestamp,
    pub scan_or_generation: Option<String>,
    pub complete_scope: bool,
}
pub enum FieldUpdate<T> { Keep, Set(T), Clear }
```

**Distinções obrigatórias:** `Option<T>` significa valor não fornecido ou não conhecido, segundo o contexto; **não** é comando para apagar dados. Em comandos de atualização parcial, usar `FieldUpdate<T>` para diferenciar `Keep`, `Set` e `Clear`. Para dados do provedor, campos ausentes em uma resposta com `fields` parcial **não** autorizam limpeza.

`StoredGoogleFileId` é uma chave interna do SQLite. A identidade canônica do recurso remoto continua sendo o ID do Google (por exemplo `GoogleFile.id`). Não expor chaves locais de armazenamento como substitutas do ID do provedor.

## 3. Tipos de domínio entregues nas respostas

```rust
pub struct SyncItem {
    pub id: SyncItemId,
    pub local: Option<LocalFile>,
    pub remote: Option<GoogleFile>,
    pub baseline: Option<SyncBaseline>,
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub status: SyncStatus,
    pub conflict_reason: Option<String>,
    pub revision: u64,
    pub last_checked_at: Option<Timestamp>,
    pub last_synced_at: Option<Timestamp>,
}

pub struct SyncBaseline {
    pub local: BaselineSide,
    pub remote: BaselineSide,
    pub confirmed_at: Timestamp,
}
pub struct BaselineSide {
    pub presence: PresenceState,
    pub name: Option<String>,       // local pode exigir bytes: ver nota
    pub local_name_bytes: Option<Vec<u8>>,
    pub parent_identity: Option<String>,
    pub version: Option<String>,
    pub checksum: Option<String>,
    pub size: Option<u64>,
    pub modified_at: Option<Timestamp>,
}

pub struct SyncInfo {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub remote_root_id: String,
    pub generation: u64,
    pub initial_download_complete: bool,
}

pub enum OperationDirection { Upload, Download }
pub enum OperationKind { Create, UpdateContent, Rename, Move, Trash, Delete, Restore }
pub enum OperationState { Pending, Running, RetryWait, Blocked, Completed, Failed }

pub struct SyncOperation {
    pub id: OperationId,
    pub item_id: SyncItemId,
    pub direction: OperationDirection,
    pub kind: OperationKind,
    pub state: OperationState,
    pub base_item_revision: u64,
    pub expected_remote_version: Option<String>,
    pub attempt_count: u32,
    pub next_attempt_at: Option<Timestamp>,
    pub last_error_kind: Option<String>,
    pub created_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}
pub struct RemoteCursor {
    pub corpus: CorpusId,
    pub cursor: String,
    pub generation: u64,
}
pub struct RemoteEvent {
    pub id: RemoteEventId,
    pub corpus: CorpusId,
    pub item_id: Option<SyncItemId>,
    pub remote_id: Option<String>,
    pub event_kind: String,
    pub observed_version: Option<String>,
    pub observed_at: Timestamp,
    pub processed_at: Option<Timestamp>,
}

pub enum ActivityType {
    Upload, Download, Rename, Move, Create, Trash, Delete,
    Restore, ConflictDetected, ConflictResolved,
}
pub enum ActivityOrigin { Local, Remote, System }
pub enum ActivityOutcome { Completed, Failed, Cancelled, Blocked, Informational }
pub struct SyncActivity {
    pub id: ActivityId,
    pub item_id: SyncItemId, // obrigatório
    pub operation_id: Option<OperationId>,
    pub event_type: ActivityType,
    pub origin: ActivityOrigin,
    pub outcome: ActivityOutcome,
    pub occurred_at: Timestamp,
    pub description: Option<String>,
    pub error_kind: Option<String>,
    pub item_name: Option<String>,
    pub item_remote_id: Option<String>,
    pub item_local_path: Option<Vec<u8>>,
}
```

O nome local deve preservar bytes Unix quando necessário; `BaselineSide.local_name_bytes` é reservado ao lado local. O objeto `SyncActivity` conserva os dados relevantes no momento do evento, sem depender do nome atual do arquivo. **Atividade sem `item_id` não é permitida**; eventos globais de aplicação pertencem a outro componente.

`SyncItem.local` e `SyncItem.remote` não indicam sozinhos exclusão. A ausência comprovada é expressa por `PresenceState::AbsentConfirmed` com evidência apropriada.

## 4. Abertura, escopo e leitura do inventário

```rust
pub struct OpenInventoryRequest {
    pub local_root: std::path::PathBuf,
    pub expected: SyncIdentity,
    pub create_if_missing: bool,
}
pub struct SyncIdentity {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub remote_root_id: String,
}
pub struct OpenInventoryResult {
    pub inventory: InventoryManager,
    pub info: SyncInfo,
    pub created: bool,
}
pub struct GetSyncInfoRequest {}
pub struct GetSyncInfoResult { pub info: SyncInfo }
pub struct MarkInitialDownloadCompleteRequest {
    pub expected_generation: u64,
    pub expected_complete: bool,
}
pub struct MarkInitialDownloadCompleteResult { pub info: SyncInfo }

pub struct GetItemRequest { pub selector: ItemSelector }
pub enum ItemSelector {
    Id(SyncItemId),
    RemoteId(String),
    LocalId(LocalFileId),
    LocalRelativePath(std::path::PathBuf),
}
pub struct GetItemResult { pub item: Option<SyncItem> }

pub struct ListItemsRequest {
    pub filter: ItemFilter,
    pub pagination: Pagination,
}
pub struct ItemFilter {
    pub statuses: Vec<SyncStatus>,
    pub local_presence: Option<PresenceState>,
    pub remote_presence: Option<PresenceState>,
    pub remote_parent_id: Option<String>,
    pub local_parent_item_id: Option<SyncItemId>,
}
pub struct ListItemsResult { pub page: Page<SyncItem> }
```

`ItemFilter` agrupa condições relacionadas; futuras condições opcionais serão adicionadas **à estrutura**, não à assinatura do método. `ItemSelector` evita combinações inválidas como buscar por `item_id` e `remote_id` conflitantes na mesma chamada.

**Abertura segura:** se `create_if_missing=false` e não existe banco, retornar erro; se `true` e a pasta já contém arquivos do usuário sem inventário, **não inicializar silenciosamente**. `SyncIdentity` deve corresponder ao `sync_info` armazenado. Não recriar/resetar banco inválido automaticamente.

## 5. Observações, associação e baseline

```rust
pub struct RecordLocalObservation {
    pub selector: ObservationTarget,
    pub observation: LocalObservation,
    pub expected_revision: Option<u64>,
}
pub struct LocalObservation {
    pub file: Option<LocalFile>,
    pub presence: PresenceState,
    pub proof: ObservationProof,
    pub relative_path: Option<std::path::PathBuf>,
}
pub struct RecordRemoteObservation {
    pub selector: ObservationTarget,
    pub observation: RemoteObservation,
    pub expected_revision: Option<u64>,
}
pub struct RemoteObservation {
    pub file: Option<GoogleFile>,
    pub presence: PresenceState,
    pub proof: ObservationProof,
}
pub enum ObservationTarget {
    Existing(SyncItemId),
    ByRemoteId(String),
    ByLocalId(LocalFileId),
    NewItem, // somente após validação de unicidade
}
pub struct RecordObservationResult {
    pub item: SyncItem,
    pub created: bool,
}

pub struct LinkItemRequest {
    pub item_id: SyncItemId,
    pub changes: ItemLinksChange,
    pub expected_revision: u64,
}
pub struct ItemLinksChange {
    pub local_file_id: FieldUpdate<LocalFileId>,
    pub google_file_id: FieldUpdate<StoredGoogleFileId>,
}
pub struct LinkItemResult { pub item: SyncItem }

pub struct UpdateItemStatusRequest {
    pub item_id: SyncItemId,
    pub change: ItemStatusChange,
    pub expected_revision: u64,
}
pub struct ItemStatusChange {
    pub status: SyncStatus,
    pub conflict_reason: FieldUpdate<String>,
}
pub struct UpdateItemStatusResult { pub item: SyncItem }
```

- `RecordLocalObservation` e `RecordRemoteObservation` **nunca** avançam o baseline.
- `PresenceState::AbsentConfirmed` exige `ObservationProof` confiável e escopo observado de forma suficiente, sem apagar o último registro nem quebrar os vínculos.
- `Unknown` com `file: None` preserva atributos anteriores; `Some(file)` com `Present` atualiza somente os campos efetivamente observados.
- Não persistir `GoogleFile` em JSON opaco: mapear campos e suas relações. Os campos não observados preservam o valor antigo ou a ausência de conhecimento conforme contrato de observação.
- Associar arquivos já pertencentes a outro `SyncItem` retorna erro de unicidade; nunca reatribuir silenciosamente.

### Confirmação atômica

```rust
pub struct ConfirmSyncRequest {
    pub item_id: SyncItemId,
    pub expected_item_revision: u64,
    pub confirmed: ConfirmedState,
    pub completed_operation: Option<OperationId>,
    pub activity: Option<NewActivity>,
}
pub struct ConfirmedState {
    pub baseline: SyncBaseline,
    pub status: SyncStatus,
    pub verified_at: Timestamp,
}
pub struct ConfirmSyncResult {
    pub item: SyncItem,
    pub completed_operation: Option<SyncOperation>,
    pub activity: Option<SyncActivity>,
}
```

`ConfirmSyncRequest` deve aplicar **em uma única transação** os fatos confirmados, `sync_baselines`, estado da operação (se houver) e atividade correspondente (quando exigida). O baseline não é atualizado por tentativa de transferência, êxito parcial, retorno não verificado ou nova observação sem confirmação. Operação concluída com efeito relevante deve produzir atividade histórica; ausência de `activity` nesse caso retorna erro de validação, em vez de criar lacuna silenciosa.

## 6. Fila persistente de operações

```rust
pub struct EnqueueOperationRequest {
    pub item_id: SyncItemId,
    pub spec: NewSyncOperation,
    pub expected_item_revision: u64,
}
pub struct NewSyncOperation {
    pub direction: OperationDirection,
    pub kind: OperationKind,
    pub expected_remote_version: Option<String>,
    pub created_at: Timestamp,
    pub deduplication_key: Option<String>,
}
pub struct EnqueueOperationResult { pub operation: SyncOperation }

pub struct ListOperationsRequest {
    pub filter: OperationFilter,
    pub pagination: Pagination,
}
pub struct OperationFilter {
    pub item_id: Option<SyncItemId>,
    pub states: Vec<OperationState>,
    pub direction: Option<OperationDirection>,
    pub eligible_before: Option<Timestamp>,
}
pub struct ListOperationsResult { pub page: Page<SyncOperation> }

pub struct ClaimOperationRequest {
    pub operation_id: OperationId,
    pub expected_state: OperationState,
    pub claimed_at: Timestamp,
}
pub struct ClaimOperationResult { pub operation: SyncOperation }

pub struct UpdateOperationRequest {
    pub operation_id: OperationId,
    pub expected_state: OperationState,
    pub transition: OperationTransition,
}
pub enum OperationTransition {
    Retry { next_attempt_at: Timestamp, error_kind: String },
    Block { error_kind: String },
    Fail { failed_at: Timestamp, error_kind: String },
    Requeue { reason: String },
}
pub struct UpdateOperationResult { pub operation: SyncOperation }
```

As operações são **solicitações persistidas**, não autorização automática para executar. `ClaimOperation` deve ser atômico e impedir dois workers de reivindicarem a mesma operação. Não oferecer `Complete` em `UpdateOperation`: a conclusão verificável passa por `ConfirmSyncRequest` para manter fila, baseline e histórico consistentes. Ao reiniciar, estados `Running` com resultado desconhecido são reavaliados antes de reexecutar.

## 7. Cursores e eventos remotos

```rust
pub struct PublishRemoteChangesRequest {
    pub checkpoint: RemoteCheckpoint,
    pub events: Vec<NewRemoteEvent>,
}
pub struct RemoteCheckpoint {
    pub corpus: CorpusId,
    pub expected_cursor: Option<String>,
    pub new_cursor: String,
    pub expected_generation: u64,
}
pub struct NewRemoteEvent {
    pub remote_id: Option<String>,
    pub item_id: Option<SyncItemId>,
    pub event_kind: String,
    pub observed_version: Option<String>,
    pub observed_at: Timestamp,
    pub deduplication_key: Option<String>,
}
pub struct PublishRemoteChangesResult {
    pub cursor: RemoteCursor,
    pub inserted: u32,
    pub duplicate: u32,
}

pub struct GetRemoteCursorRequest { pub corpus: CorpusId }
pub struct GetRemoteCursorResult { pub cursor: Option<RemoteCursor> }
pub struct ListRemoteEventsRequest {
    pub filter: RemoteEventFilter,
    pub pagination: Pagination,
}
pub struct RemoteEventFilter {
    pub corpus: Option<CorpusId>,
    pub item_id: Option<SyncItemId>,
    pub pending_only: bool,
}
pub struct ListRemoteEventsResult { pub page: Page<RemoteEvent> }
pub struct AcknowledgeRemoteEventRequest {
    pub event_id: RemoteEventId,
    pub processed_at: Timestamp,
}
pub struct AcknowledgeRemoteEventResult { pub event: RemoteEvent }
```

`PublishRemoteChangesRequest` registra **cursor e eventos na mesma transação**. `expected_cursor` e `expected_generation` são precondições de concorrência. Avançar o cursor não reconhece eventos automaticamente. Eventos repetidos pela API devem ser idempotentes segundo a chave de deduplicação definida; um evento sem identificador de item pode gerar reavaliação posterior, sem inventar que o arquivo foi excluído.

`AcknowledgeRemoteEventRequest` só é emitida após tratamento explicitamente confirmado pelo consumidor; a API do Inventário não presume processamento ao entregar o evento.

## 8. Atividades — histórico vinculado a arquivo

```rust
pub struct NewActivity {
    pub item_id: SyncItemId,
    pub operation_id: Option<OperationId>,
    pub event_type: ActivityType,
    pub origin: ActivityOrigin,
    pub outcome: ActivityOutcome,
    pub occurred_at: Timestamp,
    pub description: Option<String>,
    pub error_kind: Option<String>,
    pub item_name: Option<String>,
    pub item_remote_id: Option<String>,
    pub item_local_path: Option<Vec<u8>>,
}
pub struct RecordActivityRequest { pub activity: NewActivity }
pub struct RecordActivityResult { pub activity: SyncActivity }

pub struct ListActivitiesRequest {
    pub filter: ActivityFilter,
    pub pagination: Pagination,
}
pub struct ActivityFilter {
    pub item_id: Option<SyncItemId>,
    pub operation_id: Option<OperationId>,
    pub event_types: Vec<ActivityType>,
    pub outcomes: Vec<ActivityOutcome>,
    pub from: Option<Timestamp>,
    pub until: Option<Timestamp>,
}
pub struct ListActivitiesResult { pub page: Page<SyncActivity> }
```

**`NewActivity.item_id` é sempre obrigatório**. `RecordActivityRequest` atende acontecimentos que não são conclusão de operação (como conflito por arquivo ou falha relevante). A conclusão de operação deve usar **`ConfirmSyncRequest`**, não duas chamadas separadas, evitando inconsistência entre baseline e histórico. Não guardar atividades gerais de app/suspensão global nesta tabela.

O Inventário deve permitir consultas globais *dentro daquela sincronização* com `ActivityFilter.item_id=None`; isso não significa atividades sem item. Para preservar histórico após remoção, manter `SyncItem` como tombstone enquanto existirem atividades que o referenciam.

## 9. Interface pública consolidada

Cada método recebe **um único objeto de requisição**, mesmo quando a operação é simples. Abertura consome `OpenInventoryRequest`. O restante opera sobre `InventoryManager`, sem SQL público.

```rust
pub struct InventoryManager { /* estado privado, conexão privada */ }

impl InventoryManager {
    pub fn open(request: OpenInventoryRequest) -> InventoryResult<OpenInventoryResult>;

    pub fn get_sync_info(&self, request: GetSyncInfoRequest)
        -> InventoryResult<GetSyncInfoResult>;
    pub fn mark_initial_download_complete(&self, request: MarkInitialDownloadCompleteRequest)
        -> InventoryResult<MarkInitialDownloadCompleteResult>;

    pub fn get_item(&self, request: GetItemRequest)
        -> InventoryResult<GetItemResult>;
    pub fn list_items(&self, request: ListItemsRequest)
        -> InventoryResult<ListItemsResult>;

    pub fn record_local(&self, request: RecordLocalObservation)
        -> InventoryResult<RecordObservationResult>;
    pub fn record_remote(&self, request: RecordRemoteObservation)
        -> InventoryResult<RecordObservationResult>;
    pub fn link_item(&self, request: LinkItemRequest)
        -> InventoryResult<LinkItemResult>;
    pub fn update_item_status(&self, request: UpdateItemStatusRequest)
        -> InventoryResult<UpdateItemStatusResult>;
    pub fn confirm_sync(&self, request: ConfirmSyncRequest)
        -> InventoryResult<ConfirmSyncResult>;

    pub fn enqueue_operation(&self, request: EnqueueOperationRequest)
        -> InventoryResult<EnqueueOperationResult>;
    pub fn list_operations(&self, request: ListOperationsRequest)
        -> InventoryResult<ListOperationsResult>;
    pub fn claim_operation(&self, request: ClaimOperationRequest)
        -> InventoryResult<ClaimOperationResult>;
    pub fn update_operation(&self, request: UpdateOperationRequest)
        -> InventoryResult<UpdateOperationResult>;

    pub fn publish_remote_changes(&self, request: PublishRemoteChangesRequest)
        -> InventoryResult<PublishRemoteChangesResult>;
    pub fn get_remote_cursor(&self, request: GetRemoteCursorRequest)
        -> InventoryResult<GetRemoteCursorResult>;
    pub fn list_remote_events(&self, request: ListRemoteEventsRequest)
        -> InventoryResult<ListRemoteEventsResult>;
    pub fn acknowledge_remote_event(&self, request: AcknowledgeRemoteEventRequest)
        -> InventoryResult<AcknowledgeRemoteEventResult>;

    pub fn record_activity(&self, request: RecordActivityRequest)
        -> InventoryResult<RecordActivityResult>;
    pub fn list_activities(&self, request: ListActivitiesRequest)
        -> InventoryResult<ListActivitiesResult>;
}
```

**20 operações públicas iniciais**: 2 de escopo, 2 de consulta, 5 de associação/estado/confirmação, 4 de fila, 4 de mudanças remotas, 2 de histórico, mais `open`. Novas operações poderão ser adicionadas sem modificar as atuais.

## 10. Erros públicos e comportamento

```rust
pub enum InventoryError {
    InvalidArgument,
    InvalidIdentity,
    NotFound,
    AlreadyExists,
    DuplicateAssociation,
    VersionMismatch,
    InvalidTransition,
    UnconfirmedAbsence,
    MissingObservation,
    MissingActivity,
    InvalidCursor,
    CorruptDatabase,
    UnsupportedSchema,
    PermissionDenied,
    StorageUnavailable,
    StorageFull,
    TransactionFailed,
    SerializationFailed,
}
pub struct InventoryFailure {
    pub kind: InventoryError,
    pub message: String, // sanitizada; sem dados sensíveis
    pub retryable: bool,
}
```

| Erro | Conduta |
| --- | --- |
| `InvalidArgument` | Rejeitar opções inválidas antes de escrever |
| `InvalidIdentity` | Bloquear uso de inventário de outra conta/pasta |
| `NotFound`, `AlreadyExists`, `DuplicateAssociation` | Não criar vínculo ambíguo nem sobrescrever registro |
| `VersionMismatch` | Precondição `revision`, cursor ou geração divergente; consumidor deve reconsultar |
| `InvalidTransition` | Não permitir conclusão, retry ou confirmação sem estado apropriado |
| `UnconfirmedAbsence`, `MissingObservation` | Não interpretar campo omitido/falha de observação como exclusão |
| `MissingActivity` | Rejeitar confirmação de ação que exige registro histórico |
| `InvalidCursor` | Rejeitar cursor de paginação inválido ou de filtro incompatível |
| `CorruptDatabase`, `UnsupportedSchema` | Não reconstruir/destruir automaticamente o banco |
| `PermissionDenied`, `StorageUnavailable`, `StorageFull`, `TransactionFailed` | Falha explícita; operações não ficam declaradas concluídas |
| `SerializationFailed` | Recusar valores inválidos, preservando estado anterior |

Erros do SQLite são traduzidos para tipos próprios; nenhuma mensagem pública expõe SQL, caminhos sensíveis, credenciais ou conteúdo privado.

## 11. Garantias de evolução e testes

1. **Assinatura estável:** novos atributos entram em `RecordLocalObservation`, `ItemFilter`, `NewSyncOperation` etc.; não acrescentar parâmetros posicionais.
2. **Objetos coesos:** estruturas de entrada descrevem uma operação; dados de observação são agrupados em `LocalObservation`/`RemoteObservation`; não usar `HashMap<String, Value>` como API genérica.
3. **Comportamento opcional explícito:** novas propriedades de entrada devem preservar o comportamento quando não fornecidas. Se a semântica mudar, atualizar o contrato e testar deliberadamente.
4. **Contrato separado da persistência:** alterar colunas, índices e tabelas não muda automaticamente `SyncItem` nem as assinaturas públicas.
5. **TDD:** testar primeiro casos mínimos, opcionais, erros, idempotência, rollback, paginação, FKs, revisão/concorrência, reinício e dados parciais.
6. **Banco real isolado:** testes de Inventário usam SQLite real em diretório temporário, com `LocalFile` e `GoogleFile` sintéticos, sem depender de Google nem de outros módulos funcionando.
7. **Segurança:** atividades vinculadas a item, nenhum dado pessoal/segredo em erro, ausência observada versus desconhecida sempre diferenciadas.
8. **Sem compatibilidade legada:** não adicionar adaptadores/migração para estados antigos por padrão; proteger arquivos do usuário e falhar com erro explícito quando esquema incompatível.

### Pontos ainda dependentes de outra especificação

- Campos definitivos de `LocalFile` e como reconstruir fielmente `LocalFile` de colunas.
- Projeção relacional e reconstrução de `GoogleFile` completo, inclusive campos não retornados pelo Drive.
- Semântica exata de `ObservationProof.complete_scope`, chaves de deduplicação, paginação e política de retenção.
- Tipo concreto de tempo e de bytes locais no projeto.
- Definição de operações simultâneas sobre um mesmo item e estratégias de lock. Estes detalhes devem ser fechados antes de implementar os respectivos métodos, **sem mudar a orientação a objetos da API**.

**Importante:** este documento define as interfaces propostas e seus invariantes, não comprova compilação Rust nem implementação. O próximo passo é descrever `TESTS.md` e criar testes que falhem antes de implementar o módulo.
