# Inventário — INTERFACES

## Integração revisada com Models — contrato-alvo

**Proprietário dos dados:** [Models](../models/INTERFACES.md) define `LocalFile`, `RemoteFile`, `RemoteFileRef`, `File`, `FileData`, `FileEvent`, `MetadataValue<T>` e IDs compartilhados. O Inventário define **somente** `SyncItem`, `SyncBaseline`, presença, status, fila de operações, atividades e acesso ao próprio banco. Nem `GoogleFile` de API, nem `FileBatch`/lease do Detector devem aparecer no contrato público do Inventário.

**Correspondência de identidades:** `SyncItemId` é item lógico persistente; `File.id` é ocorrência; `LocalFile.local_id` pode estar ausente; `RemoteFile.reference` é `RemoteFileRef(AccountId,RemoteFileId)` qualificado pela conta. Não usar caminho, nome, inode ou ID remoto não qualificado como identidade global. `SyncItem.revision` é controle otimista de escrita, não versão remota nem ID de evento.

**Forma das entradas:** `RecordLocalObservation.observation.observed` e `RecordRemoteObservation.observation.observed` recebem **`File` integral** com a variante correspondente de `FileData`; o Analisador fornece `PresenceState` e a prova do escopo. O Inventário não interpreta `FileEventKind` para gerar operações nem converter uma remoção suspeita em ausência confirmada. `data=None` mantém reavaliação e último snapshot conhecido. `Present` exige `Some(FileData)` da origem correta, e `AbsentConfirmed` exige prova independente suficiente.

**Preservação de metadados:** `MetadataValue::NotCollected`, `Unsupported`, `PermissionDenied` e `Unavailable` não equivalem a valores conhecidos nem apagam silenciosamente os últimos campos válidos. A seleção definitiva dos campos a persistir depende da revisão de utilidade; preservar `ProviderFileData` somente quando aprovado, filtrado e com codec seguro. `models/` permanece sem I/O.

**Separação de responsabilidades:** o [Detector Storage](../detectors/storage/INTERFACES.md) detém a fila de `File`/leases, e o [Detector Remoto](../detectors/remote/INTERFACES.md) detém a porta de checkpoint. A antiga seção de `PublishRemoteChanges`, `RemoteCursor` e `RemoteEvent` neste arquivo é uma **sobreposição de responsabilidades identificada**, ainda não eliminada do contrato. **Não implementar dois registros de entrega dos mesmos eventos**; decidir sua remoção/realocação antes de codificar, sem criar adaptadores legados.

**Estado:** revisão conceitual documentada. As estruturas e assinaturas ainda não representam Rust implementado; não foram executados testes.

## 1. Objetivo e princípio das interfaces

**Contrato Rust inicial para implementação orientada a testes (TDD).** O Inventário é proprietário do `SyncItem` e de seu banco por pasta sincronizada, descrito em [README.md](README.md). Todos os consumidores acessam os dados por **métodos públicos com objetos de entrada e objetos de resultado**; nenhum consumidor recebe `Connection`, `Transaction`, `Row`, SQL, nome de tabela ou estrutura de armazenamento.

**Regra principal:** quando três informações pertencem à mesma ação, elas formam um **objeto de requisição**; não são três argumentos independentes. Adicionar um campo opcional ao objeto não exige alterar a assinatura do método, **mas em Rust pode exigir alterações nos construtores com literais de struct**. Para oferecer evolução compatível aos consumidores, preferir construtores ou `Default`/builder para requisições com muitos campos opcionais e manter as invariantes de validação. Não prometer compatibilidade binária ou de código apenas por encapsular parâmetros. Usar tipos semanticamente relacionados, e não um único objeto genérico com dezenas de propriedades desconexas.

```rust
// Evitar:
fn record_local(&self, item_id: SyncItemId, observed: File, revision: u64);

// Contrato escolhido:
fn record_local(&self, request: RecordLocalObservation) -> InventoryResult<RecordObservationResult>;
```

Essa convenção é aplicada **inclusive a buscas**, filtros, cursores, eventos, fila e histórico. Métodos só recebem `&self` (ou `self` em métodos de construção) e **um objeto de requisição**. Consultas sem parâmetros usam um objeto vazio nomeado, como `GetSyncInfoRequest`. Respostas utilizam **objetos nomeados**, exceto erros retornados por `InventoryResult<T>`. Não retornar `bool` ou tuplas com significados ambíguos.

### Dependências externas e limites

- `RemoteFile`, `LocalFile`, `FileData`, `FileEvent`, `File` e identidades comuns são importados de `models/`; o Inventário não importa `providers/google/api/`.
- `LocalFile` será importado do componente responsável pelos metadados locais. O contrato conceitual de Models já define os grupos de metadados; a seleção final dos campos para persistência é pendente de revisão pré-implementação.
- As estruturas de consulta representam **modelos de domínio**. Persistir ou reconstruir um `RemoteFile` completo dependerá do mapeamento dos campos e relacionamentos documentados. Atributos não persistidos não podem ser fabricados ao reconstruir o objeto.
- O Inventário não conhece autenticação, GUI, regras de conflito ou operadores físicos.
- Esta API trata **um inventário por instância**, validado por `sync_id`. Não administra contas, conexões globais, GSettings ou bancos de outros módulos.

## 2. Tipos fundamentais

Os tipos abaixo são **propostas públicas Rust**; os identificadores compartilhados e modelos de arquivos são importados de Models. Os snippets expressam a forma e semântica do contrato; os imports definitivos dos tipos externos e detalhes de ownership serão fixados ao integrar os módulos.

```rust
pub type InventoryResult<T> = Result<T, InventoryFailure>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncItemId(pub String);
// SyncId, AccountId, LocalFileId, RemoteFileRef, LocalRelativePath, Timestamp,
// File, FileData, LocalFile e RemoteFile são importados do proprietário comum Models.
// NÃO redefinir seus identificadores no Inventário.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RemoteEventId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivityId(pub i64);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CorpusId(pub String);

pub struct Pagination {
    pub limit: u32,
    pub after: Option<String>, // cursor opaco emitido pela mesma consulta
}
pub struct Page<T> {
    pub entries: Vec<T>,
    pub next_cursor: Option<String>,
}
pub enum ItemKind { File, Directory, Symlink, Other }
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

**Distinções obrigatórias:** `Option<T>` significa valor não fornecido ou não conhecido, segundo o contexto; **não** é comando para apagar dados. Campos declarados sem `Option` são obrigatórios para construir a requisição/resultado, mas isso não implica que o valor represente uma observação comprovada; por exemplo, `PresenceState::Unknown` é um valor válido e explícito. Em comandos de atualização parcial, usar `FieldUpdate<T>` para diferenciar `Keep`, `Set` e `Clear`. Para dados do provedor, campos ausentes em uma resposta com `fields` parcial **não** autorizam limpeza.

`RemoteFileRef` (conta + ID remoto) é a identidade canônica de um recurso em qualquer provedor. Chaves físicas de banco são detalhes internos, nunca tipos públicos substitutos de `RemoteFileRef`. `File.id` identifica a ocorrência detectada, **não** o `SyncItem` nem o arquivo remoto.

## 3. Tipos de domínio entregues nas respostas

```rust
pub struct SyncItem {
    pub id: SyncItemId,
    pub kind: ItemKind,
    pub local: Option<LocalFile>,
    pub remote: Option<RemoteFile>,
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
    pub name: Option<String>,       // nome remoto, ou textual quando aplicável
    pub local_name_bytes: Option<Vec<u8>>, // bytes exatos do nome local
    pub parent_identity: Option<String>, // baseline histórico tipado será definido antes de implementar
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

O tipo `ItemKind` representa a natureza lógica do item como decisão de projeto; a presença de `Symlink` e `Other` **não** implica autorização automática para sincronizar esses tipos. A correspondência de `LocalFile.kind` e `RemoteFile.classification.kind` com `ItemKind` compete ao **Analisador**; Inventário não interpreta tipos de arquivos nem dados específicos do provedor.
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
    RemoteRef(RemoteFileRef),
    LocalId(LocalFileId),
    LocalRelativePath(LocalRelativePath),
}
pub struct GetItemResult { pub item: Option<SyncItem> }

pub struct ListItemsRequest {
    pub filter: ItemFilter,
    pub pagination: Pagination,
}
pub struct ItemFilter {
    pub statuses: Vec<SyncStatus>, // vazio = sem filtro de status
    pub local_presence: Option<PresenceState>,
    pub remote_presence: Option<PresenceState>,
    pub remote_parent: Option<RemoteFileRef>,
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
    pub expected_revision: Option<u64>, // None apenas em inclusão/descoberta ainda sem revisão
}
pub struct LocalObservation {
    pub observed: File,           // FileData::Local ou data=None para reavaliação
    pub presence: PresenceState,  // decisão/evidência fornecida pelo Analisador
    pub proof: ObservationProof,  // necessário para ausência confirmada
}
pub struct RecordRemoteObservation {
    pub selector: ObservationTarget,
    pub observation: RemoteObservation,
    pub expected_revision: Option<u64>, // None apenas em inclusão/descoberta ainda sem revisão
}
pub struct RemoteObservation {
    pub observed: File,           // FileData::Remote ou data=None para reavaliação
    pub presence: PresenceState,
    pub proof: ObservationProof,
}
pub enum ObservationTarget {
    Existing(SyncItemId),
    ByRemoteRef(RemoteFileRef),
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
    pub remote_ref: FieldUpdate<RemoteFileRef>,
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

- `RecordLocalObservation` e `RecordRemoteObservation` **nunca** avançam o baseline. Ambos recebem o envelope `models::File` **completo**, cuja variante local/remota é validada no Inventário. A revisão `SyncItem.revision` não é `File.id` nem `RemoteFile.content.version`.
- `PresenceState::AbsentConfirmed` exige `ObservationProof` confiável e escopo observado de forma suficiente, sem apagar o último registro nem quebrar os vínculos.
- `presence=Unknown` e `observed.data=None` preservam o último snapshot confiável; `Present` requer `File.data=Some` da variante correta. Um evento `Deleted` ou `ScopeUnavailable` **não** produz `AbsentConfirmed` sem prova adicional.
- Não importar nem persistir `GoogleFile` diretamente: o adaptador do provedor produz `RemoteFile` e `ProviderFileData` filtrado. Mapear os campos comuns selecionados para persistência e preservar estados `MetadataValue<T>`; não limpar propriedades não coletadas. Detalhes sensíveis e codec especializado dependem da revisão antes da implementação.
- Associar arquivos já pertencentes a outro `SyncItem` retorna erro de unicidade; nunca reatribuir silenciosamente.

### Confirmação atômica

```rust
pub struct ConfirmSyncRequest {
    pub item_id: SyncItemId,
    pub expected_item_revision: u64,
    pub confirmed: ConfirmedState,
    pub completed_operation: Option<OperationId>,
    pub activity: Option<NewActivity>, // opcional somente quando não há atividade relevante
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
    pub states: Vec<OperationState>, // vazio = sem filtro de estado
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
    pub expected_cursor: Option<String>, // None = cursor ainda não cadastrado, não ignorar concorrência
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
    pub pending_only: bool, // false inclui pendentes e já processados
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
    pub event_types: Vec<ActivityType>, // vazio = sem filtro de tipo
    pub outcomes: Vec<ActivityOutcome>, // vazio = sem filtro de resultado
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

## 11. Decisões de projeto: obrigatoriedade, opcionalidade e ausência de dados

**Esta seção define o contrato atual do Inventário**, não uma comparação histórica. A distribuição de responsabilidades é intencional: `LocalFile` e `RemoteFile` representam metadados observados; `SyncItem` representa identidade e relacionamento; `SyncBaseline` preserva apenas fatos anteriormente confirmados. Campos opcionais existem **porque o dado pode ser desconhecido, não aplicável, ainda não criado ou condicionado por uma operação**. Esses casos não devem ser confundidos.

**Decisão:** `SyncItem` não duplica nome, ID remoto ou pais dos objetos associados; consulta essas informações nas representações local/remota. Se o vínculo atual for removido, o baseline e eventuais tombstones preservam a evidência necessária à recuperação. A API não transforma `None` em valor fictício.

### Obrigatoriedade dos dados retornados

| Tipo/objeto | Sempre presente | Opcional / pode estar ausente | Regra de validação |
| --- | --- | --- | --- |
| `SyncItem` | `id`, `kind`, presenças, `status`, `revision` | `local`, `remote`, `baseline`, `conflict_reason`, `last_checked_at`, `last_synced_at` | `None` em um lado não comprova exclusão; `baseline=None` é válido antes da primeira confirmação |
| `SyncBaseline` | lados e `confirmed_at` quando criado | o próprio baseline pode não existir; nomes, pais, hashes, versões, tamanhos e datas por lado | Proibir baseline sintético apenas para preencher campos |
| `BaselineSide` | `presence` | `name`, `local_name_bytes`, `parent_identity`, `version`, `checksum`, `size`, `modified_at` | Nome local em bytes; remoto em texto; `None` em arquivos nativos provedor não é erro |
| `SyncInfo` e `SyncIdentity` | `sync_id`, `account_id`, `remote_root_id`; geração e inicialização em `SyncInfo` | Nenhum desses campos é opcional | Abrir com identidade divergente deve falhar |
| `SyncOperation` | IDs, direção, tipo, estado, revisão base, tentativas, criação | `expected_remote_version`, `next_attempt_at`, `last_error_kind`, `started_at`, `completed_at` | Não fabricar versão remota; timestamps dependem do estado |
| `RemoteCursor` | corpus, cursor, geração | Nenhum após cursor cadastrado | Ausência do cursor completo é `GetRemoteCursorResult.cursor=None` |
| `RemoteEvent` | ID do evento, corpus, tipo e data observada | `item_id`, `remote_id`, `observed_version`, `processed_at` | Nem todo evento possui item já associado; evento não reconhecido mantém `processed_at=None` |
| `SyncActivity` | ID, **`item_id`**, tipo, origem, resultado, data | `operation_id`, descrição, erro, nome histórico, ID remoto, caminho histórico | Atividade sem arquivo associado é inválida; operação pode não existir para conflito |
| `InventoryFailure` | `kind`, `message`, `retryable` | Nenhum no contrato atual | Nunca inserir token ou conteúdo privado em mensagem |
| `Page<T>` | `entries` | `next_cursor` | `None` significa fim da paginação, não erro |
| `Pagination` | `limit` | `after` | Limite positivo e com máximo documentado; `after=None` inicia consulta |

### Requisições e filtros: decisão de obrigatoriedade

| Requisição/estrutura | Campos opcionais | Campos obrigatórios / observações |
| --- | --- | --- |
| `OpenInventoryRequest` | Nenhum | `local_root`, `expected`, `create_if_missing`; `false` não é ausência, é política explícita |
| `MarkInitialDownloadCompleteRequest` | Nenhum | `expected_generation`, `expected_complete` são precondições |
| `GetItemRequest` | Nenhum | `selector` é enum; escolhe uma identificação por vez |
| `ListItemsRequest` | Filtros internos | `filter`, `pagination` são objetos; filtro vazio lista itens |
| `ItemFilter` | `local_presence`, `remote_presence`, `remote_parent`, `local_parent_item_id` | `statuses` pode ser `Vec::new()` (sem filtro) |
| `RecordLocalObservation` | `expected_revision` | `selector`, `observation`; `None` em revisão permitido somente para objeto novo |
| `LocalObservation` | nenhum | `observed: File`, `presence`, `proof`; `Present` exige `FileData::Local` |
| `RecordRemoteObservation` | `expected_revision` | `selector`, `observation`; não substituir campos omitidos |
| `RemoteObservation` | nenhum | `observed: File`, `presence`, `proof`; `Present` exige `FileData::Remote` e `RemoteFileRef` válido |
| `LinkItemRequest` | Atualizações via `FieldUpdate`, não `Option` | `item_id`, `changes`, `expected_revision`; `Keep` é válido |
| `UpdateItemStatusRequest` | `conflict_reason` via `FieldUpdate` | `item_id`, `change`, `expected_revision` |
| `ConfirmSyncRequest` | `completed_operation`, `activity` | `item_id`, `expected_item_revision`, `confirmed`; atividade obrigatória quando conclui operação com efeito relevante |
| `EnqueueOperationRequest` | `expected_remote_version` e `deduplication_key` dentro de `spec` | `item_id`, `spec`, `expected_item_revision` |
| `ListOperationsRequest` | `filter.item_id`, `filter.direction`, `filter.eligible_before` | `filter.states` vazio não filtra |
| `ClaimOperationRequest` | Nenhum | `operation_id`, `expected_state`, `claimed_at` |
| `UpdateOperationRequest` | Nenhum | `transition` contém dados específicos obrigatórios da variante |
| `PublishRemoteChangesRequest` | `checkpoint.expected_cursor`, campos opcionais dos eventos | `checkpoint` e `events`; vetor vazio pode ser válido para avançar cursor sem eventos |
| `NewRemoteEvent` | `remote_id`, `item_id`, `observed_version`, `deduplication_key` | `event_kind`, `observed_at`; deve haver identidade/sinal suficiente para reavaliação |
| `GetRemoteCursorRequest` | Nenhum | `corpus` |
| `ListRemoteEventsRequest` | `filter.corpus`, `filter.item_id` | `pending_only=false` inclui todos |
| `AcknowledgeRemoteEventRequest` | Nenhum | `event_id`, `processed_at` |
| `RecordActivityRequest` | Campos opcionais de `NewActivity` | `activity.item_id` obrigatório |
| `ListActivitiesRequest` | `filter.item_id`, `operation_id`, `from`, `until` | Vetores `event_types`/`outcomes` vazios não filtram; validar intervalo |

### Opcionalidade não significa ausência comprovada

- **Não observado:** `Option::None` ou atributo de metadados não retornado significa desconhecido/não fornecido, jamais `AbsentConfirmed` automaticamente.
- **Ausência comprovada:** `PresenceState::AbsentConfirmed` só após uma observação válida do escopo e das condições necessárias; manter a última representação para reconciliação.
- **Ainda não criado:** `baseline=None`, `GetRemoteCursorResult.cursor=None` e `GetItemResult.item=None` são ausências estruturais legítimas, mas diferentes de falha de armazenamento.
- **Não aplicável:** tamanho/checksum/versão de documentos nativos do provedor, diretórios e links podem ser indisponíveis legitimamente; não preencher valores artificiais.
- **Atualização:** `FieldUpdate::Keep` preserva, `Set` grava e `Clear` limpa explicitamente; validar quais campos admitem limpeza.
- **Estado de operação:** `started_at`, `completed_at`, `next_attempt_at` variam conforme etapa e não devem ser exigidos fora dela.
- **Vetor vazio:** filtros de lista com `Vec::new()` significam ausência de restrição; diferenciar de `None` de filtro escalar.
- **Exceções:** `RemoteFile` e `LocalFile` podem conter campos internos `Option<T>` próprios, governados pelos respectivos contratos, **não** por estas tabelas.

### Evolução de structs sem alterar métodos

`fn get_item(&self, request: GetItemRequest) -> InventoryResult<GetItemResult>` permanece com a mesma assinatura quando uma opção é adicionada à requisição. **Porém, acrescentar campo a um `pub struct` quebra código que constrói o struct com literal completo**. Para tornar a evolução realmente menos custosa, documentar construtores/builder/`Default` nos tipos de request; isso permite inicializar o mínimo necessário e acrescentar opções por métodos, sem exigir que todos os chamadores sejam reescritos. Resultados públicos podem continuar exigindo adaptação em código que realiza desestruturação exaustiva.

## 12. Contrato funcional de cada método público

As assinaturas completas estão na seção 9. Esta seção explica **cada método e os campos de seus objetos de entrada e saída**, como interpretá-los e o que não é responsabilidade da chamada. Campos de entrada não listados como `Option` devem ser informados; campos condicionais podem ser representados por enum ou por `FieldUpdate`. Todos os métodos retornam `InventoryResult<ObjetoDeResultado>`, que traduz falhas para `InventoryFailure`.

### 1. `open`

**Assinatura:** `fn open(..., request: OpenInventoryRequest) -> InventoryResult<OpenInventoryResult>`.

**Finalidade:** Abre ou cria o banco privado de uma sincronização, valida identidade e esquema, sem inicializar uma pasta de usuário já ocupada inadvertidamente.

**Parâmetros:** `local_root`: raiz real do diretório; `expected`: `sync_id`/conta/raiz remota esperados; `create_if_missing`: permissão explícita para criar (não é pedido para substituir um banco inválido).

**Retorno:** `inventory`: gerenciador pronto; `info`: identidade e estado persistidos; `created`: diferencia criação de reabertura.

**Limites e erros:** Não reconcilia arquivos, não apaga banco antigo e não recupera credenciais. Falha com `InvalidIdentity`, `CorruptDatabase`, `UnsupportedSchema` ou erro de armazenamento.

### 2. `get_sync_info`

**Assinatura:** `fn get_sync_info(..., request: GetSyncInfoRequest) -> InventoryResult<GetSyncInfoResult>`.

**Finalidade:** Consulta a identidade e o progresso estrutural da sincronização.

**Parâmetros:** Request vazio: o escopo é o banco já aberto, sem passar IDs redundantes.

**Retorno:** `info`: inclui `sync_id`, `account_id`, `remote_root_id`, geração e `initial_download_complete`.

**Limites e erros:** Somente leitura; não verifica rede, não determina se arquivos estão sincronizados.

### 3. `mark_initial_download_complete`

**Assinatura:** `fn mark_initial_download_complete(..., request: MarkInitialDownloadCompleteRequest) -> InventoryResult<MarkInitialDownloadCompleteResult>`.

**Finalidade:** Altera a marca persistente de conclusão da materialização inicial, sob condição de geração e estado anterior.

**Parâmetros:** `expected_generation`: protege geração do inventário; `expected_complete`: estado atual que o chamador espera antes de alterar. A intenção é transição para concluído, não chave arbitrária.

**Retorno:** `info`: novo estado persistido após a transição.

**Limites e erros:** Não transfere arquivos; rejeita transição sem confirmação dos pré-requisitos ou versão incompatível.

### 4. `get_item`

**Assinatura:** `fn get_item(..., request: GetItemRequest) -> InventoryResult<GetItemResult>`.

**Finalidade:** Obtém um vínculo lógico por uma única identidade.

**Parâmetros:** `selector`: `Id`, `RemoteRef`, `LocalId` ou `LocalRelativePath`; variante selecionada determina os dados obrigatórios.

**Retorno:** `item: Some`: vínculo e objetos disponíveis; `None`: nenhum registro para o seletor.

**Limites e erros:** Não prova que o arquivo não existe no filesystem ou Drive; não cria vínculos nem faz leitura de rede.

### 5. `list_items`

**Assinatura:** `fn list_items(..., request: ListItemsRequest) -> InventoryResult<ListItemsResult>`.

**Finalidade:** Lista itens com filtragem e paginação estável.

**Parâmetros:** `filter.statuses` vazio: todos os status; `local_presence`/`remote_presence` `None`: qualquer; `remote_parent`/`local_parent_item_id` `None`: qualquer pai; `pagination.limit`: máximo de itens; `after`: cursor ou primeira página.

**Retorno:** `page.entries`: itens reconstruídos; `next_cursor: None`: fim da consulta.

**Limites e erros:** Não varre o disco/Drive; cursor não pode ser reutilizado com filtro incompatível.

### 6. `record_local`

**Assinatura:** `fn record_local(..., request: RecordLocalObservation) -> InventoryResult<RecordObservationResult>`.

**Finalidade:** Insere ou atualiza a observação local e seu vínculo lógico sem modificar o baseline.

**Parâmetros:** `selector`: vínculo existente, chave local ou `NewItem`; `observation.observed`: `File` contendo metadados quando disponíveis; `presence`: situação comprovada; `proof`: origem, instante e escopo; caminho local é obtido de `LocalFile.relative_path`/`File.scope`; `expected_revision`: necessária para atualização de item existente, `None` apenas para criação.

**Retorno:** `item`: estado atualizado com revisão; `created`: informa se o vínculo foi criado.

**Limites e erros:** Não executa upload; `Unknown` preserva evidências anteriores e `AbsentConfirmed` exige prova suficiente.

### 7. `record_remote`

**Assinatura:** `fn record_remote(..., request: RecordRemoteObservation) -> InventoryResult<RecordObservationResult>`.

**Finalidade:** Registra o `File` remoto recebido do Analisador, sem chamar API do provedor nem confirmar sincronização.

**Parâmetros:** `selector`: identidade lógica/remota ou novo item; `observation.observed`: `File` com `FileData::Remote(RemoteFile)` quando presente; `presence`/`proof`: fato e evidência; `expected_revision`: versão requerida ao editar item conhecido.

**Retorno:** `item`: representação remota e estado; `created`: indica novo vínculo.

**Limites e erros:** Não apaga campos omitidos em resposta parcial; não transforma falha de API em exclusão.

### 8. `link_item`

**Assinatura:** `fn link_item(..., request: LinkItemRequest) -> InventoryResult<LinkItemResult>`.

**Finalidade:** Associa ou desassocia representações local e remota ao mesmo `SyncItem`.

**Parâmetros:** `item_id`: vínculo-alvo; `changes.local_file_id` e `changes.remote_ref`: `Keep` preserva, `Set` vincula e `Clear` remove a referência; `expected_revision`: controle otimista.

**Retorno:** `item`: vínculo com associações atualizadas.

**Limites e erros:** Não transfere ou remove arquivos; rejeita IDs de outro vínculo e limpa somente associações explicitamente solicitadas.

### 9. `update_item_status`

**Assinatura:** `fn update_item_status(..., request: UpdateItemStatusRequest) -> InventoryResult<UpdateItemStatusResult>`.

**Finalidade:** Registra a condição de processamento diagnosticada pelo consumidor autorizado.

**Parâmetros:** `item_id`: alvo; `change.status`: novo status obrigatório; `change.conflict_reason`: `Keep`, `Set` ou `Clear` para motivo; `expected_revision`: precondição.

**Retorno:** `item`: condição após alteração.

**Limites e erros:** Não resolve conflito, não decide política e não conclui fisicamente operação.

### 10. `confirm_sync`

**Assinatura:** `fn confirm_sync(..., request: ConfirmSyncRequest) -> InventoryResult<ConfirmSyncResult>`.

**Finalidade:** Confirma em transação uma operação verificada e atualiza baseline, estado operacional e histórico quando pertinente.

**Parâmetros:** `item_id`/`expected_item_revision`: alvo e precondição; `confirmed.baseline`: fatos comprovados; `status`: resultado; `verified_at`: momento da verificação; `completed_operation: None`: confirmação sem operação de fila; `activity: None`: somente quando nenhum evento histórico for devido.

**Retorno:** `item`: estado confirmado; `completed_operation`: operação concluída, quando informada; `activity`: registro criado, quando necessário.

**Limites e erros:** Não aceita operação parcial ou sem prova; não transfere conteúdo; rejeita ausência de atividade para ação historicamente relevante.

### 11. `enqueue_operation`

**Assinatura:** `fn enqueue_operation(..., request: EnqueueOperationRequest) -> InventoryResult<EnqueueOperationResult>`.

**Finalidade:** Adiciona trabalho persistente à fila, incluindo períodos offline.

**Parâmetros:** `item_id`: alvo; `spec.direction`/`kind`: ação; `expected_remote_version: None`: não há versão disponível, não significa ignorar segurança; `created_at`: instante; `deduplication_key: None`: sem chave explícita; `expected_item_revision`: precondição.

**Retorno:** `operation`: operação registrada, inicialmente pendente ou deduplicada conforme contrato.

**Limites e erros:** Não executa a operação; rejeita ação incompatível ou referência sem vínculo.

### 12. `list_operations`

**Assinatura:** `fn list_operations(..., request: ListOperationsRequest) -> InventoryResult<ListOperationsResult>`.

**Finalidade:** Consulta fila e tentativas por estado, direção e elegibilidade.

**Parâmetros:** `filter.item_id`, `direction`, `eligible_before`: `None` elimina respectiva restrição; `states` vazio: todos; `pagination`: limite e cursor.

**Retorno:** `page`: operações e cursor de continuação.

**Limites e erros:** Não toma posse de operação nem dispara execução.

### 13. `claim_operation`

**Assinatura:** `fn claim_operation(..., request: ClaimOperationRequest) -> InventoryResult<ClaimOperationResult>`.

**Finalidade:** Reivindica atomicamente uma operação para execução por um worker.

**Parâmetros:** `operation_id`: alvo; `expected_state`: estado exigido (normalmente pendente/retomável); `claimed_at`: instante da reivindicação.

**Retorno:** `operation`: operação com estado atualizado para execução.

**Limites e erros:** Não executa upload/download; reivindicações concorrentes incompatíveis falham, sem dupla execução autorizada.

### 14. `update_operation`

**Assinatura:** `fn update_operation(..., request: UpdateOperationRequest) -> InventoryResult<UpdateOperationResult>`.

**Finalidade:** Registra transições de retry, bloqueio, falha ou retorno à fila.

**Parâmetros:** `operation_id`/`expected_state`: alvo/precondição; `transition`: enum com os campos exigidos pela variante (`next_attempt_at`, `error_kind`, `failed_at` ou `reason`).

**Retorno:** `operation`: novo estado, tentativas e diagnósticos pertinentes.

**Limites e erros:** Não possui variante `Complete`; conclusão verificada só acontece em `confirm_sync`.

### 15. `publish_remote_changes`

**Assinatura:** `fn publish_remote_changes(..., request: PublishRemoteChangesRequest) -> InventoryResult<PublishRemoteChangesResult>`.

**Finalidade:** Publica eventos remotos e avanço de cursor na mesma transação SQLite.

**Parâmetros:** `checkpoint.corpus` identifica fluxo; `expected_cursor: None` exige que ainda não exista cursor; `new_cursor` e `expected_generation` validam continuidade; `events` pode ser vazio para página sem mudanças; cada evento tem tipo/data obrigatórios, IDs/versão/deduplicação opcionais.

**Retorno:** `cursor`: checkpoint gravado; `inserted` e `duplicate`: contagens verificáveis.

**Limites e erros:** Não marca eventos como processados nem determina exclusão de arquivo a partir do evento.

### 16. `get_remote_cursor`

**Assinatura:** `fn get_remote_cursor(..., request: GetRemoteCursorRequest) -> InventoryResult<GetRemoteCursorResult>`.

**Finalidade:** Consulta a última posição durável do detector remoto.

**Parâmetros:** `corpus`: fluxo remoto solicitado.

**Retorno:** `cursor: Some` indica checkpoint conhecido; `None` indica ainda não registrado.

**Limites e erros:** Não cria cursor e não realiza chamada remota.

### 17. `list_remote_events`

**Assinatura:** `fn list_remote_events(..., request: ListRemoteEventsRequest) -> InventoryResult<ListRemoteEventsResult>`.

**Finalidade:** Lista eventos observados, priorizando os pendentes quando solicitado.

**Parâmetros:** `filter.corpus`/`item_id`: `None` não restringe; `pending_only=true`: sem `processed_at`; `false`: inclui todos; `pagination` limita.

**Retorno:** `page`: eventos persistidos e cursor de listagem.

**Limites e erros:** Não reconhece processamento automaticamente.

### 18. `acknowledge_remote_event`

**Assinatura:** `fn acknowledge_remote_event(..., request: AcknowledgeRemoteEventRequest) -> InventoryResult<AcknowledgeRemoteEventResult>`.

**Finalidade:** Registra que um evento remoto foi efetivamente tratado.

**Parâmetros:** `event_id`: identificador do sinal; `processed_at`: instante de conclusão reconhecida.

**Retorno:** `event`: mesmo evento com confirmação de processamento.

**Limites e erros:** Não prova sincronização de conteúdo por si só; consumidor precisa ter reavaliado antes.

### 19. `record_activity`

**Assinatura:** `fn record_activity(..., request: RecordActivityRequest) -> InventoryResult<RecordActivityResult>`.

**Finalidade:** Grava evento histórico de um arquivo fora do caminho de confirmação atômica, como conflito detectado ou falha.

**Parâmetros:** `activity.item_id`: obrigatório; `operation_id`: opcional; `event_type`, `origin`, `outcome`, `occurred_at`: obrigatórios; descrição/erro/nome/caminho/ID remoto opcionais preservam contexto quando disponível.

**Retorno:** `activity`: registro persistido com ID próprio.

**Limites e erros:** Não registra eventos globais sem arquivo; não substitui `confirm_sync` ao concluir operação.

### 20. `list_activities`

**Assinatura:** `fn list_activities(..., request: ListActivitiesRequest) -> InventoryResult<ListActivitiesResult>`.

**Finalidade:** Consulta o histórico de arquivos da sincronização, com paginação temporal.

**Parâmetros:** `filter.item_id`/`operation_id`: `None` não restringe; `event_types`/`outcomes` vazios: todos; `from`/`until`: opcionais com intervalo validado; `pagination`: tamanho e cursor.

**Retorno:** `page`: atividades históricas com identidade do arquivo e contexto disponível.

**Limites e erros:** Não altera eventos, não mistura registros técnicos globais ou de outras sincronizações.

### Regras de retorno e contratos condicionais

- Uma busca sem resultado válido retorna `GetItemResult { item: None }`, não necessariamente `NotFound`. `NotFound` aplica-se quando uma operação de escrita requer um item existente.
- Uma consulta paginada vazia retorna `Page { entries: vec![], next_cursor: None }` quando não há continuidade; não significa banco corrompido.
- `RecordObservationResult.created` significa **novo registro no inventário**, não um arquivo fisicamente criado.
- `ConfirmSyncResult.completed_operation` e `.activity` são opcionais porque nem toda confirmação envolve uma operação da fila ou um evento histórico; quando a ação é relevante e concluída, a atividade torna-se exigência condicional.
- `PublishRemoteChangesResult.duplicate` só é significativo quando existe política de deduplicação confiável para o evento; não inventar equivalência apenas por coincidência de data.
- `InventoryFailure.retryable` indica possibilidade de uma **nova tentativa controlada**, não permissão para repetir uma escrita sem verificar identidade, revisão, idempotência ou resultado anterior.

## Regra adicional — observações não associáveis

O método `record_local` aceita apenas `File` cujo `scope` corresponde ao lado local; `record_remote` apenas ao lado remoto. Quando `File.data=Some`, sua variante deve corresponder à requisição, e o objeto é armazenado sem projeção arbitrária de metadados autorizados. `File.data=None` pode atualizar diagnóstico/prova **somente se o alvo já estiver seguramente associado**. Eventos globais, `FileScope::WholeSync` e sinais sem identidade confiável não criam `SyncItem` por `NewItem` automaticamente. Permanecem no fluxo do Detector/Analisador até decisão de escopo.

A identidade do `File` (ocorrência) não deve ser persistida como chave do `SyncItem`. A implementação futura deverá decidir a retenção de referência de última observação e a idempotência, sem duplicar a fila de entrega do Detector. O Inventário não decide a ação a partir de `FileEventKind`, embora preserve dados do evento para fins de prova/diagnóstico quando isso for aprovado.

**Bloqueio conhecido:** esta especificação ainda apresenta `RemoteCursor` e `RemoteEvent` e quatro métodos relacionados, herdados do contrato do Inventário antes do Detector unificado. Eles colidem parcialmente com `RemoteCheckpoint` e `EventStore`. **Não implementar as APIs em duplicidade** até decidir a propriedade única do checkpoint/evento, sem adaptação legada.

## 13. Garantias de evolução e testes

1. **Assinatura estável:** novos atributos entram em `RecordLocalObservation`, `ItemFilter`, `NewSyncOperation` etc.; não acrescentar parâmetros posicionais.
2. **Objetos coesos:** estruturas de entrada descrevem uma operação; dados de observação são agrupados em `LocalObservation`/`RemoteObservation`; não usar `HashMap<String, Value>` como API genérica.
3. **Comportamento opcional explícito:** novas propriedades de entrada devem preservar o comportamento quando não fornecidas. Se a semântica mudar, atualizar o contrato e testar deliberadamente.
4. **Contrato separado da persistência:** alterar colunas, índices e tabelas não muda automaticamente `SyncItem` nem as assinaturas públicas.
5. **TDD:** testar primeiro casos mínimos, opcionais, erros, idempotência, rollback, paginação, FKs, revisão/concorrência, reinício e dados parciais.
6. **Banco real isolado:** testes de Inventário usam SQLite real em diretório temporário, com `LocalFile` e `RemoteFile` sintéticos, sem depender de provedor nem de outros módulos funcionando.
7. **Segurança:** atividades vinculadas a item, nenhum dado pessoal/segredo em erro, ausência observada versus desconhecida sempre diferenciadas.
8. **Sem compatibilidade legada:** não adicionar adaptadores/migração para estados antigos por padrão; proteger arquivos do usuário e falhar com erro explícito quando esquema incompatível.

### Pontos ainda dependentes de outra especificação

- Campos definitivos de `LocalFile` e como reconstruir fielmente `LocalFile` de colunas.
- Projeção relacional e reconstrução de `RemoteFile` completo, inclusive campos não retornados pelo Drive.
- Semântica exata de `ObservationProof.complete_scope`, chaves de deduplicação, paginação e política de retenção.
- Tipo concreto de tempo e de bytes locais no projeto.
- Definição de operações simultâneas sobre um mesmo item e estratégias de lock. Estes detalhes devem ser fechados antes de implementar os respectivos métodos, **sem mudar a orientação a objetos da API**.

**Importante:** este documento é o contrato de projeto para implementação e seus invariantes, não comprova compilação Rust nem implementação. O próximo passo é descrever `TESTS.md` e criar testes que falhem antes de implementar o módulo.
