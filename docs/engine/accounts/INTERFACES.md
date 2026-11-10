# Accounts — INTERFACES

## 1. Limite e contrato público

`accounts/` administra **cadastro e ciclo de vida de contas** e a **guarda segura de credenciais associadas**. As operações de autenticação de cada serviço (OAuth/OIDC Google, refresh, revogação pela API, etc.) são disponibilizadas exclusivamente pela API específica em [Providers](../providers/INTERFACES.md). Accounts coordena a inclusão, associação e exclusão de uma conta, mas **não implementa protocolo de autenticação, endpoints, HTTP ou renovação de tokens**.

Não existe `accounts/auth/`. Armazenamento seguro de credenciais é uma responsabilidade **interna de Accounts**, não um submódulo público nem um segundo provedor de autenticação.

Os tipos abaixo são **especificações Rust propostas**, não código implementado. Imports e tipos finais de `AccountId`, `ProviderId`, `Timestamp` e de sessão serão acertados na implementação; não recriar IDs já definidos em [Models](../models/INTERFACES.md).

## 2. Dados públicos

```rust
pub type AccountsResult<T> = Result<T, AccountsFailure>;

pub struct Account {
    pub id: AccountId,
    pub provider: ProviderId,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub state: AccountState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub enum AccountState {
    Connected,
    ReauthorizationRequired,
    Unavailable,
}

pub enum AccountsError {
    InvalidRequest,
    UnsupportedProvider,
    NotFound,
    AlreadyExists,
    IdentityMismatch,
    AuthorizationFailed,
    SecureStorageUnavailable,
    CredentialMissing,
    ProviderUnavailable,
    AccountInUse,
    StorageUnavailable,
    Internal,
}
pub struct AccountsFailure {
    pub kind: AccountsError,
    pub retryable: bool,
    pub message: String, // sanitizada; nunca contém credenciais
}
```

`Account.state` é uma indicação derivada do cadastro e da condição conhecida de credenciais/provedor; **não é prova de conectividade atual**. A identidade lógica é `(provider, provider_user_id)`, nunca apenas e-mail ou nome. Credenciais e segredos não aparecem em `Account`, eventos, atividades, logs ou valores de erro.

## 3. Requisições e respostas

```rust
pub struct AddAccountRequest {
    pub provider: ProviderId,
    pub request_id: Option<String>, // correlação opcional do registro em andamento
    // Entrada de autorização específica obtida pela API do provedor,
    // não um token bruto nem um protocolo OAuth reimplementado em Accounts.
    pub authorization: ProviderAuthorizationHandle,
}
pub struct AddAccountResult { pub account: Account, pub created: bool }

// Apenas acompanhamento transitório da execução atual, não cadastro nem histórico.
pub enum AccountOperationKind { Adding, Removing, AcquiringSession, UpdatingAuthorization }
pub enum AccountOperationPhase {
    Validating,
    CheckingAssociations,
    AccessingSecureStorage,
    PersistingCredentials,
    PersistingMetadata,
    Finalizing,
}
pub struct ActiveAccountOperation {
    pub account_id: Option<AccountId>, // None antes de identificar/criar conta
    pub request_id: Option<String>,    // correlação de add, sem segredos
    pub kind: AccountOperationKind,
    pub phase: AccountOperationPhase,
}
pub struct AccountsStatusRequest {}
pub struct AccountsStatusResult {
    pub active_operations: Vec<ActiveAccountOperation>,
}

pub struct GetAccountRequest { pub account_id: AccountId }
pub struct GetAccountResult { pub account: Account }

pub struct ListAccountsRequest { pub provider: Option<ProviderId> }
pub struct ListAccountsResult { pub accounts: Vec<Account> }

pub struct UpdateAccountRequest {
    pub account_id: AccountId,
    pub display_name: Option<String>,
}
pub struct UpdateAccountResult { pub account: Account }

pub struct RemoveAccountRequest {
    pub account_id: AccountId,
}
pub struct RemoveAccountResult { pub removed: bool }

pub struct AcquireSessionRequest { pub account_id: AccountId }
pub struct AcquireSessionResult {
    pub session: ProviderSessionHandle,
}

pub struct UpdateAuthorizationRequest {
    pub account_id: AccountId,
    pub authorization: ProviderAuthorizationHandle,
}
pub struct UpdateAuthorizationResult { pub account: Account }
```

`ProviderAuthorizationHandle` e `ProviderSessionHandle` são **placeholders conceituais de integração**, não tipos universais impostos a `providers/`. Sua concretização deve preservar a interface particular de cada provedor sem divulgar tokens. O fluxo externo de consentimento (navegador/callback, PKCE, troca de código, refresh, revogação) pertence ao Provider correspondente. `AcquireSession` apenas disponibiliza uma referência segura/escopada para o consumidor autorizado chamar esse Provider, sem revelar segredos.

`UpdateAccountRequest` altera apenas apresentação local; o e-mail e o ID externo são provenientes do provedor, não arbitrariamente editáveis pelo usuário. A operação de exclusão não implica revogação remota ou exclusão de arquivos sem solicitação/contrato explícito; se existir sincronização vinculada, devolver `AccountInUse` até resolução pela responsabilidade proprietária das conexões.

## 4. Assinaturas públicas propostas

```rust
impl Accounts {
    pub fn new(config: AccountsConfig) -> AccountsResult<Self>;

    pub async fn add(&self, request: AddAccountRequest) -> AccountsResult<AddAccountResult>;
    pub fn status(&self, request: AccountsStatusRequest) -> AccountsStatusResult;
    pub fn get(&self, request: GetAccountRequest) -> AccountsResult<GetAccountResult>;
    pub fn list(&self, request: ListAccountsRequest) -> AccountsResult<ListAccountsResult>;
    pub fn update(&self, request: UpdateAccountRequest) -> AccountsResult<UpdateAccountResult>;
    pub async fn remove(&self, request: RemoveAccountRequest) -> AccountsResult<RemoveAccountResult>;
    pub async fn acquire_session(&self, request: AcquireSessionRequest) -> AccountsResult<AcquireSessionResult>;
    pub async fn update_authorization(&self, request: UpdateAuthorizationRequest) -> AccountsResult<UpdateAuthorizationResult>;
}
```

Os métodos usam **um objeto de requisição** e retornam objeto de resultado, no padrão da engine. A escolha de sincronicidade e integração concreta será validada antes de compilar; não atribuir operações de rede a `get`, `list` ou `update`.

## 5. Acompanhamento operacional

**Separação obrigatória:** `Account.state` (`Connected`, `ReauthorizationRequired`, `Unavailable`) representa a condição **conhecida da conta e das credenciais**, não a atividade em execução, a disponibilidade instantânea da rede, nem o andamento do OAuth no navegador. `Accounts::status` informa operações **já iniciadas e ainda em andamento nesta instância** de Accounts; `active_operations=[]` não significa que todas as contas estejam conectadas nem que não haja autenticação externa.

As operações `add`, `remove`, `acquire_session` e `update_authorization` são assíncronas pelo acesso potencialmente demorado ao keyring, Provider ou checagem de associações. Reportam internamente fases efetivamente alcançadas; nenhuma fase é percentual nem confirmação de persistência concluída. `get`, `list` e `update` são operações locais delimitadas; não criar progresso artificial para elas. A escolha de executor/worker para I/O potencialmente bloqueante deverá preservar a responsividade GTK.

O `status()` é síncrono e rápido, sem consultar banco, Secret Service ou rede. É apenas um snapshot de `ActiveAccountOperation`, sem segredos, handles, tokens ou URLs de autorização. As operações ativas são retiradas do snapshot **em todos os desfechos** (`Ok`, `Err` e cancelamento, se houver); resultados e erros retornam pelo método original. O módulo **não guarda fila nem histórico de tarefas**, e não expõe `start`, `pause`, `resume` ou `stop`.

`AddAccountRequest.request_id` é identificador opcional de correlação fornecido pelo chamador, útil para acompanhar `add` antes de haver `AccountId`. Quando ausente, `request_id=None` não impede a operação; a identificação da conta aparece assim que for verificada, se disponível. IDs de correlação são opacos e não devem conter segredos. A API não promete rastreamento durável de fluxos externos de consentimento: Accounts recebe a autorização técnica já obtida pela API especializada do Provider.

**Consulta e falhas:** `get/list` e `Account.state` continuam sendo as fontes de situação conhecida das contas. Falhas concretas das operações são comunicadas por `AccountsFailure`, inclusive `SecureStorageUnavailable` e `CredentialMissing`, não convertidas em estado global artificial do módulo. `Unavailable` não é uma sonda de rede. Uma falha de keyring não deve marcar automaticamente todas as contas desconectadas ou remover credenciais.

## 6. Comportamento por método

| Método | Ação e retorno | Erros relevantes |
| --- | --- | --- |
| `new` | Preparar acesso ao cadastro e armazenamento seguro, sem iniciar OAuth | Configuração/storage indisponível |
| `status` | Consultar snapshot transitório de operações assíncronas ativas, sem I/O ou segredos | Sem erro de rede/keyring; não consulta dependências |
| `add` | Consumir resultado autorizado pelo Provider, obter identidade externa verificada, cadastrar conta e guardar credenciais com referência segura; repetição comprovadamente idêntica é idempotente | Provider indisponível, credencial inválida, identidade duplicada, keyring inacessível |
| `get` | Consultar conta por ID estável, sem segredos | `NotFound` |
| `list` | Listar contas e filtrar opcionalmente pelo provedor, sem consultar rede | `StorageUnavailable` |
| `update` | Alterar metadados locais permitidos, mantendo identidade externa | `InvalidRequest`, `NotFound` |
| `remove` | Verificar vínculos externos por contrato, remover cadastro e segredos locais somente após validações; sem exclusões de arquivos | `AccountInUse`, storage/segredos indisponíveis |
| `acquire_session` | Recuperar referência de autorização necessária ao Provider específico, mantendo segredos encapsulados | `ReauthorizationRequired` como `AuthorizationFailed`, `CredentialMissing`, `SecureStorageUnavailable` |
| `update_authorization` | Atualizar material de credencial após novo fluxo do Provider, verificando **mesma identidade externa** antes de substituir | `IdentityMismatch`, `AuthorizationFailed`, storage indisponível |

`add`/atualizações credenciais são operações com **duas persistências distintas** (cadastro e keyring). Não há transação atômica universal: projetar compensação, idempotência e limpeza de registros parciais, sem expor segredos. Não declarar conta `Connected` com credencial não persistida.

## 7. Limites de integração

- **Provider:** operações OAuth/OIDC, identidades externas, tokens e renovação específicos; Accounts não duplica a API e não exige um catálogo/fábrica universal que o contrato Providers não possui.
- **Armazenamento seguro:** Secret Service/keyring (ou equivalente apropriado) guarda segredos em escopo do aplicativo. Accounts possui o vínculo e a política de acesso local. Não registrar tokens em SQLite, configurações em texto claro ou logs.
- **Conexões:** o cadastro de sincronizações ainda tem proprietário pendente; não criar tabelas, CRUD ou referências cruzadas implícitas em Accounts. Excluir conta exige conhecer uso por contrato explícito quando implementado.
- **Inventário, Scanner, Detector, Analisador:** não acessam armazenamento de contas ou tokens diretamente; recorrem às APIs autorizadas.
- **Recuperação:** falha de credenciais ou keyring indisponível não é "corrupção do inventário". Se o cadastro adotar persistência local, **seu armazenamento** define as regras seguras de integridade e recuperação com padrões próprios; [Configuration](../configuration/README.md) é auxiliar/opcional e pode oferecer preferências por demanda, **nunca** exportação automática de secrets.

## 8. Decisões concretas da implementação

- `ProviderAuthorizationHandle::google(GoogleTokenResponse)` consome o resultado técnico já obtido por Providers. O handle pertence a Accounts, tem campos privados, não é serializável pela API pública e seu Debug é redigido. Não é exigido um novo tipo em Providers.
- `ProviderSessionHandle::google()` empresta `&GoogleSession`, sem extrair tokens. Accounts usa `get_userinfo` para verificar identidade e `refresh_access_token` para renovar; toda comunicação Google permanece em Providers, que não será alterado.
- `AccountsConfig::new(directory, application_id, associations)` configura cadastro JSON privado e Secret Service. `with_google(client, oauth_config)` habilita Google. Sem configuração, Google retorna `UnsupportedProvider`; outros serviços não são forçados a implementar OAuth.
- `AccountId`, `ProviderId` e `Timestamp` terão um único proprietário em `engine::models`, com reexportação na raiz; nenhum modelo de arquivo será implementado nesta tarefa.
- `AccountAssociations::lock_for_removal(account_id)` retorna uma guarda que impede novas associações até Drop, ou `AccountInUse`. O proprietário das conexões implementa esse contrato; Accounts não cria cadastro de sincronizações. Falha ou ausência de comprovação bloqueia a remoção. Não há implementação permissiva padrão.
- O cadastro usa JSON versionado, substituição atômica, fsync e permissões privadas. Um lock exclusivo impede duas instâncias de abrir o mesmo cadastro. Corrupção/formato incompatível não são resetados. Configuration não é necessário.
- Antes de alterar secrets, persiste-se uma intenção sem segredo. Credenciais são versionadas em itens distintos no keyring. A publicação do novo cadastro ocorre somente após a persistência segura. A intenção retém referências para limpeza ou retomada após falha/reabertura; `get/list` mostram `Unavailable` enquanto a conta está em transição.
- Operações assíncronas executam em worker Tokio e serializam mutações por instância. Cancelar a espera retira a atividade do status, mas não desfaz I/O já iniciado: o worker termina a unidade durável antes de liberar a exclusividade. Não há fila durável de tarefas. `new/get/list/update` são locais; chamadores GTK devem executá-los fora do loop principal quando envolverem disco.
- Recuperação é tentada pelas operações assíncronas antes de iniciar outra mutação. Secret ausente em inclusão interrompida descarta somente o registro incompleto; em substituição preserva a versão anterior; remoção interrompida é retomada. Referências não são descartadas quando a limpeza segura falha.
- `update(display_name=None)` limpa o nome local; identidade/e-mail permanecem. Remover ID inexistente retorna `removed=false`. Adicionar identidade já cadastrada retorna `created=false` e não substitui credenciais; a substituição usa `update_authorization`.
- Estado é conhecimento persistido: ausência de credencial/reautorização exige `ReauthorizationRequired`; falha transitória de keyring não altera todas as contas. Rede não é consultada por `get/list`. Refresh sem novo refresh token conserva o anterior, somente para a mesma conta; autorizações sem refresh token permitem acesso até expirar, quando a ausência de renovação exige reautorização. Atualização da mesma identidade sem novo refresh token conserva o anterior quando disponível.

**Estado:** contrato de implementação; evidências de execução serão registradas em TESTS.md.
