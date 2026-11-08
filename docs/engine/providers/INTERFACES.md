# Providers — INTERFACES: fronteira única de serviços externos

> **Estado:** arquitetura-alvo contratual, não Rust compilado. O módulo `providers/` é a **única fronteira da engine para APIs de serviços remotos** (Google Drive e provedores futuros). O contrato existente de 59 métodos REST da [Google API](google/api/INTERFACES.md) **continua válido apenas como interface técnica específica da implementação Google**, sem se tornar API comum dos consumidores.

## 1. Regra de dependência

**Permitido:** `accounts/auth/`, `detectors/remote/`, `operations/upload/`, `operations/download/` e futuros consumidores chamam **somente portas de `providers/`**. As implementações internas `providers/google/` podem chamar `providers/google/api/` e interpretar `GoogleFile`, `GoogleChange` e protocolo OAuth.

**Proibido:** importações de `GoogleSession`, `GoogleFile`, `GoogleApiClient`, endpoints REST, tokens, HTTP ou credenciais em contas, detectores, analisador, inventário e operadores. `models/` permanece puro, sem I/O. `ProviderId`, `AccountId`, `SyncId`, `RemoteFile`, `RemoteFileRef` são tipos compartilhados, não redefinidos por Providers.

**Abstração:** operações comuns são contratadas por capacidades; não criar um equivalente genérico de todos os 59 endpoints Google. A ausência de capacidade é erro `UnsupportedCapability`, **não** sucesso fictício.

## 2. Fachada e composição do provedor

Os exemplos são **assinaturas Rust conceituais**; implementações poderão ajustar borrowing/async/streaming sem alterar propriedades do domínio.

```rust
pub trait Providers {
    fn list_available(&self, request: ListProvidersRequest)
        -> ProviderResult<ListProvidersResult>;
    fn capabilities(&self, request: ProviderCapabilitiesRequest)
        -> ProviderResult<ProviderCapabilitiesResult>;
    fn auth(&self, request: OpenProviderAuthRequest)
        -> ProviderResult<ProviderAuthPort>;
    fn open_account(&self, request: OpenProviderAccountRequest)
        -> ProviderResult<ProviderAccountPort>;
}

pub struct ListProvidersRequest {}
pub struct ListProvidersResult { pub providers: Vec<ProviderDescriptor> }

pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: String,
    pub capabilities: ProviderCapabilities,
}

pub struct ProviderCapabilities {
    pub authentication: bool,
    pub list_files: bool,
    pub incremental_changes: bool,
    pub upload: bool,
    pub download: bool,
    pub mutate_metadata: bool,
    pub trash: bool,
    pub resumable_transfer: bool,
}
pub struct ProviderCapabilitiesRequest { pub provider_id: ProviderId }
pub struct ProviderCapabilitiesResult { pub descriptor: ProviderDescriptor }

pub struct OpenProviderAuthRequest { pub provider_id: ProviderId }
pub struct OpenProviderAccountRequest {
    pub provider_id: ProviderId,
    pub account_id: AccountId,
    pub access: AuthorizedAccountHandle,
}

// Portas/handles opacos de sessão e autenticação, não strings de token.
pub struct ProviderAuthPort { /* implementação específica encapsulada */ }
pub struct ProviderAccountPort { /* conta + capacidades + sessão autorizada */ }
pub struct AuthorizedAccountHandle { /* emitido pelo coordenador accounts/auth */ }
```

**Composição:** selecionar `ProviderId` uma vez durante cadastro/abertura da conta. O retorno `ProviderAccountPort` fica vinculado a **uma conta autenticada**, sem seleção global a cada requisição. `ProviderId` não identifica uma conta nem concede autorização. `ProviderAccountPort` não oferece SQL, token bruto ou referência à API do Google.

## 3. Porta de autenticação para Accounts

`accounts/` possui cadastro e vinculação `(provider, provider_user_id)`. `accounts/auth/` é proprietário do **ciclo de vida de credenciais e armazenamento seguro**, do consentimento e de invalidar sessão. `providers/` é proprietário dos **protocolos externos de autenticação**, implementados por cada provedor (OAuth 2.0/OIDC Google, outro protocolo futuro).

```rust
pub trait ProviderAuthentication {
    fn begin(&self, request: BeginAuthenticationRequest)
        -> ProviderResult<BeginAuthenticationResult>;
    fn finish(&self, request: FinishAuthenticationRequest)
        -> ProviderResult<FinishAuthenticationResult>;
    fn refresh(&self, request: RefreshAuthenticationRequest)
        -> ProviderResult<RefreshAuthenticationResult>;
    fn revoke(&self, request: RevokeAuthenticationRequest)
        -> ProviderResult<RevokeAuthenticationResult>;
}

pub struct BeginAuthenticationRequest {
    pub requested_scopes: Vec<String>,
    pub purpose: AuthenticationPurpose,
}
pub enum AuthenticationPurpose { NewAccount, Reauthorize }
pub struct BeginAuthenticationResult {
    pub attempt: AuthAttemptHandle,
    pub user_action: AuthenticationUserAction,
}
pub enum AuthenticationUserAction {
    OpenBrowser { url: String },
    // Outros mecanismos só serão definidos se algum provedor os exigir.
}
pub struct FinishAuthenticationRequest {
    pub attempt: AuthAttemptHandle,
    pub callback: AuthenticationCallback,
}
pub struct FinishAuthenticationResult {
    pub identity: ProviderIdentity,
    pub credential_material: ProtectedCredentialMaterial,
}
pub struct ProviderIdentity {
    pub provider_id: ProviderId,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}
pub struct RefreshAuthenticationRequest {
    pub account_id: AccountId,
    pub credential: ProtectedCredentialMaterial,
}
pub struct RefreshAuthenticationResult {
    pub updated_credential: ProtectedCredentialMaterial,
}
pub struct RevokeAuthenticationRequest {
    pub account_id: AccountId,
    pub credential: ProtectedCredentialMaterial,
}
pub struct RevokeAuthenticationResult {}
pub struct AuthAttemptHandle { /* nonce/PKCE/state privados da sessão */ }
pub struct AuthenticationCallback { /* callback verificado, sem URL crua em logs */ }
pub struct ProtectedCredentialMaterial { /* apenas transferência interna controlada */ }
```

**Fronteira de segredos:** `ProtectedCredentialMaterial` é valor **sensível e não serializável pela API de domínio**, acessível só ao serviço de contas/autenticação e ao adaptador autenticado de Providers; não é exposto à GUI, Detector, Inventário ou operador de transferências. `accounts/auth` armazena os segredos no keyring (serviço do sistema operacional), não em `inventory.sqlite3` ou registros de contas. A implementação Google verifica callback, `state`, PKCE, identidade OIDC e renovação por protocolo. Não prometer que todos os provedores têm OAuth, refresh token ou revogação remota.

**Pendência técnica:** decidir a representação segura do handle/segredo e fluxo async/callback antes de compilar Rust. Os structs vazios são **posições contratuais**, não implementações.

## 4. Porta de observação remota para Detector

```rust
pub trait ProviderRemoteObservation {
    fn list_scope(&self, request: ListRemoteScopeRequest)
        -> ProviderResult<ListRemoteScopeResult>;
    fn read_metadata(&self, request: ReadRemoteMetadataRequest)
        -> ProviderResult<ReadRemoteMetadataResult>;
    fn list_changes(&self, request: ListRemoteChangesRequest)
        -> ProviderResult<ListRemoteChangesResult>;
}

pub struct ListRemoteScopeRequest {
    pub remote_root: RemoteFileRef,
    pub continuation: Option<String>,
}
pub struct ListRemoteScopeResult {
    pub files: Vec<RemoteFile>,
    pub continuation: Option<String>,
    pub coverage: ProviderObservationCoverage,
}
pub struct ReadRemoteMetadataRequest { pub file: RemoteFileRef }
pub struct ReadRemoteMetadataResult {
    pub file: Option<RemoteFile>,
    pub coverage: ProviderObservationCoverage,
}
pub struct ListRemoteChangesRequest {
    pub remote_root: RemoteFileRef,
    pub cursor: Option<String>,
    pub continuation: Option<String>,
}
pub struct ListRemoteChangesResult {
    pub changes: Vec<ProviderChange>,
    pub next_page: Option<String>,
    pub checkpoint_candidate: Option<String>,
    pub coverage: ProviderObservationCoverage,
}
pub struct ProviderChange {
    pub resource: Option<RemoteFileRef>,
    pub metadata: Option<RemoteFile>,
    pub signal: ProviderChangeSignal,
}
pub enum ProviderChangeSignal {
    Changed,
    RemovedOrInaccessible, // NÃO implica exclusão comprovada
    RecheckRequired,
}
pub enum ProviderObservationCoverage {
    Complete,
    Partial,
    Unavailable,
}
```

**Separação rigorosa:** Providers consulta o serviço e converte a resposta para modelos normalizados; o **Detector Remoto** decide periodicidade, escopo, cursor aceito e quando produzir `File`, `FileEvent`, `FileBatch` e `EventStore`. O checkpoint durável **pertence ao Detector Remoto**, jamais a Providers. O cursor é opaco para o consumidor: Providers não o persiste e só oferece um candidato após leitura de uma página. Resposta parcial, `RemovedOrInaccessible` ou HTTP 403/404 **não** autorizam exclusão.

A conversão `GoogleFile → RemoteFile` e encapsulamento de `ProviderFileData` pertencem à implementação `providers/google/`. O Detector não importa tipos Google. Operações como Changes API são detalhes internos, não métodos públicos com nomes de serviço.

## 5. Porta de arquivos e operações autorizadas

```rust
pub trait ProviderRemoteFiles {
    fn apply_metadata(&self, request: ApplyRemoteMetadataRequest)
        -> ProviderResult<ApplyRemoteMetadataResult>;
    fn create_directory(&self, request: CreateRemoteDirectoryRequest)
        -> ProviderResult<CreateRemoteDirectoryResult>;
    fn trash(&self, request: TrashRemoteFileRequest)
        -> ProviderResult<TrashRemoteFileResult>;
    fn delete(&self, request: DeleteRemoteFileRequest)
        -> ProviderResult<DeleteRemoteFileResult>;
}
pub struct ApplyRemoteMetadataRequest {
    pub file: RemoteFileRef,
    pub expected_version: Option<String>,
    pub change: RemoteMetadataChange,
}
pub struct RemoteMetadataChange {
    pub rename_to: Option<String>,
    pub move_to: Option<RemoteFileRef>,
}
pub struct ApplyRemoteMetadataResult { pub file: RemoteFile }
pub struct CreateRemoteDirectoryRequest {
    pub parent: RemoteFileRef,
    pub name: String,
}
pub struct CreateRemoteDirectoryResult { pub file: RemoteFile }
pub struct TrashRemoteFileRequest {
    pub file: RemoteFileRef,
    pub expected_version: Option<String>,
}
pub struct TrashRemoteFileResult { pub file: Option<RemoteFile> }
pub struct DeleteRemoteFileRequest {
    pub file: RemoteFileRef,
    pub expected_version: Option<String>,
}
pub struct DeleteRemoteFileResult {}
```

A estrutura `RemoteMetadataChange` é **deliberadamente restrita**; atualizações de pais não podem ser convertidas em exclusões ou reupload sem prova de equivalência. Operações perigosas exigem precondições e decisões já tomadas pelo Analisador/Orquestrador, e `None` para versão esperada não significa autorização para sobrescrever. Provedores não decidem políticas de conflito nem deixam de validar permissões.

## 6. Porta de transferência para Upload e Download

```rust
pub trait ProviderTransfers {
    fn begin_upload(&self, request: BeginProviderUploadRequest)
        -> ProviderResult<ProviderUploadSession>;
    fn upload_part(&self, request: UploadProviderPartRequest)
        -> ProviderResult<ProviderTransferProgress>;
    fn finish_upload(&self, request: FinishProviderUploadRequest)
        -> ProviderResult<RemoteFile>;
    fn begin_download(&self, request: BeginProviderDownloadRequest)
        -> ProviderResult<ProviderDownloadSession>;
    fn download_part(&self, request: DownloadProviderPartRequest)
        -> ProviderResult<ProviderDownloadPart>;
}

pub struct BeginProviderUploadRequest {
    pub parent: RemoteFileRef,
    pub name: String,
    pub existing: Option<RemoteFileRef>,
    pub expected_version: Option<String>,
    pub total_bytes: Option<u64>,
}
pub struct ProviderUploadSession { /* handle opaco de transferência */ }
pub struct UploadProviderPartRequest {
    pub session: ProviderUploadSession,
    pub bytes: ProviderByteChunk,
    pub offset: u64,
}
pub struct FinishProviderUploadRequest { pub session: ProviderUploadSession }
pub struct BeginProviderDownloadRequest {
    pub file: RemoteFileRef,
    pub expected_version: Option<String>,
}
pub struct ProviderDownloadSession { /* handle opaco de transferência */ }
pub struct DownloadProviderPartRequest {
    pub session: ProviderDownloadSession,
    pub offset: u64,
    pub length: Option<u64>,
}
pub struct ProviderByteChunk { /* fluxo/chunk restrito, não materializar blob no banco */ }
pub struct ProviderDownloadPart { /* bytes e intervalo real retornado */ }
pub struct ProviderTransferProgress {
    pub confirmed_bytes: u64,
    pub complete: bool,
}
```

**Upload/Download** continuam responsáveis pelo planejamento e execução funcional, filas de prioridade, retomada e verificação final. **Providers** executa o protocolo remoto de transferência, informa capacidades e preserva offsets/condições; não mantém a fila de operações do Inventário. O desenho de chunks, cancelamento, streams, retomada e integridade é **provisório**, sujeito ao suporte real do provedor e à revisão de assincronicidade/ownership.

## 7. Erros e garantias da fronteira

```rust
pub type ProviderResult<T> = Result<T, ProviderFailure>;
pub enum ProviderError {
    UnknownProvider,
    UnsupportedCapability,
    AuthenticationRequired,
    ReauthorizationRequired,
    PermissionDenied,
    NotFoundOrInaccessible,
    PreconditionFailed,
    RateLimited,
    QuotaExceeded,
    Network,
    Timeout,
    InvalidResponse,
    IncompleteListing,
    ExpiredCursor,
    IncompleteTransfer,
    UnsupportedOperation,
    Cancelled,
}
pub struct ProviderFailure {
    pub kind: ProviderError,
    pub provider_id: ProviderId,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
    pub message: String, // sanitizada, sem tokens nem corpo HTTP
}
```

- Erro de autenticação não é sinal de arquivo excluído. `NotFoundOrInaccessible` preserva a ambiguidade real.
- Paginação e completude são explícitas; nunca inferir fim de árvore de uma página.
- Sessão incompatível com `ProviderId`/`AccountId` é rejeitada antes da chamada externa.
- Credenciais ficam sob `accounts/auth`; Providers não abre banco de contas nem chave de usuário.
- Provedor retorna fatos e resultados de API, **não** `SyncOperation`, `SyncBaseline`, conflito resolvido, `FileBatch` ou confirmação de checkpoint.
- Não exigir que todo provedor implemente todos os recursos; consultar capacidades, rejeitar ausência e nunca criar retorno fictício.

## 8. Composição de `ProviderAccountPort` e visibilidade

A fachada `Providers` seleciona a implementação por `ProviderId`, valida a identidade da conta e fornece à aplicação **portas especializadas e opacas** de observação, operações remotas e transferência, restritas às capacidades da conta. A forma concreta para obter cada porta e emprestar `&self` é deliberadamente deixada para a implementação Rust — não se afirma que `ProviderAccountPort` implementa automaticamente os traits acima.

**Fronteira de consumidor:** `accounts/auth` usa `ProviderAuthentication`; Detector Remoto usa `ProviderRemoteObservation`; Upload/Download usam `ProviderTransfers`; executores de ações remotas usam `ProviderRemoteFiles`. Nenhum consumidor precisa conhecer o `GoogleSession` ou `GoogleFile` da API técnica. **Não criar compatibilidade** que mantenha chamadas diretas de `providers/google/api/` fora de `providers/`.

## 9. Decisões pendentes antes do código

1. Representação não vazante de `ProtectedCredentialMaterial` e `AuthorizedAccountHandle`; segurança de callback, keyring e reinício.
2. Assinaturas concretas async, streams, `Send/Sync`, cancelamento, handles por referência, limites de memória e arquivo.
3. Versões e semântica por provedor para `expected_version`, pais remotos, exclusão/lixeira e documentos nativos.
4. Granularidade das capacidades e `ProviderChange`, sem perder fidelidade nem extrapolar prova de exclusão.
5. Protocolo de upload/download retomável e testes com falhas reais do transporte.
6. Mapeamento dos 59 métodos Google existentes às **capacidades efetivamente utilizadas** sem expor o cliente inteiro para a engine.

Todas essas definições pertencem à implementação e revisão dos contratos; **não alegar código existente**.
