# Google API — INTERFACES

**Contrato Rust v1 para TDD.** Todas as operações são públicas nesta versão. As assinaturas representam a API que será implementada, não código existente. A visibilidade será revisada posteriormente.

**Escopo:** os **59 métodos oficiais dos 13 recursos REST da Drive API v3** documentados no [índice oficial](https://developers.google.com/workspace/drive/api/reference/rest/v3), mais OAuth 2.0, OpenID Connect, uploads e downloads. APIs de outros produtos Google, como Gmail, Calendar e Drive Labels API v2, são serviços separados.

**Fontes:** [Drive REST v3](https://developers.google.com/workspace/drive/api/reference/rest/v3), [Discovery v3](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest), [OAuth desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [OpenID Connect](https://developers.google.com/identity/openid-connect/reference).

## 1. Tipos públicos comuns

```rust
pub type GoogleResult<T> = Result<T, GoogleApiError>;
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
    InvalidArgument,
    AuthenticationRequired,
    ReauthorizationRequired,
    PermissionDenied,
    NotFoundOrInaccessible,
    PreconditionFailed,
    RateLimited { retry_after_seconds: Option<u64> },
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

## 2. Recursos e dados retornados

| Tipo público | Campos essenciais expostos (campo JSON original quando diferente) |
| --- | --- |
| `GoogleAbout` | `user`, `storage_quota`, `import_formats`, `export_formats`, `max_import_sizes`, `app_installed` |
| `GoogleFile` | `id`, `name`, `mime_type` (`mimeType`), `parents`, `trashed`, `version`, `modified_time`, `size`, `md5_checksum`, `sha1_checksum`, `sha256_checksum`, `drive_id`, `web_view_link`, `web_content_link`, `capabilities`, `owners`, `permissions`, `shortcut_details`, `content_hints`, `description`, `properties`, `app_properties`, `labels`, `resource_key`, `head_revision_id`, `created_time`, `quota_bytes_used` |
| `GoogleFileWrite` | `name?`, `mime_type?`, `parents?`, `description?`, `trashed?`, `properties?`, `app_properties?`, demais campos graváveis conforme API |
| `GoogleFileList` | `files: Vec<GoogleFile>`, `next_page_token?`, `incomplete_search` |
| `GoogleChange` | `file_id?`, `file?`, `removed`, `time?`, `change_type?`, `drive_id?`, `drive?` |
| `GoogleChangeList` | `changes: Vec<GoogleChange>`, `next_page_token?`, `new_start_page_token?` |
| `GoogleStartPageToken` | `start_page_token` |
| `GoogleDrive` | `id`, `name`, `hidden`, `created_time`, `restrictions`, `capabilities` |
| `GooglePermission` | `id`, `type`, `role`, `email_address?`, `domain?`, `allow_file_discovery?`, `expiration_time?`, `permission_details?` |
| `GoogleRevision` | `id`, `mime_type?`, `modified_time?`, `keep_forever?`, `size?`, `md5_checksum?`, `published?` |
| `GoogleComment` | `id`, `content`, `created_time`, `modified_time`, `author`, `resolved`, `replies` |
| `GoogleReply` | `id`, `content`, `created_time`, `modified_time`, `author`, `action?` |
| `GoogleChannel` | `id`, `resource_id`, `resource_uri`, `expiration?`, `token?` |
| `GoogleApp` | `id`, `name`, `supports_create`, `supports_import`, `open_url_template?` |
| `GoogleAccessProposal` | `file_id`, `proposal_id`, `requester_email_address`, `recipient_email_address?`, `roles_and_views`, `create_time` |
| `GoogleApproval` | `approval_id`, `target_file_id`, `status`, `initiator`, `reviewer_responses`, `due_time?`, `file_content_change_behavior` |
| `GoogleOperation` | `name`, `done?`, `metadata?`, `response?`, `error?` |
| `GoogleCseToken` | Token de criptografia do lado do cliente; tratado como segredo |
| `GoogleGeneratedIds` | `ids: Vec<FileId>`, `space` |
| `GoogleLabelList` | `labels`, `next_page_token?` |
| `GoogleModifyLabelsResponse` | `modified_labels` |

**Representações complexas:** `GoogleUser`, `GoogleStorageQuota`, `GoogleFileCapabilities`, `GooglePermissionDetails`, `GoogleShortcutDetails`, `GoogleContentHints`, `GoogleDriveRestrictions`, `GoogleApprovalReviewerResponse`, `GoogleLabel`, `GoogleAppList`, `GooglePermissionList`, `GoogleRevisionList`, `GoogleCommentList`, `GoogleReplyList`, `GoogleDriveList`, `GoogleAccessProposalList` e `GoogleApprovalList` são **tipos públicos próprios**, não JSON genérico. As listas seguem a convenção de paginação do recurso e suas respectivas propriedades adicionais oficiais.

### Parâmetros e corpos de entrada

| Grupo | Tipos de entrada | Campos relevantes que devem ser disponibilizados |
| --- | --- | --- |
| Geral | `PageOptions`, `FieldMask`, `AboutOptions` | `page_size`, `page_token`, `fields` |
| Files leitura | `FileGetOptions`, `FileListOptions`, `FileDownloadOptions` | `fields`, `q`, `corpora`, `drive_id`, `include_items_from_all_drives`, `supports_all_drives`, `spaces`, `order_by`, `page_size`, `page_token`, `mime_type?`, `revision_id?` |
| Files escrita | `FileCopyOptions`, `FileCreateOptions`, `FileUpdateOptions`, `FileDeleteOptions`, `EmptyTrashOptions` | `fields`, `supports_all_drives`, `add_parents`, `remove_parents`, `keep_revision_forever`, `ignore_default_visibility`, `enforce_single_parent` quando suportado |
| Files extras | `LabelListOptions`, `GoogleModifyLabelsRequest`, `GenerateIdsOptions`, `CseTokenOptions` | `page_token`, `max_results`, `label_modifications`, `count`, `space`, `type` |
| Changes | `ChangeScopeOptions`, `ChangeListOptions`, `ChangeWatchOptions` | `drive_id`, `page_token`, `page_size`, `spaces`, `include_removed`, `include_corpus_removals`, `supports_all_drives` |
| Canais | `GoogleChannelRequest`, `GoogleChannelStopRequest`, `FileWatchOptions` | `id`, `type`, `address`, `token?`, `expiration?`, `params?`, `resource_id` para interromper |
| Drives | `GoogleDriveWrite`, `DriveGetOptions`, `DriveListOptions`, `DriveDeleteOptions`, `DriveUpdateOptions` | `name`, `restrictions`, `fields`, `q`, `page_size`, `page_token`, `use_domain_admin_access`, `allow_item_deletion` |
| Permissions | `GooglePermissionWrite`, `PermissionCreateOptions`, `PermissionDeleteOptions`, `PermissionGetOptions`, `PermissionListOptions`, `PermissionUpdateOptions` | `type`, `role`, `email_address?`, `domain?`, `send_notification_email`, `transfer_ownership`, `use_domain_admin_access`, `supports_all_drives` |
| Comments | `GoogleCommentWrite`, `CommentOptions`, `CommentListOptions` | `content`, `anchor?`, `quoted_file_content?`, `fields`, `include_deleted`, `page_token`, `page_size` |
| Replies | `GoogleReplyWrite`, `ReplyOptions`, `ReplyListOptions` | `content`, `action?`, `fields`, `include_deleted`, `page_token`, `page_size` |
| Revisions | `GoogleRevisionWrite`, `RevisionGetOptions`, `RevisionListOptions` | `keep_forever`, `published`, `publish_auto`, `fields`, `page_token`, `page_size` |
| Access proposals | `ResolveAccessProposalRequest` | `action`, `role`, `view`, `send_notification` conforme contrato oficial |
| Approvals | `StartApprovalRequest`, `ApproveApprovalRequest`, `CancelApprovalRequest`, `CommentApprovalRequest`, `DeclineApprovalRequest`, `ReassignApprovalRequest` | revisores, mensagem, comentários, prazo e ações autorizadas conforme o método |
| Apps | `AppListOptions` | `app_filter_extensions`, `app_filter_mime_types`, `language_code` |

Campos de parâmetros marcados como “quando suportado” precisam ser validados contra **cada endpoint**, não aplicados indiscriminadamente. Tipos `*Write` devem distinguir campo ausente de campo explicitamente limpo.

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
| `access_proposals_resolve` | `file_id: &FileId, proposal_id: &ProposalId, request: ResolveAccessProposalRequest` | `GoogleResult<GoogleAccessProposal>` | `POST /files/{fileId}/accessproposals/{proposalId}:resolve` |

```rust
pub trait GoogleAccessproposalsApi {
    fn access_proposals_get(&self, session: &GoogleSession, file_id: &FileId, proposal_id: &ProposalId) -> GoogleResult<GoogleAccessProposal>;
    fn access_proposals_list(&self, session: &GoogleSession, file_id: &FileId, options: PageOptions) -> GoogleResult<GoogleAccessProposalList>;
    fn access_proposals_resolve(&self, session: &GoogleSession, file_id: &FileId, proposal_id: &ProposalId, request: ResolveAccessProposalRequest) -> GoogleResult<GoogleAccessProposal>;
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
    fn files_get(&self, session: &GoogleSession, file_id: &FileId, options: FileGetOptions) -> GoogleResult<GoogleFile>;
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

`accounts/auth/` coordena cadastro, callback, estado de login e cofre de credenciais. `google_api/` fornece as operações específicas do protocolo Google; não mantém contas cadastradas nem persiste tokens.

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
| `GoogleUploadChunk` | Offset inicial, bytes, total esperado e indicador de finalização |
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

## 6. Critérios de teste independentes

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

## 7. Revisão posterior

Esta versão fixa a **cobertura de métodos** e os tipos de entrada/saída conceituais. Antes de converter em código compilável serão validados: (a) todos os campos aninhados de cada recurso do Discovery, (b) as opções individuais permitidas por endpoint, (c) quais métodos/tipos podem tornar-se privados, (d) sincronismo versus async e (e) capacidades adicionais de outros serviços Google, caso sejam incorporados ao escopo do projeto.
