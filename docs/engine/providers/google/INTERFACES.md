# Google — INTERFACES

**Fronteira técnica:** esta é a documentação da API Google diretamente no submódulo `google/`. As assinaturas e tipos aqui descritos mapeiam operações técnicas do serviço, não processos de negócio.

**Contrato Rust v1 para TDD.** Todas as operações e os tipos declarados são públicos nesta versão. As assinaturas representam a API que será implementada, não código existente. Não há compromisso de reduzir a visibilidade dos métodos depois.

**Escopo:** os **59 métodos oficiais dos 13 recursos REST da Drive API v3** documentados no [índice oficial](https://developers.google.com/workspace/drive/api/reference/rest/v3), mais OAuth 2.0, OpenID Connect, uploads e downloads. APIs de outros produtos Google, como Gmail, Calendar e Drive Labels API v2, são serviços separados.

**Fontes:** [Drive REST v3](https://developers.google.com/workspace/drive/api/reference/rest/v3), [Discovery v3](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest), [OAuth desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [OpenID Connect](https://developers.google.com/identity/openid-connect/reference).

## Execução síncrona e assíncrona

**Decisão aprovada (substitui a lacuna anterior):** toda operação que aguarda HTTP ou transfere bytes é `async fn`, executada sobre `reqwest` assíncrono e Tokio; os métodos dos blocos de assinatura abaixo escritos como `fn` são a forma conceitual e devem ser lidos como `async fn` (neste grupo, só `files_get` está implementado e escrito assim). Funções puras (construção de parâmetros, serialização/decodificação e tradução de erros) permanecem síncronas. Esta decisão **não** define política de concorrência, número de workers, retry, agendamento nem o tipo concreto de `GoogleByteStream` (hoje conceitual; continua pendente para o grupo de transferências).

Não modificar automaticamente os 59 métodos REST, 9 OAuth/OIDC e 10 complementares para um único `trait` ou protocolo artificial. Manter argumentos, semântica, tipos e contratos específicos da API Google; ajustar apenas a estratégia efetiva de execução e streaming quando validada.

### Estado da chamada — não estado global

| Operação técnica | Informação que a API permite conhecer |
| --- | --- |
| REST comum e OAuth/OIDC | Resultado/erro tipado da requisição; timeout ou resposta perdida pode deixar efeito remoto indeterminado |
| Listagens e mudanças | Página/cursor retornados, não conclusão de uma varredura inteira |
| Upload resumível | `GoogleUploadProgress::Incomplete` e intervalo efetivamente confirmado; `Complete` apenas quando a API confirma |
| Download/exportação | Stream e metadados HTTP da resposta; a leitura dos bytes e a integridade final são controladas pelo consumidor |
| Operação remota de longa duração | `GoogleOperation.done` e eventual consulta explícita `operations_get`, sem polling automático obrigatório |

O módulo **não** cria `GoogleProviderStatus`, consulta `status()` global, callbacks de interface, agenda de transferências, armazenamento de progresso por arquivo, fila de retry ou métodos `start/pause/stop`. Cada método é uma chamada técnica delimitada. Não prometer cancelamento de efeito já enviado; interrupção da leitura/HTTP não comprova reversão. Quando a resposta de uma operação de escrita se perde após o envio, preservar a incerteza; o consumidor decide como verificar o estado antes de repetir.

**Feedback técnico:** manter erros tipados, `retryable`, indicação `retry_after_seconds`, estado parcial e limites do protocolo quando existirem. Bytes transportados pelo stream não se transformam em uma porcentagem de sincronização dentro do provider. O `ProgressReporter` pertence ao Operador e não deve ser importado por `providers/google/`.

## 1. Tipos públicos fundamentais e erros

```rust
pub type GoogleResult<T> = Result<T, GoogleApiFailure>;
pub type GoogleByteStream = Box<dyn std::io::Read + Send>;

pub struct FileId(pub String);
pub struct DriveId(pub String);
pub struct ApprovalId(pub String);
pub struct ProposalId(pub String);
pub struct AppId(pub String);
pub struct CommentId(pub String);
pub struct ReplyId(pub String);
pub struct PermissionId(pub String);
pub struct RevisionId(pub String);
pub struct OperationName(pub String);
pub struct ChannelId(pub String);
pub struct RequestId(pub String);
pub struct MimeType(pub String);
pub struct FieldMask(pub String);
pub struct GoogleSession { pub account_id: String /* segredo encapsulado */ }

pub struct PageOptions {
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub fields: Option<FieldMask>,
}
pub struct GooglePage<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
}
pub enum GoogleApiError {
    InvalidGrant,
    InsufficientScope,
    StorageQuotaExceeded,
    FileNotDownloadable,
    InvalidArgument,
    AuthenticationRequired,
    ReauthorizationRequired,
    PermissionDenied,
    NotFoundOrInaccessible,
    PreconditionFailed,
    RateLimited,
    QuotaExceeded,
    ProviderUnavailable,
    Network,
    Timeout,
    Cancelled,
    InvalidResponse,
    UnsupportedOperation,
    UnsupportedMimeType,
    ExpiredCursor,
    ExpiredUploadSession,
    IncompleteTransfer,
}

pub struct GoogleApiFailure {
    pub kind: GoogleApiError,
    pub http_status: Option<u16>,
    pub provider_reason: Option<String>,
    pub retry_after_seconds: Option<u64>,
    pub retryable: bool,
}
```

| Convenção | Regra |
| --- | --- |
| Visibilidade | Todos os traits, métodos e tipos públicos nesta versão |
| Operações | Todas recebem sessão vinculada à conta; nunca token bruto como argumento |
| Campos | Campos omitíveis da resposta usam `Option<T>` |
| Atualização | Objetos `*Write` distinguem **não alterar** e **limpar** um campo |
| Lista | Paginação explícita, não inferir completude de uma única página |
| Tipos Google | Serialização JSON, parâmetros HTTP e URLs ficam encapsulados |
| Fluxos | `GoogleByteStream` é assinatura conceitual; I/O async poderá ser escolhido na validação técnica |
| Resultados | Nenhuma resposta parcial se converte em sucesso final |
| Erros | Categorias sanitizadas; sem credenciais, tokens e corpo HTTP privado |

## 2. Tipos públicos e dados retornados

**Todos os tipos usados nas assinaturas pertencem ao contrato público nesta versão.** A relação completa, com declarações Rust de estruturas e campos, está na [seção 9](#9-declarações-dos-tipos-públicos). Os tipos comuns de identidade e erro ficam na seção 1; "comum" não significa que os demais são privados ou opcionais.

| Conjunto | Tipos de saída e entrada públicos |
| --- | --- |
| Files | `GoogleFile`, `GoogleFileWrite`, `GoogleFileList`, `GoogleFileCapabilities`, `GoogleShortcutDetails`, opções de Files |
| Changes | `GoogleChange`, `GoogleChangeList`, `GoogleStartPageToken`, opções de Changes |
| Drives | `GoogleDrive`, `GoogleDriveList`, `GoogleDriveRestrictions`, opções de Drives |
| Permissions | `GooglePermission`, `GooglePermissionList`, opções e solicitações |
| Comments e Replies | `GoogleComment`, `GoogleReply`, suas listas, opções e tipos de escrita |
| Revisions | `GoogleRevision`, `GoogleRevisionList`, opções |
| About e Apps | `GoogleAbout`, `GoogleApp`, `GoogleAppList`, opções |
| Access proposals e Approvals | `GoogleAccessProposal`, `GoogleApproval`, listas, opções e solicitações |
| Channels, Labels, CSE e Operations | `GoogleChannel`, `GoogleLabel`, `GoogleOperation`, `GoogleCseToken`, respostas e opções |
| OAuth / OIDC | `GoogleOAuthConfig`, `GoogleTokenResponse`, `GoogleUserInfo`, `GoogleIdClaims`, `GoogleOidcDiscovery`, solicitações |
| Transferências | `GoogleUploadSession`, `GoogleUploadChunk`, `GoogleUploadProgress`, `GoogleDownloadStream`, `GoogleByteRange` |

**Regra de disponibilidade:** uma propriedade pode ser **não solicitada**, **não disponibilizada para aquele tipo de recurso**, **inacessível àquela conta** ou **efetivamente sem valor**. `None` em um campo Rust não distingue sozinho essas quatro situações. A seleção de `fields` é conhecida pelo chamador; quando a distinção afetar o significado dos dados retornados, a API deverá carregar informação de completude/projeção da resposta. Campo omitido não constitui, por si só, prova de remoção ou de valor apagado no serviço.

### Entradas dos métodos

A **assinatura Rust é o contrato principal**. Os argumentos identificados nas assinaturas não são necessariamente opcionais: a coluna **“Entradas além de `session`”** da seção 3 omite apenas a sessão, presente em todas as operações.

| Elemento na assinatura | Obrigatoriedade | Significado |
| --- | --- | --- |
| `session: &GoogleSession` | Obrigatório nos métodos autenticados | Identifica a conta e fornece acesso autorizado |
| `file_id: &FileId` e outros IDs explícitos | Obrigatórios quando aparecem na assinatura | Identificam recursos; não podem ser omitidos |
| `options: XxxOptions` | Objeto obrigatório na assinatura; **campos internos podem ser opcionais** | Agrupa parâmetros de consulta ou opções da operação |
| `request: XxxRequest` ou `request: XxxWrite` | Obrigatório quando aparece na assinatura | Representa o corpo enviado para criar, modificar ou executar uma ação |
| `mime_type: &MimeType` e outros valores explícitos | Obrigatórios quando aparecem na assinatura | Representam parâmetros exigidos pela operação |

**Exemplo — consulta de metadados:**

```rust
async fn files_get(
    &self,
    session: &GoogleSession,
    file_id: &FileId,
    options: FileGetOptions,
) -> GoogleResult<GoogleFile>;

// Exemplo do tipo de entrada (contrato proposto)
// FileGetOptions está definido uma única vez na seção 8.2.
```

| Entrada | Obrigatória? | Enviada ao Google como | Finalidade |
| --- | --- | --- | --- |
| `session` | Sim | Autorização HTTP | Acessar a conta correta |
| `file_id` | Sim | Parâmetro do caminho `{fileId}` | Identificar o arquivo |
| `options.fields` | Não | Query `fields` | Selecionar campos de resposta |
| `options.supports_all_drives` | Não | Query `supportsAllDrives` | Informar suporte a drives compartilhados |
| **Retorno** | — | Resposta JSON `File` | `GoogleResult<GoogleFile>` |

**Exemplo — atualização de metadados:**

```rust
fn files_update(
    &self,
    session: &GoogleSession,
    file_id: &FileId,
    request: GoogleFileWrite,
    options: FileUpdateOptions,
) -> GoogleResult<GoogleFile>;
```

| Entrada | Onde é utilizada | Finalidade |
| --- | --- | --- |
| `file_id` | Caminho `{fileId}` | Recurso a atualizar |
| `request: GoogleFileWrite` | **Corpo JSON (body)** | Campos que devem ser modificados |
| `options: FileUpdateOptions` | Query da operação | Parâmetros como `fields`, `addParents` ou `removeParents`, conforme aplicáveis |
| **Retorno** | Resposta JSON `File` | `GoogleResult<GoogleFile>` |

### Convenções para definir os tipos de entrada

| Categoria | Exemplos | Regra |
| --- | --- | --- |
| Identificadores | `FileId`, `DriveId`, `RevisionId` | Tipos individuais usados como argumentos explícitos |
| Consulta e paginação | `FileGetOptions`, `FileListOptions`, `ChangeListOptions`, `PageOptions` | Cada estrutura representa **somente os parâmetros aceitos pela operação correspondente** |
| Configuração de operações | `FileCreateOptions`, `FileUpdateOptions`, `PermissionCreateOptions` | Opções de envio/resultado, não conteúdo do recurso |
| Dados enviados | `GoogleFileWrite`, `GooglePermissionWrite`, `GoogleCommentWrite` | Corpo da requisição; distinguir **campo não informado** de **campo explicitamente limpo** |
| Solicitações de ações | `ResolveAccessProposalRequest`, `StartApprovalRequest`, `GoogleModifyLabelsRequest` | Corpo específico da ação, com campos exigidos pelo endpoint |
| Canais e notificações | `GoogleChannelRequest`, `GoogleChannelStopRequest` | Dados necessários para iniciar ou interromper notificações |

Os tipos `XxxOptions` e `XxxRequest` citados nas assinaturas são **tipos públicos a detalhar individualmente**. Para cada um, a definição Rust deverá declarar campos, tipos, obrigatoriedade, validações e correspondência com parâmetros HTTP ou corpo JSON, conforme a documentação oficial do método. Uma lista genérica de parâmetros compartilhada entre vários endpoints **não constitui** a definição desses tipos.

## 3. Assinaturas públicas por recurso REST

### 3.1 about (1 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `about_get` | `options: AboutOptions` | `GoogleResult<GoogleAbout>` | `GET /about` |

```rust
pub trait GoogleAboutApi {
    fn about_get(&self, session: &GoogleSession, options: AboutOptions) -> GoogleResult<GoogleAbout>;
}
```

### 3.2 accessproposals (3 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `access_proposals_get` | `file_id: &FileId, proposal_id: &ProposalId` | `GoogleResult<GoogleAccessProposal>` | `GET /files/{fileId}/accessproposals/{proposalId}` |
| `access_proposals_list` | `file_id: &FileId, options: PageOptions` | `GoogleResult<GoogleAccessProposalList>` | `GET /files/{fileId}/accessproposals` |
| `access_proposals_resolve` | `file_id: &FileId, proposal_id: &ProposalId, request: ResolveAccessProposalRequest` | `GoogleResult<()>` | `POST /files/{fileId}/accessproposals/{proposalId}:resolve` |

```rust
pub trait GoogleAccessproposalsApi {
    fn access_proposals_get(&self, session: &GoogleSession, file_id: &FileId, proposal_id: &ProposalId) -> GoogleResult<GoogleAccessProposal>;
    fn access_proposals_list(&self, session: &GoogleSession, file_id: &FileId, options: PageOptions) -> GoogleResult<GoogleAccessProposalList>;
    fn access_proposals_resolve(&self, session: &GoogleSession, file_id: &FileId, proposal_id: &ProposalId, request: ResolveAccessProposalRequest) -> GoogleResult<()>;
}
```

### 3.3 approvals (8 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `approvals_approve` | `file_id: &FileId, approval_id: &ApprovalId, request: ApproveApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals/{approvalId}:approve` |
| `approvals_cancel` | `file_id: &FileId, approval_id: &ApprovalId, request: CancelApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals/{approvalId}:cancel` |
| `approvals_comment` | `file_id: &FileId, approval_id: &ApprovalId, request: CommentApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals/{approvalId}:comment` |
| `approvals_decline` | `file_id: &FileId, approval_id: &ApprovalId, request: DeclineApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals/{approvalId}:decline` |
| `approvals_get` | `file_id: &FileId, approval_id: &ApprovalId` | `GoogleResult<GoogleApproval>` | `GET /files/{fileId}/approvals/{approvalId}` |
| `approvals_list` | `file_id: &FileId, options: PageOptions` | `GoogleResult<GoogleApprovalList>` | `GET /files/{fileId}/approvals` |
| `approvals_reassign` | `file_id: &FileId, approval_id: &ApprovalId, request: ReassignApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals/{approvalId}:reassign` |
| `approvals_start` | `file_id: &FileId, request: StartApprovalRequest` | `GoogleResult<GoogleApproval>` | `POST /files/{fileId}/approvals:start` |

```rust
pub trait GoogleApprovalsApi {
    fn approvals_approve(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId, request: ApproveApprovalRequest) -> GoogleResult<GoogleApproval>;
    fn approvals_cancel(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId, request: CancelApprovalRequest) -> GoogleResult<GoogleApproval>;
    fn approvals_comment(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId, request: CommentApprovalRequest) -> GoogleResult<GoogleApproval>;
    fn approvals_decline(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId, request: DeclineApprovalRequest) -> GoogleResult<GoogleApproval>;
    fn approvals_get(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId) -> GoogleResult<GoogleApproval>;
    fn approvals_list(&self, session: &GoogleSession, file_id: &FileId, options: PageOptions) -> GoogleResult<GoogleApprovalList>;
    fn approvals_reassign(&self, session: &GoogleSession, file_id: &FileId, approval_id: &ApprovalId, request: ReassignApprovalRequest) -> GoogleResult<GoogleApproval>;
    fn approvals_start(&self, session: &GoogleSession, file_id: &FileId, request: StartApprovalRequest) -> GoogleResult<GoogleApproval>;
}
```

### 3.4 apps (2 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `apps_get` | `app_id: &AppId` | `GoogleResult<GoogleApp>` | `GET /apps/{appId}` |
| `apps_list` | `options: AppListOptions` | `GoogleResult<GoogleAppList>` | `GET /apps` |

```rust
pub trait GoogleAppsApi {
    fn apps_get(&self, session: &GoogleSession, app_id: &AppId) -> GoogleResult<GoogleApp>;
    fn apps_list(&self, session: &GoogleSession, options: AppListOptions) -> GoogleResult<GoogleAppList>;
}
```

### 3.5 changes (3 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `changes_get_start_page_token` | `options: ChangeScopeOptions` | `GoogleResult<GoogleStartPageToken>` | `GET /changes/startPageToken` |
| `changes_list` | `options: ChangeListOptions` | `GoogleResult<GoogleChangeList>` | `GET /changes` |
| `changes_watch` | `options: ChangeWatchOptions, channel: GoogleChannelRequest` | `GoogleResult<GoogleChannel>` | `POST /changes/watch` |

```rust
pub trait GoogleChangesApi {
    fn changes_get_start_page_token(&self, session: &GoogleSession, options: ChangeScopeOptions) -> GoogleResult<GoogleStartPageToken>;
    fn changes_list(&self, session: &GoogleSession, options: ChangeListOptions) -> GoogleResult<GoogleChangeList>;
    fn changes_watch(&self, session: &GoogleSession, options: ChangeWatchOptions, channel: GoogleChannelRequest) -> GoogleResult<GoogleChannel>;
}
```

### 3.6 channels (1 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `channels_stop` | `channel: GoogleChannelStopRequest` | `GoogleResult<()>` | `POST /channels/stop` |

```rust
pub trait GoogleChannelsApi {
    fn channels_stop(&self, session: &GoogleSession, channel: GoogleChannelStopRequest) -> GoogleResult<()>;
}
```

### 3.7 comments (5 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `comments_create` | `file_id: &FileId, request: GoogleCommentWrite, options: CommentOptions` | `GoogleResult<GoogleComment>` | `POST /files/{fileId}/comments` |
| `comments_delete` | `file_id: &FileId, comment_id: &CommentId` | `GoogleResult<()>` | `DELETE /files/{fileId}/comments/{commentId}` |
| `comments_get` | `file_id: &FileId, comment_id: &CommentId, options: CommentOptions` | `GoogleResult<GoogleComment>` | `GET /files/{fileId}/comments/{commentId}` |
| `comments_list` | `file_id: &FileId, options: CommentListOptions` | `GoogleResult<GoogleCommentList>` | `GET /files/{fileId}/comments` |
| `comments_update` | `file_id: &FileId, comment_id: &CommentId, request: GoogleCommentWrite, options: CommentOptions` | `GoogleResult<GoogleComment>` | `PATCH /files/{fileId}/comments/{commentId}` |

```rust
pub trait GoogleCommentsApi {
    fn comments_create(&self, session: &GoogleSession, file_id: &FileId, request: GoogleCommentWrite, options: CommentOptions) -> GoogleResult<GoogleComment>;
    fn comments_delete(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId) -> GoogleResult<()>;
    fn comments_get(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, options: CommentOptions) -> GoogleResult<GoogleComment>;
    fn comments_list(&self, session: &GoogleSession, file_id: &FileId, options: CommentListOptions) -> GoogleResult<GoogleCommentList>;
    fn comments_update(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, request: GoogleCommentWrite, options: CommentOptions) -> GoogleResult<GoogleComment>;
}
```

### 3.8 drives (7 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `drives_create` | `request_id: &RequestId, request: GoogleDriveWrite` | `GoogleResult<GoogleDrive>` | `POST /drives` |
| `drives_delete` | `drive_id: &DriveId, options: DriveDeleteOptions` | `GoogleResult<()>` | `DELETE /drives/{driveId}` |
| `drives_get` | `drive_id: &DriveId, options: DriveGetOptions` | `GoogleResult<GoogleDrive>` | `GET /drives/{driveId}` |
| `drives_hide` | `drive_id: &DriveId` | `GoogleResult<GoogleDrive>` | `POST /drives/{driveId}/hide` |
| `drives_list` | `options: DriveListOptions` | `GoogleResult<GoogleDriveList>` | `GET /drives` |
| `drives_unhide` | `drive_id: &DriveId` | `GoogleResult<GoogleDrive>` | `POST /drives/{driveId}/unhide` |
| `drives_update` | `drive_id: &DriveId, request: GoogleDriveWrite, options: DriveUpdateOptions` | `GoogleResult<GoogleDrive>` | `PATCH /drives/{driveId}` |

```rust
pub trait GoogleDrivesApi {
    fn drives_create(&self, session: &GoogleSession, request_id: &RequestId, request: GoogleDriveWrite) -> GoogleResult<GoogleDrive>;
    fn drives_delete(&self, session: &GoogleSession, drive_id: &DriveId, options: DriveDeleteOptions) -> GoogleResult<()>;
    fn drives_get(&self, session: &GoogleSession, drive_id: &DriveId, options: DriveGetOptions) -> GoogleResult<GoogleDrive>;
    fn drives_hide(&self, session: &GoogleSession, drive_id: &DriveId) -> GoogleResult<GoogleDrive>;
    fn drives_list(&self, session: &GoogleSession, options: DriveListOptions) -> GoogleResult<GoogleDriveList>;
    fn drives_unhide(&self, session: &GoogleSession, drive_id: &DriveId) -> GoogleResult<GoogleDrive>;
    fn drives_update(&self, session: &GoogleSession, drive_id: &DriveId, request: GoogleDriveWrite, options: DriveUpdateOptions) -> GoogleResult<GoogleDrive>;
}
```

### 3.9 files (14 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `files_copy` | `file_id: &FileId, request: GoogleFileWrite, options: FileCopyOptions` | `GoogleResult<GoogleFile>` | `POST /files/{fileId}/copy` |
| `files_create` | `request: GoogleFileWrite, options: FileCreateOptions` | `GoogleResult<GoogleFile>` | `POST /files` |
| `files_delete` | `file_id: &FileId, options: FileDeleteOptions` | `GoogleResult<()>` | `DELETE /files/{fileId}` |
| `files_download` | `file_id: &FileId, options: FileDownloadOptions` | `GoogleResult<GoogleOperation>` | `POST /files/{fileId}/download` |
| `files_empty_trash` | `options: EmptyTrashOptions` | `GoogleResult<()>` | `DELETE /files/trash` |
| `files_export` | `file_id: &FileId, mime_type: &MimeType` | `GoogleResult<GoogleByteStream>` | `GET /files/{fileId}/export` |
| `files_generate_cse_token` | `options: CseTokenOptions` | `GoogleResult<GoogleCseToken>` | `GET /files/generateCseToken` |
| `files_generate_ids` | `options: GenerateIdsOptions` | `GoogleResult<GoogleGeneratedIds>` | `GET /files/generateIds` |
| `files_get` | `file_id: &FileId, options: FileGetOptions` | `GoogleResult<GoogleFile>` | `GET /files/{fileId}` |
| `files_list` | `options: FileListOptions` | `GoogleResult<GoogleFileList>` | `GET /files` |
| `files_list_labels` | `file_id: &FileId, options: LabelListOptions` | `GoogleResult<GoogleLabelList>` | `GET /files/{fileId}/listLabels` |
| `files_modify_labels` | `file_id: &FileId, request: GoogleModifyLabelsRequest` | `GoogleResult<GoogleModifyLabelsResponse>` | `POST /files/{fileId}/modifyLabels` |
| `files_update` | `file_id: &FileId, request: GoogleFileWrite, options: FileUpdateOptions` | `GoogleResult<GoogleFile>` | `PATCH /files/{fileId}` |
| `files_watch` | `file_id: &FileId, options: FileWatchOptions, channel: GoogleChannelRequest` | `GoogleResult<GoogleChannel>` | `POST /files/{fileId}/watch` |

```rust
pub trait GoogleFilesApi {
    fn files_copy(&self, session: &GoogleSession, file_id: &FileId, request: GoogleFileWrite, options: FileCopyOptions) -> GoogleResult<GoogleFile>;
    fn files_create(&self, session: &GoogleSession, request: GoogleFileWrite, options: FileCreateOptions) -> GoogleResult<GoogleFile>;
    fn files_delete(&self, session: &GoogleSession, file_id: &FileId, options: FileDeleteOptions) -> GoogleResult<()>;
    fn files_download(&self, session: &GoogleSession, file_id: &FileId, options: FileDownloadOptions) -> GoogleResult<GoogleOperation>;
    fn files_empty_trash(&self, session: &GoogleSession, options: EmptyTrashOptions) -> GoogleResult<()>;
    fn files_export(&self, session: &GoogleSession, file_id: &FileId, mime_type: &MimeType) -> GoogleResult<GoogleByteStream>;
    fn files_generate_cse_token(&self, session: &GoogleSession, options: CseTokenOptions) -> GoogleResult<GoogleCseToken>;
    fn files_generate_ids(&self, session: &GoogleSession, options: GenerateIdsOptions) -> GoogleResult<GoogleGeneratedIds>;
    async fn files_get(&self, session: &GoogleSession, file_id: &FileId, options: FileGetOptions) -> GoogleResult<GoogleFile>;
    fn files_list(&self, session: &GoogleSession, options: FileListOptions) -> GoogleResult<GoogleFileList>;
    fn files_list_labels(&self, session: &GoogleSession, file_id: &FileId, options: LabelListOptions) -> GoogleResult<GoogleLabelList>;
    fn files_modify_labels(&self, session: &GoogleSession, file_id: &FileId, request: GoogleModifyLabelsRequest) -> GoogleResult<GoogleModifyLabelsResponse>;
    fn files_update(&self, session: &GoogleSession, file_id: &FileId, request: GoogleFileWrite, options: FileUpdateOptions) -> GoogleResult<GoogleFile>;
    fn files_watch(&self, session: &GoogleSession, file_id: &FileId, options: FileWatchOptions, channel: GoogleChannelRequest) -> GoogleResult<GoogleChannel>;
}
```

### 3.10 operations (1 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `operations_get` | `name: &OperationName` | `GoogleResult<GoogleOperation>` | `GET /operations/{name}` |

```rust
pub trait GoogleOperationsApi {
    fn operations_get(&self, session: &GoogleSession, name: &OperationName) -> GoogleResult<GoogleOperation>;
}
```

### 3.11 permissions (5 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `permissions_create` | `file_id: &FileId, request: GooglePermissionWrite, options: PermissionCreateOptions` | `GoogleResult<GooglePermission>` | `POST /files/{fileId}/permissions` |
| `permissions_delete` | `file_id: &FileId, permission_id: &PermissionId, options: PermissionDeleteOptions` | `GoogleResult<()>` | `DELETE /files/{fileId}/permissions/{permissionId}` |
| `permissions_get` | `file_id: &FileId, permission_id: &PermissionId, options: PermissionGetOptions` | `GoogleResult<GooglePermission>` | `GET /files/{fileId}/permissions/{permissionId}` |
| `permissions_list` | `file_id: &FileId, options: PermissionListOptions` | `GoogleResult<GooglePermissionList>` | `GET /files/{fileId}/permissions` |
| `permissions_update` | `file_id: &FileId, permission_id: &PermissionId, request: GooglePermissionWrite, options: PermissionUpdateOptions` | `GoogleResult<GooglePermission>` | `PATCH /files/{fileId}/permissions/{permissionId}` |

```rust
pub trait GooglePermissionsApi {
    fn permissions_create(&self, session: &GoogleSession, file_id: &FileId, request: GooglePermissionWrite, options: PermissionCreateOptions) -> GoogleResult<GooglePermission>;
    fn permissions_delete(&self, session: &GoogleSession, file_id: &FileId, permission_id: &PermissionId, options: PermissionDeleteOptions) -> GoogleResult<()>;
    fn permissions_get(&self, session: &GoogleSession, file_id: &FileId, permission_id: &PermissionId, options: PermissionGetOptions) -> GoogleResult<GooglePermission>;
    fn permissions_list(&self, session: &GoogleSession, file_id: &FileId, options: PermissionListOptions) -> GoogleResult<GooglePermissionList>;
    fn permissions_update(&self, session: &GoogleSession, file_id: &FileId, permission_id: &PermissionId, request: GooglePermissionWrite, options: PermissionUpdateOptions) -> GoogleResult<GooglePermission>;
}
```

### 3.12 replies (5 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `replies_create` | `file_id: &FileId, comment_id: &CommentId, request: GoogleReplyWrite, options: ReplyOptions` | `GoogleResult<GoogleReply>` | `POST /files/{fileId}/comments/{commentId}/replies` |
| `replies_delete` | `file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId` | `GoogleResult<()>` | `DELETE /files/{fileId}/comments/{commentId}/replies/{replyId}` |
| `replies_get` | `file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId, options: ReplyOptions` | `GoogleResult<GoogleReply>` | `GET /files/{fileId}/comments/{commentId}/replies/{replyId}` |
| `replies_list` | `file_id: &FileId, comment_id: &CommentId, options: ReplyListOptions` | `GoogleResult<GoogleReplyList>` | `GET /files/{fileId}/comments/{commentId}/replies` |
| `replies_update` | `file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId, request: GoogleReplyWrite, options: ReplyOptions` | `GoogleResult<GoogleReply>` | `PATCH /files/{fileId}/comments/{commentId}/replies/{replyId}` |

```rust
pub trait GoogleRepliesApi {
    fn replies_create(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, request: GoogleReplyWrite, options: ReplyOptions) -> GoogleResult<GoogleReply>;
    fn replies_delete(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId) -> GoogleResult<()>;
    fn replies_get(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId, options: ReplyOptions) -> GoogleResult<GoogleReply>;
    fn replies_list(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, options: ReplyListOptions) -> GoogleResult<GoogleReplyList>;
    fn replies_update(&self, session: &GoogleSession, file_id: &FileId, comment_id: &CommentId, reply_id: &ReplyId, request: GoogleReplyWrite, options: ReplyOptions) -> GoogleResult<GoogleReply>;
}
```

### 3.13 revisions (4 métodos)

| Método | Entradas além de `session` | Saída | REST |
| --- | --- | --- | --- |
| `revisions_delete` | `file_id: &FileId, revision_id: &RevisionId` | `GoogleResult<()>` | `DELETE /files/{fileId}/revisions/{revisionId}` |
| `revisions_get` | `file_id: &FileId, revision_id: &RevisionId, options: RevisionGetOptions` | `GoogleResult<GoogleRevision>` | `GET /files/{fileId}/revisions/{revisionId}` |
| `revisions_list` | `file_id: &FileId, options: RevisionListOptions` | `GoogleResult<GoogleRevisionList>` | `GET /files/{fileId}/revisions` |
| `revisions_update` | `file_id: &FileId, revision_id: &RevisionId, request: GoogleRevisionWrite` | `GoogleResult<GoogleRevision>` | `PATCH /files/{fileId}/revisions/{revisionId}` |

```rust
pub trait GoogleRevisionsApi {
    fn revisions_delete(&self, session: &GoogleSession, file_id: &FileId, revision_id: &RevisionId) -> GoogleResult<()>;
    fn revisions_get(&self, session: &GoogleSession, file_id: &FileId, revision_id: &RevisionId, options: RevisionGetOptions) -> GoogleResult<GoogleRevision>;
    fn revisions_list(&self, session: &GoogleSession, file_id: &FileId, options: RevisionListOptions) -> GoogleResult<GoogleRevisionList>;
    fn revisions_update(&self, session: &GoogleSession, file_id: &FileId, revision_id: &RevisionId, request: GoogleRevisionWrite) -> GoogleResult<GoogleRevision>;
}
```

## 4. OAuth 2.0 / OpenID Connect — assinaturas públicas

```rust
pub trait GoogleOAuthApi {
    fn load_google_config(&self, sources: GoogleConfigSources) -> GoogleResult<GoogleOAuthConfig>;
    fn authorization_request(&self, config: &GoogleOAuthConfig, request: GoogleAuthorizationRequest) -> GoogleResult<GoogleAuthorizationUrl>;
    fn exchange_authorization_code(&self, config: &GoogleOAuthConfig, code: &str, redirect_uri: &str, pkce_verifier: &str) -> GoogleResult<GoogleTokenResponse>;
    fn refresh_access_token(&self, config: &GoogleOAuthConfig, refresh_token: &str) -> GoogleResult<GoogleTokenResponse>;
    fn revoke_token(&self, token: &str) -> GoogleResult<()>;
    fn get_userinfo(&self, session: &GoogleSession) -> GoogleResult<GoogleUserInfo>;
    fn validate_id_token(&self, id_token: &str, validation: GoogleIdTokenValidation) -> GoogleResult<GoogleIdClaims>;
    fn get_oidc_discovery(&self) -> GoogleResult<GoogleOidcDiscovery>;
    fn get_oidc_keys(&self) -> GoogleResult<GoogleJwkSet>;
}
```

| Tipo | Campos essenciais |
| --- | --- |
| `GoogleConfigSources` | Overrides de ambiente, caminho de arquivo de configuração e configuração empacotada, com precedência definida |
| `GoogleOAuthConfig` | `client_id`, `client_secret?`, `redirect_uris`, `scopes` |
| `GoogleAuthorizationRequest` | `redirect_uri`, `state`, `code_challenge`, `code_challenge_method=S256`, `scopes`, `access_type`, `prompt?` |
| `GoogleAuthorizationUrl` | URL completa de autorização |
| `GoogleTokenResponse` | `access_token`, `token_type`, `expires_in`, `refresh_token?`, `scope?`, `id_token?` — **segredos** |
| `GoogleUserInfo` | `sub`, `email?`, `email_verified?`, `name?`, `picture?`, `hd?` |
| `GoogleIdTokenValidation` | `issuer`, `audience`, `nonce?`, validade temporal e tolerância de relógio |
| `GoogleIdClaims` | `iss`, `aud`, `sub`, `exp`, `iat`, `email?`, `email_verified?`, `nonce?` |
| `GoogleOidcDiscovery` | Endpoints de autorização, token, UserInfo, revogação e chaves públicas |
| `GoogleJwkSet` | Chaves públicas de verificação e respectivos identificadores |

| Endpoint oficial | Operação |
| --- | --- |
| `https://accounts.google.com/o/oauth2/v2/auth` | Autorizar no navegador |
| `POST https://oauth2.googleapis.com/token` | Trocar código / renovar token |
| `POST https://oauth2.googleapis.com/revoke` | Revogar token |
| `GET https://openidconnect.googleapis.com/v1/userinfo` | Consultar identidade |
| `GET https://accounts.google.com/.well-known/openid-configuration` | Descobrir endpoints OIDC e URL das chaves |

A API fornece as operações específicas do protocolo OAuth/OIDC Google. Não mantém cadastro de contas nem define persistência de tokens; credenciais e contexto de sessão são dados técnicos dos métodos que os requerem.

## 5. Transferências e conveniências públicas adicionais

Essas interfaces representam capacidades necessárias do protocolo Drive que **não são métodos REST adicionais**. Mantê-las distintas evita confundir bytes, metadados, exportação e operações longas.

```rust
pub trait GoogleTransferApi {
    fn get_file_bytes(&self, session: &GoogleSession, file_id: &FileId, range: Option<GoogleByteRange>) -> GoogleResult<GoogleDownloadStream>;
    fn get_revision_bytes(&self, session: &GoogleSession, file_id: &FileId, revision_id: &RevisionId, range: Option<GoogleByteRange>) -> GoogleResult<GoogleDownloadStream>;
    fn export_file_bytes(&self, session: &GoogleSession, file_id: &FileId, mime_type: &MimeType) -> GoogleResult<GoogleDownloadStream>;
    fn start_resumable_create(&self, session: &GoogleSession, request: GoogleFileWrite, options: FileCreateOptions, total_bytes: Option<u64>) -> GoogleResult<GoogleUploadSession>;
    fn start_resumable_update(&self, session: &GoogleSession, file_id: &FileId, request: GoogleFileWrite, options: FileUpdateOptions, total_bytes: Option<u64>) -> GoogleResult<GoogleUploadSession>;
    fn upload_chunk(&self, session: &GoogleSession, upload: &GoogleUploadSession, chunk: GoogleUploadChunk) -> GoogleResult<GoogleUploadProgress>;
    fn query_upload_progress(&self, session: &GoogleSession, upload: &GoogleUploadSession) -> GoogleResult<GoogleUploadProgress>;
    fn trash_file(&self, session: &GoogleSession, file_id: &FileId) -> GoogleResult<GoogleFile>;
    fn rename_file(&self, session: &GoogleSession, file_id: &FileId, name: &str) -> GoogleResult<GoogleFile>;
    fn move_file(&self, session: &GoogleSession, file_id: &FileId, add_parent: &FileId, remove_parents: &[FileId]) -> GoogleResult<GoogleFile>;
}
```

| Tipo | Campos e semântica |
| --- | --- |
| `GoogleByteRange` | `start: u64`, `end_inclusive: Option<u64>` |
| `GoogleDownloadStream` | Fluxo de bytes, status HTTP, `content_length?`, `content_range?`, `etag?`, `mime_type?` |
| `GoogleUploadSession` | Identificador/URI opaca de sessão, tamanho total opcional; deve ser tratada como informação sensível |
| `GoogleUploadChunk` | Offset inicial, bytes e total esperado que determina o bloco final |
| `GoogleUploadProgress` | `Incomplete { received_range }` ou `Complete { file: GoogleFile }`; `308` significa **incompleto** |
| `GoogleOperation` | `done=false` não equivale a transferência concluída; usar `operations_get` para acompanhar |

| Operação | Endpoint / comportamento |
| --- | --- |
| `get_file_bytes` | `GET /files/{fileId}?alt=media`, resposta binária |
| `get_revision_bytes` | `GET /files/{fileId}/revisions/{revisionId}?alt=media`, quando suportado |
| `export_file_bytes` | `GET /files/{fileId}/export?mimeType=…`, resposta binária |
| `start_resumable_create` | `POST /upload/drive/v3/files?uploadType=resumable` |
| `start_resumable_update` | `PATCH /upload/drive/v3/files/{fileId}?uploadType=resumable` |
| `upload_chunk`, `query_upload_progress` | URI opaca da sessão de upload, `PUT` com `Content-Range`; retorno `308` incompleto ou sucesso final |
| `trash_file` | `files.update`, `trashed=true`; **não é** `files.delete` |
| `rename_file` | `files.update`, alteração de `name` |
| `move_file` | `files.update`, `addParents` / `removeParents`, sem reupload |

Referências: [download e exportação](https://developers.google.com/workspace/drive/api/guides/manage-downloads), [upload](https://developers.google.com/workspace/drive/api/guides/manage-uploads).

## 6. Contrato funcional de cada método público

**Guia de leitura sem alterações contratuais.** Esta seção reapresenta os **59 métodos REST, 9 operações OAuth/OIDC e 10 métodos de transferência/conveniência** com finalidade, parâmetros, retorno e limites. Todos os blocos de assinatura Rust, nomes, tipos, campos e regras das seções anteriores e das declarações posteriores **permanecem inalterados**. A documentação detalhada de `XxxOptions`, `XxxRequest` e outros tipos é a fonte de verdade sobre os respectivos campos opcionais.

### 6.1 Drive REST v3 — 59 métodos

#### `about_get`

**Finalidade:** Consulta a conta Google Drive por meio de `GET /about`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: AboutOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleAbout>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `access_proposals_get`

**Finalidade:** Consulta as solicitações de acesso por meio de `GET /files/{fileId}/accessproposals/{proposalId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `proposal_id: &ProposalId``: identidade da proposta de acesso.

**Retorno:** `GoogleResult<GoogleAccessProposal>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `access_proposals_list`

**Finalidade:** Lista as solicitações de acesso por meio de `GET /files/{fileId}/accessproposals`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: PageOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleAccessProposalList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `access_proposals_resolve`

**Finalidade:** Resolve as solicitações de acesso por meio de `POST /files/{fileId}/accessproposals/{proposalId}:resolve`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `proposal_id: &ProposalId`: identidade da proposta de acesso; `request: ResolveAccessProposalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_approve`

**Finalidade:** Aprova as aprovações por meio de `POST /files/{fileId}/approvals/{approvalId}:approve`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId`: identidade da aprovação; `request: ApproveApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_cancel`

**Finalidade:** Cancela as aprovações por meio de `POST /files/{fileId}/approvals/{approvalId}:cancel`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId`: identidade da aprovação; `request: CancelApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_comment`

**Finalidade:** Comenta as aprovações por meio de `POST /files/{fileId}/approvals/{approvalId}:comment`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId`: identidade da aprovação; `request: CommentApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_decline`

**Finalidade:** Recusa as aprovações por meio de `POST /files/{fileId}/approvals/{approvalId}:decline`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId`: identidade da aprovação; `request: DeclineApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_get`

**Finalidade:** Consulta as aprovações por meio de `GET /files/{fileId}/approvals/{approvalId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId``: identidade da aprovação.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_list`

**Finalidade:** Lista as aprovações por meio de `GET /files/{fileId}/approvals`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: PageOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleApprovalList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_reassign`

**Finalidade:** Reatribui as aprovações por meio de `POST /files/{fileId}/approvals/{approvalId}:reassign`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `approval_id: &ApprovalId`: identidade da aprovação; `request: ReassignApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `approvals_start`

**Finalidade:** Inicia as aprovações por meio de `POST /files/{fileId}/approvals:start`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: StartApprovalRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleApproval>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `apps_get`

**Finalidade:** Consulta os aplicativos por meio de `GET /apps/{appId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``app_id: &AppId``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleApp>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `apps_list`

**Finalidade:** Lista os aplicativos por meio de `GET /apps`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: AppListOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleAppList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `changes_get_start_page_token`

**Finalidade:** Obtém cursor inicial de as alterações por meio de `GET /changes/startPageToken`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: ChangeScopeOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleStartPageToken>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `changes_list`

**Finalidade:** Lista as alterações por meio de `GET /changes`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: ChangeListOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleChangeList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `changes_watch`

**Finalidade:** Solicita notificações para as alterações por meio de `POST /changes/watch`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: ChangeWatchOptions`: valor obrigatório pelo contrato tipado; `channel: GoogleChannelRequest``: objeto de canal de notificação.

**Retorno:** `GoogleResult<GoogleChannel>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não consome nem reconhece as notificações posteriores. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `channels_stop`

**Finalidade:** Interrompe os canais de notificação por meio de `POST /channels/stop`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``channel: GoogleChannelStopRequest``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `comments_create`

**Finalidade:** Cria os comentários por meio de `POST /files/{fileId}/comments`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: GoogleCommentWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: CommentOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleComment>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `comments_delete`

**Finalidade:** Exclui os comentários por meio de `DELETE /files/{fileId}/comments/{commentId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId``: identidade do comentário.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `comments_get`

**Finalidade:** Consulta os comentários por meio de `GET /files/{fileId}/comments/{commentId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `options: CommentOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleComment>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `comments_list`

**Finalidade:** Lista os comentários por meio de `GET /files/{fileId}/comments`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: CommentListOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleCommentList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `comments_update`

**Finalidade:** Atualiza os comentários por meio de `PATCH /files/{fileId}/comments/{commentId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `request: GoogleCommentWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: CommentOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleComment>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_create`

**Finalidade:** Cria os drives compartilhados por meio de `POST /drives`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``request_id: &RequestId`: valor obrigatório pelo contrato tipado; `request: GoogleDriveWrite``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleDrive>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_delete`

**Finalidade:** Exclui os drives compartilhados por meio de `DELETE /drives/{driveId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``drive_id: &DriveId`: valor obrigatório pelo contrato tipado; `options: DriveDeleteOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_get`

**Finalidade:** Consulta os drives compartilhados por meio de `GET /drives/{driveId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``drive_id: &DriveId`: valor obrigatório pelo contrato tipado; `options: DriveGetOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleDrive>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_hide`

**Finalidade:** Oculta os drives compartilhados por meio de `POST /drives/{driveId}/hide`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``drive_id: &DriveId``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleDrive>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_list`

**Finalidade:** Lista os drives compartilhados por meio de `GET /drives`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: DriveListOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleDriveList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_unhide`

**Finalidade:** Exibe novamente os drives compartilhados por meio de `POST /drives/{driveId}/unhide`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``drive_id: &DriveId``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleDrive>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `drives_update`

**Finalidade:** Atualiza os drives compartilhados por meio de `PATCH /drives/{driveId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``drive_id: &DriveId`: valor obrigatório pelo contrato tipado; `request: GoogleDriveWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: DriveUpdateOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleDrive>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_copy`

**Finalidade:** Copia os arquivos por meio de `POST /files/{fileId}/copy`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: GoogleFileWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: FileCopyOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_create`

**Finalidade:** Cria os arquivos por meio de `POST /files`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``request: GoogleFileWrite`: valor obrigatório pelo contrato tipado; `options: FileCreateOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_delete`

**Finalidade:** Exclui os arquivos por meio de `DELETE /files/{fileId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: FileDeleteOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_download`

**Finalidade:** Solicita download de os arquivos por meio de `POST /files/{fileId}/download`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: FileDownloadOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleOperation>`. Operação possivelmente assíncrona; `done=false` não confirma conclusão.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_empty_trash`

**Finalidade:** Esvazia a lixeira de os arquivos por meio de `DELETE /files/trash`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: EmptyTrashOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_export`

**Finalidade:** Exporta os arquivos por meio de `GET /files/{fileId}/export`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `mime_type: &MimeType``: formato MIME solicitado.

**Retorno:** `GoogleResult<GoogleByteStream>`. Fluxo de bytes, cuja leitura deve ser completada e verificada.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_generate_cse_token`

**Finalidade:** Obtém token CSE para os arquivos por meio de `GET /files/generateCseToken`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: CseTokenOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleCseToken>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_generate_ids`

**Finalidade:** Obtém identificadores para os arquivos por meio de `GET /files/generateIds`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: GenerateIdsOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleGeneratedIds>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_get`

**Finalidade:** Consulta os arquivos por meio de `GET /files/{fileId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: FileGetOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_list`

**Finalidade:** Lista os arquivos por meio de `GET /files`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``options: FileListOptions``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleFileList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_list_labels`

**Finalidade:** Lista rótulos de os arquivos por meio de `GET /files/{fileId}/listLabels`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: LabelListOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleLabelList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_modify_labels`

**Finalidade:** Altera rótulos de os arquivos por meio de `POST /files/{fileId}/modifyLabels`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: GoogleModifyLabelsRequest``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleModifyLabelsResponse>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_update`

**Finalidade:** Atualiza os arquivos por meio de `PATCH /files/{fileId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: GoogleFileWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: FileUpdateOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `files_watch`

**Finalidade:** Solicita notificações para os arquivos por meio de `POST /files/{fileId}/watch`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: FileWatchOptions`: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo; `channel: GoogleChannelRequest``: objeto de canal de notificação.

**Retorno:** `GoogleResult<GoogleChannel>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não consome nem reconhece as notificações posteriores. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `operations_get`

**Finalidade:** Consulta as operações assíncronas por meio de `GET /operations/{name}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``name: &OperationName``: valor obrigatório pelo contrato tipado.

**Retorno:** `GoogleResult<GoogleOperation>`. Operação possivelmente assíncrona; `done=false` não confirma conclusão.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `permissions_create`

**Finalidade:** Cria as permissões por meio de `POST /files/{fileId}/permissions`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `request: GooglePermissionWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: PermissionCreateOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GooglePermission>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `permissions_delete`

**Finalidade:** Exclui as permissões por meio de `DELETE /files/{fileId}/permissions/{permissionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `permission_id: &PermissionId`: identidade da permissão; `options: PermissionDeleteOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `permissions_get`

**Finalidade:** Consulta as permissões por meio de `GET /files/{fileId}/permissions/{permissionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `permission_id: &PermissionId`: identidade da permissão; `options: PermissionGetOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GooglePermission>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `permissions_list`

**Finalidade:** Lista as permissões por meio de `GET /files/{fileId}/permissions`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: PermissionListOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GooglePermissionList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `permissions_update`

**Finalidade:** Atualiza as permissões por meio de `PATCH /files/{fileId}/permissions/{permissionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `permission_id: &PermissionId`: identidade da permissão; `request: GooglePermissionWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: PermissionUpdateOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GooglePermission>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `replies_create`

**Finalidade:** Cria as respostas aos comentários por meio de `POST /files/{fileId}/comments/{commentId}/replies`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `request: GoogleReplyWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: ReplyOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleReply>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `replies_delete`

**Finalidade:** Exclui as respostas aos comentários por meio de `DELETE /files/{fileId}/comments/{commentId}/replies/{replyId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `reply_id: &ReplyId``: identidade da resposta.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `replies_get`

**Finalidade:** Consulta as respostas aos comentários por meio de `GET /files/{fileId}/comments/{commentId}/replies/{replyId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `reply_id: &ReplyId`: identidade da resposta; `options: ReplyOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleReply>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `replies_list`

**Finalidade:** Lista as respostas aos comentários por meio de `GET /files/{fileId}/comments/{commentId}/replies`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `options: ReplyListOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleReplyList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `replies_update`

**Finalidade:** Atualiza as respostas aos comentários por meio de `PATCH /files/{fileId}/comments/{commentId}/replies/{replyId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `comment_id: &CommentId`: identidade do comentário; `reply_id: &ReplyId`: identidade da resposta; `request: GoogleReplyWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: ReplyOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleReply>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `revisions_delete`

**Finalidade:** Exclui as revisões por meio de `DELETE /files/{fileId}/revisions/{revisionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `revision_id: &RevisionId``: identidade da revisão.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `revisions_get`

**Finalidade:** Consulta as revisões por meio de `GET /files/{fileId}/revisions/{revisionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `revision_id: &RevisionId`: identidade da revisão; `options: RevisionGetOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleRevision>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `revisions_list`

**Finalidade:** Lista as revisões por meio de `GET /files/{fileId}/revisions`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `options: RevisionListOptions``: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo.

**Retorno:** `GoogleResult<GoogleRevisionList>`. Coleção de recursos, possivelmente paginada; processar cursores.

**Limites e erros:** Não percorre todas as páginas automaticamente. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

#### `revisions_update`

**Finalidade:** Atualiza as revisões por meio de `PATCH /files/{fileId}/revisions/{revisionId}`.

**Parâmetros:** `session: &GoogleSession` seleciona a conta autenticada. ``file_id: &FileId`: valor obrigatório pelo contrato tipado; `revision_id: &RevisionId`: identidade da revisão; `request: GoogleRevisionWrite``: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleRevision>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Falhas seguem `GoogleApiFailure`; a obrigatoriedade e semântica dos atributos internos continuam definidas nos tipos do contrato.

### 6.2 OAuth/OpenID Connect — 9 métodos

#### `load_google_config`

**Finalidade:** Carrega a configuração OAuth das fontes definidas.

**Parâmetros:** `sources: GoogleConfigSources`: fontes e precedência de configuração.

**Retorno:** `GoogleResult<GoogleOAuthConfig>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `authorization_request`

**Finalidade:** Prepara URL de autorização com os parâmetros OAuth e PKCE.

**Parâmetros:** `config: &GoogleOAuthConfig`: configuração OAuth; `request: GoogleAuthorizationRequest`: objeto de dados enviados/atualizados; ausência não significa limpeza.

**Retorno:** `GoogleResult<GoogleAuthorizationUrl>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `exchange_authorization_code`

**Finalidade:** Troca código de autorização por tokens com verificação PKCE.

**Parâmetros:** `config: &GoogleOAuthConfig`: configuração OAuth; `code: &str`: código OAuth recebido; `redirect_uri: &str`: URI de redirecionamento OAuth; `pkce_verifier: &str`: verificador PKCE.

**Retorno:** `GoogleResult<GoogleTokenResponse>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `refresh_access_token`

**Finalidade:** Renova o acesso com refresh token.

**Parâmetros:** `config: &GoogleOAuthConfig`: configuração OAuth; `refresh_token: &str`: credencial de renovação.

**Retorno:** `GoogleResult<GoogleTokenResponse>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `revoke_token`

**Finalidade:** Solicita a revogação do token informado.

**Parâmetros:** `token: &str`: token submetido para revogação.

**Retorno:** `GoogleResult<()>`. Confirmação sem objeto de recurso; erros continuam tipados.

**Limites e erros:** Pode alterar ou remover dados; não assumir reversibilidade. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `get_userinfo`

**Finalidade:** Consulta os dados OIDC da conta autenticada.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada.

**Retorno:** `GoogleResult<GoogleUserInfo>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `validate_id_token`

**Finalidade:** Verifica o ID token conforme os critérios de validação fornecidos.

**Parâmetros:** `id_token: &str`: ID token a validar; `validation: GoogleIdTokenValidation`: critérios de validação.

**Retorno:** `GoogleResult<GoogleIdClaims>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `get_oidc_discovery`

**Finalidade:** Obtém o documento de descoberta OIDC.

**Parâmetros:** Nenhum parâmetro adicional..

**Retorno:** `GoogleResult<GoogleOidcDiscovery>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `get_oidc_keys`

**Finalidade:** Obtém as chaves públicas de validação OIDC.

**Parâmetros:** Nenhum parâmetro adicional..

**Retorno:** `GoogleResult<GoogleJwkSet>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

### 6.3 Transferências e conveniências — 10 métodos

#### `get_file_bytes`

**Finalidade:** Lê os bytes de um arquivo regular.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `range: Option<GoogleByteRange>`: intervalo de bytes opcional; ausente significa solicitação integral.

**Retorno:** `GoogleResult<GoogleDownloadStream>`. Fluxo de bytes, cuja leitura deve ser completada e verificada.

**Limites e erros:** Não confundir etapa parcial de transferência com confirmação final. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `get_revision_bytes`

**Finalidade:** Lê os bytes de uma revisão disponível.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `revision_id: &RevisionId`: identidade da revisão; `range: Option<GoogleByteRange>`: intervalo de bytes opcional; ausente significa solicitação integral.

**Retorno:** `GoogleResult<GoogleDownloadStream>`. Fluxo de bytes, cuja leitura deve ser completada e verificada.

**Limites e erros:** Não confundir etapa parcial de transferência com confirmação final. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `export_file_bytes`

**Finalidade:** Exporta conteúdo Google Workspace no formato solicitado.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `mime_type: &MimeType`: formato MIME solicitado.

**Retorno:** `GoogleResult<GoogleDownloadStream>`. Fluxo de bytes, cuja leitura deve ser completada e verificada.

**Limites e erros:** Não confundir etapa parcial de transferência com confirmação final. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `start_resumable_create`

**Finalidade:** Inicia sessão de upload retomável para criação.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `request: GoogleFileWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: FileCreateOptions`: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo; `total_bytes: Option<u64>`: tamanho total opcional.

**Retorno:** `GoogleResult<GoogleUploadSession>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `start_resumable_update`

**Finalidade:** Inicia sessão de upload retomável para atualização.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `request: GoogleFileWrite`: objeto de dados enviados/atualizados; ausência não significa limpeza; `options: FileUpdateOptions`: objeto de opções desse endpoint; campos opcionais definidos no próprio tipo; `total_bytes: Option<u64>`: tamanho total opcional.

**Retorno:** `GoogleResult<GoogleUploadSession>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `upload_chunk`

**Finalidade:** Transmite um bloco de upload à sessão.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `upload: &GoogleUploadSession`: sessão de upload existente; `chunk: GoogleUploadChunk`: bloco, offset e estado do envio.

**Retorno:** `GoogleResult<GoogleUploadProgress>`. Progresso parcial ou conclusão; HTTP 308 não representa sucesso final.

**Limites e erros:** Não confundir etapa parcial de transferência com confirmação final. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `query_upload_progress`

**Finalidade:** Consulta a posição confirmada da sessão de upload.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `upload: &GoogleUploadSession`: sessão de upload existente.

**Retorno:** `GoogleResult<GoogleUploadProgress>`. Progresso parcial ou conclusão; HTTP 308 não representa sucesso final.

**Limites e erros:** Não confundir etapa parcial de transferência com confirmação final. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `trash_file`

**Finalidade:** Move arquivo para lixeira, sem exclusão permanente.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `rename_file`

**Finalidade:** Renomeia arquivo sem modificar conteúdo.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `name: &str`: nome da operação remota.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

#### `move_file`

**Finalidade:** Move arquivo por alterações de pais sem reupload.

**Parâmetros:** `session: &GoogleSession`: sessão autenticada e conta selecionada; `file_id: &FileId`: identidade do arquivo; `add_parent: &FileId`: novo diretório pai; `remove_parents: &[FileId]`: diretórios pais a remover.

**Retorno:** `GoogleResult<GoogleFile>`. Objeto tipado retornado pelo serviço, respeitando campos disponibilizados.

**Limites e erros:** Não executa etapas adicionais não descritas nem dispensa permissões e validações. Respeita exatamente os objetos, validações e categorias de erro já especificados; não redefine o contrato.

## 7. Critérios de teste independentes

| Grupo | Casos mínimos de contrato |
| --- | --- |
| OAuth / OIDC | PKCE e `state`; token emitido; refresh sem novo refresh token; erro de consentimento; `sub` imutável; JWT inválido |
| About / Apps | Máscara de campos, lista de apps e metadados indisponíveis |
| Files | Criar, copiar, atualizar, mover, renomear, lixeira, excluir permanentemente, listar com filtros; 404 não significa exclusão comprovada |
| Changes | Cursor inicial, paginação, `newStartPageToken` somente no final; `removed=true` também pode indicar perda de acesso |
| Drives | Criação, listagem paginada, atualização, ocultação e exclusão autorizada |
| Permissions / Proposals | Criação, atualização, listagem, rejeição e direitos insuficientes |
| Comments / Replies | CRUD, paginação, recursos removidos e escopos insuficientes |
| Revisions | Leitura, listagem, atualização, exclusão e bytes de revisão quando suportados |
| Approvals | Todos os oito comandos, recusas, reatribuição e restrições do provedor |
| Labels / CSE | Rótulos, alterações válidas, formatos e erros de criptografia |
| Watch / Channels | Criação de canal, notificação, expiração e parada |
| Bytes / Upload | `alt=media` entrega bytes; exportação e download assíncrono são diferentes; `308` não finaliza; sessão vencida; ranges |
| HTTP / Segurança | 400, 401, 403 por `reason`, 404, 410, 429, 5xx, timeout, redirecionamento, limites, sanitização de erros e segredos |

**Estratégia TDD:** primeiro implementar fakes do contrato para testar os consumidores; depois testar a implementação Google API contra servidor HTTP simulado com requisições e respostas oficiais; por fim validar integração real opcional. O contrato **não depende** do estado atual da engine.

## 8. Condições de validação TDD

| Verificação | Critério |
| --- | --- |
| Assinaturas | Cada método da seção 3 tem entrada e saída tipadas, sem dependência de código existente |
| Tipos | Os nomes utilizados pelas assinaturas possuem declaração pública no documento, inclusive tipos aninhados |
| Disponibilidade | Campos opcionais e máscaras de seleção não se confundem com ausência confirmada |
| Erros | Reações da engine são guiadas por categorias de erro e critérios de retry, nunca por mensagens textuais cruas |
| Implementação | Contratos podem ser simulados antes de implementar HTTP e usados em testes TDD |

**Limite de escopo:** a referência é Google Drive REST v3 e protocolos Google OAuth/OIDC. Demais APIs Google são contratos separados. As estruturas abaixo são **projeções Rust propostas** a partir dos recursos oficiais; a correspondência exata de campos e subtipos com o Discovery deve ser verificada antes de considerar o documento um espelho integral de todo o esquema JSON oficial.

## 9. Declarações dos tipos públicos

**Referência de modelagem:** [Google Drive API v3 — recursos](https://developers.google.com/workspace/drive/api/reference/rest/v3), [Discovery](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest). Esta seção reúne os tipos **de resposta e de escrita** em declarações Rust. `Option<T>` indica que o campo pode não estar presente na resposta recebida; não representa, por si só, uma declaração de ausência do objeto no Drive.

**Tipos numéricos recebidos como strings:** o Drive representa vários `int64` JSON como string (por exemplo `size`, `version`, `quotaBytesUsed`). Mantêm-se como `String` nos tipos de transporte para evitar perda de precisão, com conversão tipada e validada quando necessária. Datas são strings RFC 3339 no transporte.

**Atributos sensíveis ou efêmeros:** links de download, `resourceKey`, JWT de CSE e URIs de sessão devem receber tratamento restrito, sem logs nem persistência inadvertida.

### 9.1 Estruturas de saída e metadados

```rust
pub struct GoogleAbout {
    pub user: Option<GoogleUser>,
    pub storage_quota: Option<GoogleStorageQuota>,
    pub import_formats: Option<std::collections::BTreeMap<String,Vec<String>>>,
    pub export_formats: Option<std::collections::BTreeMap<String,Vec<String>>>,
    pub max_import_sizes: Option<std::collections::BTreeMap<String,String>>,
    pub app_installed: Option<bool>,
    pub folder_color_palette: Option<Vec<String>>,
}

pub struct GoogleUser {
    pub display_name: Option<String>,
    pub email_address: Option<String>,
    pub photo_link: Option<String>,
    pub permission_id: Option<String>,
    pub me: Option<bool>,
}

pub struct GoogleStorageQuota {
    pub limit: Option<String>,
    pub usage: Option<String>,
    pub usage_in_drive: Option<String>,
    pub usage_in_drive_trash: Option<String>,
}

pub struct GoogleFile {
    pub id: Option<FileId>,
    pub name: Option<String>,
    pub mime_type: Option<MimeType>,
    pub parents: Option<Vec<FileId>>,
    pub description: Option<String>,
    pub trashed: Option<bool>,
    pub explicitly_trashed: Option<bool>,
    pub trashed_time: Option<String>,
    pub version: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub modified_by_me_time: Option<String>,
    pub viewed_by_me_time: Option<String>,
    pub size: Option<String>,
    pub quota_bytes_used: Option<String>,
    pub md5_checksum: Option<String>,
    pub sha1_checksum: Option<String>,
    pub sha256_checksum: Option<String>,
    pub head_revision_id: Option<RevisionId>,
    pub drive_id: Option<DriveId>,
    pub resource_key: Option<String>,
    pub web_view_link: Option<String>,
    pub web_content_link: Option<String>,
    pub thumbnail_link: Option<String>,
    pub icon_link: Option<String>,
    pub file_extension: Option<String>,
    pub full_file_extension: Option<String>,
    pub original_filename: Option<String>,
    pub shortcut_details: Option<GoogleShortcutDetails>,
    pub capabilities: Option<GoogleFileCapabilities>,
    pub permissions: Option<Vec<GooglePermission>>,
    pub owners: Option<Vec<GoogleUser>>,
    pub last_modifying_user: Option<GoogleUser>,
    pub sharing_user: Option<GoogleUser>,
    pub shared: Option<bool>,
    pub owned_by_me: Option<bool>,
    pub modified_by_me: Option<bool>,
    pub starred: Option<bool>,
    pub folder_color_rgb: Option<String>,
    pub properties: Option<std::collections::BTreeMap<String,String>>,
    pub app_properties: Option<std::collections::BTreeMap<String,String>>,
    pub content_hints: Option<GoogleContentHints>,
    pub image_media_metadata: Option<GoogleImageMediaMetadata>,
    pub video_media_metadata: Option<GoogleVideoMediaMetadata>,
    pub label_info: Option<GoogleLabelInfo>,
    pub content_restrictions: Option<Vec<GoogleContentRestriction>>,
    pub download_restrictions: Option<GoogleDownloadRestrictions>,
    pub client_encryption_details: Option<GoogleClientEncryptionDetails>,
    pub copy_requires_writer_permission: Option<bool>,
    pub writers_can_share: Option<bool>,
    pub has_thumbnail: Option<bool>,
    pub spaces: Option<Vec<String>>,
    pub permission_ids: Option<Vec<PermissionId>>,
    pub team_drive_id: Option<String>,
    pub inherited_permissions_disabled: Option<bool>,
}

pub struct GoogleShortcutDetails {
    pub target_id: Option<FileId>,
    pub target_mime_type: Option<MimeType>,
    pub target_resource_key: Option<String>,
}

pub struct GoogleFileCapabilities {
    pub can_edit: Option<bool>,
    pub can_copy: Option<bool>,
    pub can_download: Option<bool>,
    pub can_delete: Option<bool>,
    pub can_rename: Option<bool>,
    pub can_move_item_within_drive: Option<bool>,
    pub can_move_item_out_of_drive: Option<bool>,
    pub can_add_children: Option<bool>,
    pub can_list_children: Option<bool>,
    pub can_share: Option<bool>,
    pub can_comment: Option<bool>,
    pub can_read_revisions: Option<bool>,
    pub can_change_copy_requires_writer_permission: Option<bool>,
}

pub struct GoogleContentHints {
    pub indexable_text: Option<String>,
    pub thumbnail: Option<GoogleThumbnail>,
}

pub struct GoogleThumbnail {
    pub image: Option<String>,
    pub mime_type: Option<String>,
}

pub struct GoogleImageMediaMetadata {
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub rotation: Option<i32>,
    pub time: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub exposure_time: Option<f64>,
    pub aperture: Option<f64>,
    pub iso_speed: Option<i32>,
    pub location: Option<GoogleGeoLocation>,
}

pub struct GoogleGeoLocation {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
}

pub struct GoogleVideoMediaMetadata {
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_millis: Option<String>,
}

pub struct GoogleContentRestriction {
    pub read_only: Option<bool>,
    pub reason: Option<String>,
    pub restricting_user: Option<GoogleUser>,
    pub restriction_time: Option<String>,
    pub owner_restricted: Option<bool>,
    pub system_restricted: Option<bool>,
}

pub struct GoogleDownloadRestrictions {
    pub item_download_restriction: Option<GoogleDownloadRestriction>,
    /// JSON `effectiveDownloadRestrictionWithContext` (Discovery v3).
    pub effective_download_restriction_with_context: Option<GoogleDownloadRestriction>,
}

pub struct GoogleDownloadRestriction {
    pub restricted_for_readers: Option<bool>,
    pub restricted_for_writers: Option<bool>,
}

pub struct GoogleClientEncryptionDetails {
    pub encryption_state: Option<String>,
    pub decryption_metadata: Option<GoogleDecryptionMetadata>,
}

pub struct GoogleDecryptionMetadata {
    pub wrapped_key: Option<String>,
    pub kacls_id: Option<String>,
}

pub struct GoogleLabelInfo {
    pub labels: Option<Vec<GoogleLabel>>,
}

pub struct GoogleLabel {
    pub id: Option<String>,
    pub revision_id: Option<String>,
    pub fields: Option<std::collections::BTreeMap<String,GoogleLabelField>>,
}

pub struct GoogleLabelField {
    pub value_type: Option<String>,
    pub text: Option<Vec<String>>,
    pub integer: Option<Vec<String>>,
    pub date_string: Option<Vec<String>>,
    pub selection: Option<Vec<String>>,
    pub user: Option<Vec<GoogleUser>>,
}

pub struct GoogleFileList {
    pub files: Vec<GoogleFile>,
    pub next_page_token: Option<String>,
    pub incomplete_search: Option<bool>,
}

pub struct GoogleChange {
    pub file_id: Option<FileId>,
    pub file: Option<GoogleFile>,
    pub removed: Option<bool>,
    pub time: Option<String>,
    pub change_type: Option<String>,
    pub drive_id: Option<DriveId>,
    pub drive: Option<GoogleDrive>,
    pub change_id: Option<String>,
}

pub struct GoogleChangeList {
    pub changes: Vec<GoogleChange>,
    pub next_page_token: Option<String>,
    pub new_start_page_token: Option<String>,
}

pub struct GoogleStartPageToken {
    pub start_page_token: String,
}

pub struct GoogleDrive {
    pub id: Option<DriveId>,
    pub name: Option<String>,
    pub hidden: Option<bool>,
    pub created_time: Option<String>,
    pub restrictions: Option<GoogleDriveRestrictions>,
    pub capabilities: Option<GoogleDriveCapabilities>,
    pub color_rgb: Option<String>,
    pub background_image_link: Option<String>,
    pub org_unit_id: Option<String>,
}

pub struct GoogleDriveRestrictions {
    pub copy_requires_writer_permission: Option<bool>,
    pub domain_users_only: Option<bool>,
    pub drive_members_only: Option<bool>,
    pub admin_managed_restrictions: Option<bool>,
    pub sharing_folders_requires_organizer_permission: Option<bool>,
}

pub struct GoogleDriveCapabilities {
    pub can_add_children: Option<bool>,
    pub can_comment: Option<bool>,
    pub can_copy: Option<bool>,
    pub can_delete_drive: Option<bool>,
    pub can_download: Option<bool>,
    pub can_edit: Option<bool>,
    pub can_list_children: Option<bool>,
    pub can_manage_members: Option<bool>,
    pub can_read_revisions: Option<bool>,
    pub can_rename: Option<bool>,
    pub can_share: Option<bool>,
}

pub struct GooglePermission {
    pub id: Option<PermissionId>,
    pub type_: Option<String>,
    pub role: Option<String>,
    pub email_address: Option<String>,
    pub domain: Option<String>,
    pub allow_file_discovery: Option<bool>,
    pub expiration_time: Option<String>,
    pub display_name: Option<String>,
    pub photo_link: Option<String>,
    pub deleted: Option<bool>,
    pub pending_owner: Option<bool>,
    pub permission_details: Option<Vec<GooglePermissionDetails>>,
    pub view: Option<String>,
}

pub struct GooglePermissionDetails {
    pub permission_type: Option<String>,
    pub role: Option<String>,
    pub inherited: Option<bool>,
    pub inherited_from: Option<FileId>,
}

pub struct GoogleRevision {
    pub id: Option<RevisionId>,
    pub mime_type: Option<MimeType>,
    pub modified_time: Option<String>,
    pub keep_forever: Option<bool>,
    pub size: Option<String>,
    pub md5_checksum: Option<String>,
    pub published: Option<bool>,
    pub publish_auto: Option<bool>,
    pub published_outside_domain: Option<bool>,
    pub original_filename: Option<String>,
    pub last_modifying_user: Option<GoogleUser>,
    pub export_links: Option<std::collections::BTreeMap<String,String>>,
}

pub struct GoogleComment {
    pub id: Option<CommentId>,
    pub content: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub author: Option<GoogleUser>,
    pub resolved: Option<bool>,
    pub deleted: Option<bool>,
    pub replies: Option<Vec<GoogleReply>>,
    pub anchor: Option<String>,
    pub quoted_file_content: Option<GoogleQuotedContent>,
}

pub struct GoogleQuotedContent {
    pub mime_type: Option<String>,
    pub value: Option<String>,
}

pub struct GoogleReply {
    pub id: Option<ReplyId>,
    pub content: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub author: Option<GoogleUser>,
    pub action: Option<String>,
    pub deleted: Option<bool>,
}

pub struct GoogleChannel {
    pub id: Option<ChannelId>,
    pub resource_id: Option<String>,
    pub resource_uri: Option<String>,
    pub expiration: Option<String>,
    pub token: Option<String>,
    pub type_: Option<String>,
    pub address: Option<String>,
    pub params: Option<std::collections::BTreeMap<String,String>>,
}

pub struct GoogleApp {
    pub id: Option<AppId>,
    pub name: Option<String>,
    pub object_type: Option<String>,
    pub product_url: Option<String>,
    pub create_url: Option<String>,
    pub open_url_template: Option<String>,
    pub supports_create: Option<bool>,
    pub supports_import: Option<bool>,
    pub installed: Option<bool>,
    pub authorized: Option<bool>,
    pub use_by_default: Option<bool>,
    pub primary_mime_types: Option<Vec<String>>,
    pub secondary_mime_types: Option<Vec<String>>,
}

pub struct GoogleAccessProposal {
    pub file_id: Option<FileId>,
    pub proposal_id: Option<ProposalId>,
    pub requester_email_address: Option<String>,
    pub recipient_email_address: Option<String>,
    pub roles_and_views: Option<Vec<GoogleRoleAndView>>,
    pub request_message: Option<String>,
    pub create_time: Option<String>,
}

pub struct GoogleRoleAndView {
    pub role: Option<String>,
    pub view: Option<String>,
}

pub struct GoogleApproval {
    pub approval_id: Option<ApprovalId>,
    pub target_file_id: Option<FileId>,
    pub create_time: Option<String>,
    pub modify_time: Option<String>,
    pub complete_time: Option<String>,
    pub due_time: Option<String>,
    pub status: Option<String>,
    pub initiator: Option<GoogleUser>,
    pub reviewer_responses: Option<Vec<GoogleApprovalReviewerResponse>>,
    pub file_content_change_behavior: Option<String>,
}

pub struct GoogleApprovalReviewerResponse {
    pub reviewer: Option<GoogleUser>,
    pub response: Option<String>,
}

pub struct GoogleOperation {
    pub name: Option<OperationName>,
    pub done: Option<bool>,
    pub metadata: Option<GoogleOperationMetadata>,
    pub response: Option<GoogleDownloadFileResponse>,
    pub error: Option<GoogleRpcStatus>,
}

pub struct GoogleOperationMetadata {
    pub state: Option<String>,
    pub progress_percent: Option<i32>,
}

pub struct GoogleRpcStatus {
    pub code: Option<i32>,
    pub message: Option<String>,
    pub details: Option<Vec<GoogleRpcDetail>>,
}

pub struct GoogleRpcDetail {
    pub type_url: Option<String>,
    pub reason: Option<String>,
    pub domain: Option<String>,
    pub metadata: Option<std::collections::BTreeMap<String, String>>,
}

pub struct GoogleDownloadFileResponse {
    pub download_uri: Option<String>,
    pub partial_download_allowed: Option<bool>,
}

pub struct GoogleCseToken {
    pub file_id: Option<FileId>,
    pub current_kacls_id: Option<String>,
    pub current_kacls_name: Option<String>,
    pub jwt: Option<String>,
}

pub struct GoogleGeneratedIds {
    pub ids: Option<Vec<FileId>>,
    pub space: Option<String>,
}

pub struct GoogleModifyLabelsResponse {
    pub modified_labels: Option<Vec<GoogleLabel>>,
}

pub enum GooglePatch<T> {
    Unchanged,
    Set(T),
    Clear,
}

pub struct GoogleFileWrite {
    pub name: GooglePatch<String>,
    pub mime_type: GooglePatch<MimeType>,
    pub description: GooglePatch<String>,
    pub trashed: GooglePatch<bool>,
    pub parents: GooglePatch<Vec<FileId>>,
    pub starred: GooglePatch<bool>,
    pub properties: GooglePatch<std::collections::BTreeMap<String,String>>,
    pub app_properties: GooglePatch<std::collections::BTreeMap<String,String>>,
    pub copy_requires_writer_permission: GooglePatch<bool>,
    pub folder_color_rgb: GooglePatch<String>,
    pub modified_time: GooglePatch<String>,
}

pub struct GoogleDriveWrite {
    pub name: GooglePatch<String>,
    pub restrictions: GooglePatch<GoogleDriveRestrictions>,
    pub color_rgb: GooglePatch<String>,
}

pub struct GooglePermissionWrite {
    pub type_: GooglePatch<String>,
    pub role: GooglePatch<String>,
    pub email_address: GooglePatch<String>,
    pub domain: GooglePatch<String>,
    pub allow_file_discovery: GooglePatch<bool>,
    pub expiration_time: GooglePatch<String>,
    pub view: GooglePatch<String>,
}

pub struct GoogleCommentWrite {
    pub content: GooglePatch<String>,
    pub anchor: GooglePatch<String>,
}

pub struct GoogleReplyWrite {
    pub content: GooglePatch<String>,
    pub action: GooglePatch<String>,
}

pub struct GoogleRevisionWrite {
    pub keep_forever: GooglePatch<bool>,
    pub published: GooglePatch<bool>,
    pub publish_auto: GooglePatch<bool>,
    pub published_outside_domain: GooglePatch<bool>,
}

``

```

### 9.2 Respostas de listas, solicitações, opções e contratos OAuth

As estruturas `XxxOptions` são **objetos de entrada**. O objeto `Options` é argumento obrigatório da assinatura, mas seus campos `Option<T>` são opcionais. Campos necessários, como o cursor de `ChangeListOptions`, usam tipos não opcionais.

As estruturas `Google*List` refletem a resposta paginada; as propriedades opcionais são independentes da existência de itens na lista.

```rust
pub struct GoogleAccessProposalList {
    pub access_proposals: Vec<GoogleAccessProposal>,
    pub next_page_token: Option<String>,
}

pub struct GoogleAppList {
    pub items: Vec<GoogleApp>,
    pub next_page_token: Option<String>,
}

pub struct GoogleApprovalList {
    pub items: Vec<GoogleApproval>,
    pub next_page_token: Option<String>,
}

pub struct GoogleCommentList {
    pub comments: Vec<GoogleComment>,
    pub next_page_token: Option<String>,
}

pub struct GoogleDriveList {
    pub drives: Vec<GoogleDrive>,
    pub next_page_token: Option<String>,
}

pub struct GoogleLabelList {
    pub labels: Vec<GoogleLabel>,
    pub next_page_token: Option<String>,
}

pub struct GooglePermissionList {
    pub permissions: Vec<GooglePermission>,
    pub next_page_token: Option<String>,
}

pub struct GoogleReplyList {
    pub replies: Vec<GoogleReply>,
    pub next_page_token: Option<String>,
}

pub struct GoogleRevisionList {
    pub revisions: Vec<GoogleRevision>,
    pub next_page_token: Option<String>,
}

pub struct AboutOptions {
    pub fields: Option<FieldMask>,
}

pub struct AppListOptions {
    pub app_filter_extensions: Option<Vec<String>>,
    pub app_filter_mime_types: Option<Vec<String>>,
    pub language_code: Option<String>,
    pub fields: Option<FieldMask>,
}

pub struct ChangeScopeOptions {
    pub drive_id: Option<DriveId>,
    pub supports_all_drives: Option<bool>,
}

pub struct ChangeListOptions {
    pub page_token: String,
    pub drive_id: Option<DriveId>,
    pub page_size: Option<u32>,
    pub spaces: Option<String>,
    pub include_removed: Option<bool>,
    pub include_corpus_removals: Option<bool>,
    pub include_items_from_all_drives: Option<bool>,
    pub supports_all_drives: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct ChangeWatchOptions {
    pub page_token: String,
    pub drive_id: Option<DriveId>,
    pub page_size: Option<u32>,
    pub spaces: Option<String>,
    pub include_removed: Option<bool>,
    pub include_items_from_all_drives: Option<bool>,
    pub supports_all_drives: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct CommentOptions {
    pub fields: Option<FieldMask>,
    pub include_deleted: Option<bool>,
}

pub struct CommentListOptions {
    pub fields: Option<FieldMask>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub include_deleted: Option<bool>,
    pub start_modified_time: Option<String>,
}

pub struct CseTokenOptions {
    pub file_id: Option<FileId>,
    pub parent: Option<FileId>,
}

pub struct DriveDeleteOptions {
    pub allow_item_deletion: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
}

pub struct DriveGetOptions {
    pub use_domain_admin_access: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct DriveListOptions {
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub q: Option<String>,
    pub use_domain_admin_access: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct DriveUpdateOptions {
    pub use_domain_admin_access: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct EmptyTrashOptions {
    pub drive_id: Option<DriveId>,
}

pub struct FileCopyOptions {
    pub supports_all_drives: Option<bool>,
    pub ignore_default_visibility: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct FileCreateOptions {
    pub supports_all_drives: Option<bool>,
    pub ignore_default_visibility: Option<bool>,
    pub use_content_as_indexable_text: Option<bool>,
    pub keep_revision_forever: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct FileDeleteOptions {
    pub supports_all_drives: Option<bool>,
    pub enforce_single_parent: Option<bool>,
}

pub struct FileDownloadOptions {
    pub mime_type: Option<MimeType>,
    pub revision_id: Option<RevisionId>,
}

pub struct FileGetOptions {
    pub fields: Option<FieldMask>,
    pub supports_all_drives: Option<bool>,
    /// Comma-separated label IDs for `labelInfo` (query `includeLabels`).
    pub include_labels: Option<String>,
    /// Only `published` is supported (query `includePermissionsForView`).
    pub include_permissions_for_view: Option<String>,
}

pub struct FileListOptions {
    pub q: Option<String>,
    pub corpora: Option<String>,
    pub drive_id: Option<DriveId>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub spaces: Option<String>,
    pub order_by: Option<String>,
    pub fields: Option<FieldMask>,
    pub include_items_from_all_drives: Option<bool>,
    pub supports_all_drives: Option<bool>,
    pub include_permissions_for_view: Option<String>,
}

pub struct FileUpdateOptions {
    pub add_parents: Option<Vec<FileId>>,
    pub remove_parents: Option<Vec<FileId>>,
    pub supports_all_drives: Option<bool>,
    pub keep_revision_forever: Option<bool>,
    pub use_content_as_indexable_text: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct FileWatchOptions {
    pub acknowledge_abuse: Option<bool>,
    pub supports_all_drives: Option<bool>,
}

pub struct GenerateIdsOptions {
    pub count: Option<u32>,
    pub space: Option<String>,
    pub type_: Option<String>,
}

pub struct LabelListOptions {
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
}

pub struct PermissionCreateOptions {
    pub send_notification_email: Option<bool>,
    pub email_message: Option<String>,
    pub supports_all_drives: Option<bool>,
    pub transfer_ownership: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct PermissionDeleteOptions {
    pub supports_all_drives: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
}

pub struct PermissionGetOptions {
    pub supports_all_drives: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct PermissionListOptions {
    pub supports_all_drives: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub fields: Option<FieldMask>,
}

pub struct PermissionUpdateOptions {
    pub supports_all_drives: Option<bool>,
    pub use_domain_admin_access: Option<bool>,
    pub remove_expiration: Option<bool>,
    pub transfer_ownership: Option<bool>,
    pub fields: Option<FieldMask>,
}

pub struct ReplyOptions {
    pub fields: Option<FieldMask>,
    pub include_deleted: Option<bool>,
}

pub struct ReplyListOptions {
    pub fields: Option<FieldMask>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub include_deleted: Option<bool>,
}

pub struct RevisionGetOptions {
    pub fields: Option<FieldMask>,
    pub acknowledge_abuse: Option<bool>,
}

pub struct RevisionListOptions {
    pub fields: Option<FieldMask>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
}

pub struct GoogleChannelRequest {
    pub id: String,
    pub type_: String,
    pub address: String,
    pub token: Option<String>,
    pub expiration: Option<String>,
    pub params: Option<std::collections::BTreeMap<String,String>>,
}

pub struct GoogleChannelStopRequest {
    pub id: ChannelId,
    pub resource_id: String,
}

pub struct GoogleModifyLabelsRequest {
    pub label_modifications: Vec<GoogleLabelModification>,
}

pub struct GoogleLabelModification {
    pub label_id: String,
    pub field_modifications: Vec<GoogleLabelFieldModification>,
    pub remove_label: Option<bool>,
}

pub struct GoogleLabelFieldModification {
    pub field_id: String,
    pub set_text_values: Option<Vec<String>>,
    pub set_integer_values: Option<Vec<String>>,
    pub set_date_values: Option<Vec<String>>,
    pub set_selection_values: Option<Vec<String>>,
    pub unset_values: Option<bool>,
}

pub struct ApproveApprovalRequest {
    pub message: Option<String>,
}

pub struct CancelApprovalRequest {
    pub message: Option<String>,
}

pub struct CommentApprovalRequest {
    pub message: String,
}

pub struct DeclineApprovalRequest {
    pub message: Option<String>,
}

pub struct ReassignApprovalRequest {
    pub replace_reviewers: Option<Vec<GoogleReplaceReviewer>>,
    pub add_reviewers: Option<Vec<GoogleAddReviewer>>,
    pub message: Option<String>,
}
pub struct GoogleReplaceReviewer {
    pub removed_reviewer_email: String,
    pub added_reviewer_email: String,
}
pub struct GoogleAddReviewer {
    pub added_reviewer_email: String,
}

pub struct StartApprovalRequest {
    pub reviewer_emails: Vec<String>,
    pub lock_file: Option<bool>,
    pub message: Option<String>,
    pub due_time: Option<String>,
    pub file_content_change_behavior: Option<String>,
}

pub struct GoogleApprovalReviewer {
    pub email_address: String,
}

pub struct ResolveAccessProposalRequest {
    pub action: String,
    pub role: Option<Vec<String>>,
    pub view: Option<String>,
    pub send_notification: Option<bool>,
}

pub struct GoogleConfigSources {
    pub client_id_env: Option<String>,
    pub client_secret_env: Option<String>,
    pub oauth_file_env: Option<String>,
    pub bundled_config_path: Option<String>,
}

pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
}

pub struct GoogleAuthorizationRequest {
    pub redirect_uri: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scopes: Vec<String>,
    pub access_type: Option<String>,
    pub prompt: Option<String>,
    pub login_hint: Option<String>,
}

pub struct GoogleAuthorizationUrl {
    pub url: String,
}

pub struct GoogleTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub id_token: Option<String>,
    pub refresh_token_expires_in: Option<u64>,
}

pub struct GoogleUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub hd: Option<String>,
}

pub struct GoogleIdTokenValidation {
    pub issuer: String,
    pub audience: String,
    pub nonce: Option<String>,
    pub clock_skew_seconds: Option<u64>,
}

pub struct GoogleIdClaims {
    pub iss: String,
    pub aud: GoogleAudience,
    pub azp: Option<String>,
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub nonce: Option<String>,
}

pub struct GoogleOidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub revocation_endpoint: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
}

pub struct GoogleJwkSet {
    pub keys: Vec<GoogleJwk>,
}

pub struct GoogleJwk {
    pub kid: Option<String>,
    pub kty: String,
    pub alg: Option<String>,
    pub use_: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
    pub crv: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
}

pub struct GoogleByteRange {
    pub start: u64,
    pub end_inclusive: Option<u64>,
}

pub struct GoogleDownloadStream {
    pub body: GoogleByteStream,
    pub content_length: Option<u64>,
    pub content_range: Option<String>,
    pub etag: Option<String>,
    pub mime_type: Option<MimeType>,
}

pub struct GoogleUploadSession {
    pub session_uri: String,
    pub total_bytes: Option<u64>,
}

pub struct GoogleUploadChunk {
    pub start: u64,
    pub bytes: Vec<u8>,
    pub total_bytes: Option<u64>,
}

pub enum GoogleUploadProgress {
    Incomplete { received_range: Option<GoogleByteRange> },
    Complete { file: GoogleFile },
}
```

### 9.3 Obrigatoriedade e disponibilidade dos campos

| Situação | Como representar | Consequência para o consumidor |
| --- | --- | --- |
| Argumento obrigatório na assinatura | `FileId`, `String`, outros tipos não opcionais | A chamada não pode ser construída sem o argumento |
| Parâmetro HTTP opcional | Campo `Option<T>` em `*Options` | `None` significa **não enviar o parâmetro** |
| Campo JSON possivelmente omitido na resposta | `Option<T>` em estrutura de resposta | `None` significa **não recebido**, não confirma ausência do atributo no Drive |
| Campo solicitado por `fields` | `FieldMask` configurada pelo consumidor | A interpretação precisa considerar a máscara de campos efetivamente requisitada |
| Campo válido só para alguns recursos | Ex.: checksum para conteúdo binário, `shortcut_details` para atalhos | `None` é esperado para tipos que não suportam o campo |
| Dados sem acesso por permissão ou escopo | Erro tipado ou atributo omitido, conforme endpoint | Não deduzir `false`, vazio ou exclusão |
| PATCH com campo omitido | `GooglePatch::Unchanged` em `*Write` | Não alterar o campo |
| PATCH que exige limpar um campo | `GooglePatch::Clear`, quando o campo admite remoção | Não confundir limpar com campo não informado |
| Coleção retornada vazia | `Vec<T>` vazio com resposta válida | Não equivale necessariamente a árvore remota vazia; considerar paginação/incompletude |
| Int64 em JSON | `String` no transporte, com parse validado | Não converter para `f64` nem descartar sinal/precisão |

**Os modelos acima são o contrato Rust proposto do DriveSync**, não uma reprodução literal de toda propriedade possível dos schemas JSON. No momento de implementar cada método, os campos devem ser comparados aos recursos oficiais e aos campos solicitados por `fields`. Esse controle protege a evolução e a fidelidade do contrato técnico sem expor o formato JSON ou regras de transporte fora da interface da API.
### 9.4 Erros públicos e critérios de recuperação

A interface retorna **`GoogleApiFailure`**, que separa a categoria estável (`kind`) do contexto sanitizado (`http_status`, `provider_reason`, `retry_after_seconds`, `retryable`). O HTTP e as razões específicas são interpretados **dentro de `providers/google/`**. A camada consumidora não precisa analisar JSON ou mensagens do Google para decidir o que fazer.

| Resposta ou motivo do Google | Categoria pública esperada | Recuperação |
| --- | --- | --- |
| `400 badRequest`, parâmetro malformado | `InvalidArgument` | Corrigir entrada; não repetir sem mudança |
| OAuth `invalid_grant` | `InvalidGrant` ou `ReauthorizationRequired` | Exigir novo fluxo de autorização quando necessário |
| `401` e token expirado/rejeitado | `AuthenticationRequired` | Renovação ou autenticação; sem excluir conta automaticamente |
| `403 insufficientPermissions` ou escopo insuficiente | `InsufficientScope` / `PermissionDenied` | Solicitar autorização compatível ou intervenção |
| `403 userRateLimitExceeded`, `rateLimitExceeded`, `429` | `RateLimited` | Retry com backoff e respeito a `Retry-After` |
| `403 storageQuotaExceeded` | `StorageQuotaExceeded` | Intervenção/liberação de quota; não reexecutar indiscriminadamente |
| `403 fileNotDownloadable` | `FileNotDownloadable` | Verificar tipo MIME; exportar quando suportado |
| `403` por permissão, política administrativa ou compartilhamento | `PermissionDenied` | Não presumir falha transitória |
| `404` | `NotFoundOrInaccessible` | Não considerar exclusão remota comprovada |
| `409`/`412` por condição/precondição | `PreconditionFailed` | Reavaliar metadados antes de repetir |
| `410` em cursores de changes | `ExpiredCursor` | Invalidar cursor e reiniciar detecção segura |
| `500`–`504` (e demais `5xx`) | `ProviderUnavailable` | Retry limitado, com backoff |
| `3xx` (redirecionamento) | `InvalidResponse` com `http_status` preservado | Redirecionamentos **não são seguidos** e o `Location` não é exposto; a API do Drive não deve redirecionar |
| `2xx` diferente do esperado pela operação, p.ex. `204` em `files_get` | `InvalidResponse` | Falha explícita |
| Corpo acima do limite de metadados, truncado ou inválido | `InvalidResponse` / `Network` | Nunca interpretar resposta parcial como completa |
| Timeout, conexão ou resolução DNS | `Timeout` / `Network` | Retry condicionado a idempotência/resultado conhecido |
| Conteúdo JSON inválido ou excessivo | `InvalidResponse` | Falha explícita, sem declarar sucesso |
| Operação cancelada | `Cancelled` | Não confirmar resultado não verificado |
| Sessão de upload recusada/expirada | `ExpiredUploadSession` | Retomar ou reiniciar conforme garantias do upload |
| `308 Resume Incomplete` | **`GoogleUploadProgress::Incomplete`** | **Não é erro** e **não é conclusão** |
| `Operation.done=false` | **`GoogleOperation` pendente** | **Não é erro** e **não é conclusão** |

**Regras:** (1) `retryable` expressa uma recomendação técnica, mas reexecução de escrita exige considerar idempotência e efeito possivelmente já aplicado; (2) `provider_reason` contém só o código seguro da razão, nunca o corpo da resposta; (3) `http_status=None` é válido para falhas de rede/validação local; (4) erros de autorização de uma conta não invalidam as demais; (5) `Retry-After` é representado **uma única vez**, em `GoogleApiFailure.retry_after_seconds` (a variante `RateLimited` não carrega dados), aceitando segundos inteiros ou HTTP-date (data passada resulta em `0`; valor inválido é descartado); (6) requisições de metadados usam timeout total e limite de bytes de resposta explícitos, fixos em produção; (7) endpoints diferentes dos oficiais existem apenas sob o feature `test-endpoints` e aceitam somente `http` com IP loopback.

Fontes: [erros do Drive](https://developers.google.com/workspace/drive/api/guides/handle-errors), [parâmetro fields](https://developers.google.com/workspace/drive/api/guides/fields-parameter), [OAuth 2.0](https://developers.google.com/identity/protocols/oauth2/native-app).


### Limites concretos de `files_get`

A implementação usa timeout total de 30 segundos e limite de 8 MiB para metadados,
incluindo leitura incremental do corpo. Não segue redirects nem descobre proxies
automaticamente. Só HTTP 200 com objeto JSON completo é sucesso. Erros HTTP com
corpo excessivo preservam status e `Retry-After`, sem interpretar o corpo parcial.
IDs vazios, `.` e `..` são rejeitados antes do envio; demais IDs são codificados
como um único segmento. Falhas do transporte não expõem URLs ou causas brutas.

`GoogleClientLimits` expõe os limites técnicos; `with_limits` e `with_base_urls`
só existem com `test-endpoints`. O construtor de testes retorna
`GoogleResult<GoogleDriveClient>`, valida ambas as bases e exige HTTP com IP
loopback literal, sem credenciais, query ou fragmento. Não implementa uploads.
A sessão é enviada por requisição, sem credenciais persistidas no cliente.

### 9.5 Divergências verificadas contra o Discovery v3 (auditoria TDD do grupo `files_get`)

| Item | Correção/estado |
| --- | --- |
| `FileGetOptions` | Adicionados `includeLabels` e `includePermissionsForView` (parâmetros oficiais de `files.get`); `supportsTeamDrives` é obsoleto e não é exposto |
| `GoogleDownloadRestrictions` | O campo oficial é `effectiveDownloadRestrictionWithContext`, não `effectiveDownloadRestriction` |
| `GoogleLabel.name` | Inexistente no recurso `Label` oficial; removido |
| `GoogleFile`, `GoogleFileCapabilities`, `GoogleImageMediaMetadata`, `GoogleDecryptionMetadata`, `GooglePermission` | **Decisão pendente:** o Discovery possui campos adicionais (ex.: `trashingUser`, `viewedByMe`, `exportLinks`, `canTrash`, `canModifyContent`); decidir quais fazem parte do contrato antes de GREEN dos grupos que os consumam |
| `files_get` com `acknowledgeAbuse` | **Decidido:** o parâmetro só é válido com `alt=media`; foi removido de `FileGetOptions` e nunca é enviado em consultas de metadados. Pertence às operações de download (grupo de transferências) |
| Execução | **Decidido:** I/O é `async` (reqwest + Tokio); sem política de concorrência |
| `Retry-After` | **Decidido:** representado uma vez em `GoogleApiFailure`; `RateLimited` sem dados |
| 3xx | **Decidido:** `InvalidResponse`, sem seguir redirecionamento |


### 9.6 Auditoria dos grupos restantes (Discovery revisão 20261106)

Fonte normativa: https://www.googleapis.com/discovery/v1/apis/drive/v3/rest.
A implementação preserva o catálogo de 59 métodos REST, sem os aliases obsoletos
`teamdrives`. As estruturas públicas são projeções tipadas, não todo o Discovery.
Campos desconhecidos da API são ignorados; campos omitidos não provam ausência.

Correções antes de implementar:

- `LabelListOptions.page_size` mapeia para `maxResults`, não `pageSize`.
- `GoogleApprovalList.items` corresponde ao JSON `items`.
- Cancelamento, comentário e recusa de aprovação usam `message`.
- Início usa `reviewerEmails`; reatribuição usa `addReviewers` e
  `replaceReviewers`, com os tipos oficiais de e-mail.
- `GoogleRpcStatus.details` usa detalhes tipados de erro, sem strings opacas.
- Opções compartilhadas são filtradas por método: `includeDeleted` só em
  leituras de comentários/respostas, `acknowledgeAbuse` não em metadados de revisão ou watch. Esses campos
  de compatibilidade não autorizam enviar o parâmetro sem `alt=media`.
- `GoogleChange.change_id` e `GoogleAppList.next_page_token` são campos de
  compatibilidade que não são preenchidos pela API atual.
- `GooglePatch::Unchanged` omite o campo, `Set` envia o valor e `Clear` envia null;
  o método remoto ainda valida se o campo admite limpeza. `parents` é aceito na
  criação; mover arquivo existente usa `addParents`/`removeParents`. PATCH (incluindo início
  de upload de atualização) rejeita `parents` no corpo antes do envio.

A forma concreta do stream será assíncrona, com erro tipado por leitura e sem
buffer completo obrigatório. Uploads não terão retry automático; `308` preserva
somente o intervalo confirmado. URIs de sessão serão validadas antes de enviar
autorização e nunca expostas em Debug ou erros.


### 9.7 Contrato concreto OAuth/OIDC

`GoogleConfigSources` recebe valores de overrides já resolvidos pelo chamador:
`client_id_env`/`client_secret_env` são os valores, e `oauth_file_env` é um caminho.
Precedência: overrides individuais > arquivo indicado > arquivo empacotado.
Arquivo explicitamente indicado inválido falha; não há fallback silencioso.
São aceitos JSON desktop `installed` e o objeto `GoogleOAuthConfig` direto.
Configuração pública de desktop não exige segredo. Escopos não são inventados;
autorização exige os escopos explícitos e permitidos pela configuração.

`load_google_config` é async, com leitura de arquivo limitada a 64 KiB via Tokio.
`authorization_request` é pura e síncrona: não abre navegador e não gera state ou
verifier. O chamador fornece state não vazio e challenge S256 base64url canônico com 43
caracteres; troca de código exige verifier RFC 7636 com 43–128 caracteres.
O redirect desktop deve ser HTTP loopback e corresponder a host/caminho da
configuração, permitindo a porta efêmera do callback.

Validação de ID token exige assinatura RS256 por chave oficial, kid conhecido,
issuer Google, audiência esperada, expiração, iat e nonce quando fornecido.
`GoogleAudience` preserva string ou lista de strings do claim `aud`; `azp` é
validado quando presente e exigido para múltiplas audiências. Tolerância de relógio
é explícita (padrão 60 s, máximo 300 s). Chaves/discovery são consultadas sem cache
persistente. A URL de chaves deve ser o endpoint Google conhecido; destinos locais
existem apenas sob `test-endpoints`. Nenhuma URL retornada pela rede autoriza o
envio de credenciais a um destino arbitrário.

### 9.8 Streaming e retomada de bytes

`GoogleByteStream` é um tipo concreto com
`async fn next_chunk(&mut self) -> GoogleResult<Option<Vec<u8>>>`; não implementa
`std::io::Read`. Uma falha encerra o stream e nunca confirma integridade. A leitura
é incremental, sem carregar o arquivo completo. `GoogleDownloadStream` também
expõe `http_status: u16`. Comprimento conhecido é verificado no EOF; corpo truncado
é `IncompleteTransfer`, timeout continua `Timeout`.

Range inválido falha antes do envio. Pedido de intervalo exige HTTP 206 e
`Content-Range` coerente com início, limite e comprimento; HTTP 200 não é tratado
como se começasse no offset solicitado. Sem range exige HTTP 200. `files_export`
e `export_file_bytes` usam a exportação oficial, sem confundir com `alt=media`.
O timeout de transporte vale também durante leitura; cancelamento por descarte
não prova reversão ou ausência de recebimento remoto.

### 9.9 Upload resumível e conveniências de metadados

Sessões são iniciadas no endpoint oficial de upload com `uploadType=resumable`.
A URI recebida só é aceita se tiver a mesma origem e caminho sob a base de upload
configurada, sem usuário/fragmento; toda utilização revalida essa restrição.
`GoogleUploadSession` e `GoogleUploadChunk` ocultam URI/bytes em Debug.

Blocos não finais têm tamanho múltiplo de 256 KiB; o bloco final pode ser menor.
Tamanhos e offsets são validados com aritmética verificada. Quando o total não é
conhecido, blocos não vazios continuam não finais até um bloco informar o total.
Um arquivo vazio usa `Content-Range: bytes */0`. Consulta de progresso usa corpo
vazio e `bytes */total` (ou `bytes */*`). `Range` de resposta 308 confirma apenas
`bytes=0-N`; cabeçalho ausente significa nenhum intervalo confirmado, não sucesso.
404/410 de sessão viram `ExpiredUploadSession`. Respostas 200/201 com recurso
completo são a única confirmação final; não há retry nem consulta automática.

`trash_file`, `rename_file` e `move_file` usam exclusivamente PATCH de metadados,
com `supportsAllDrives=true`. Mover usa `addParents` e `removeParents`, nunca bytes.
