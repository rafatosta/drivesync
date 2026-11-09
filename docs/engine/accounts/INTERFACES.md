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
    // Entrada de autorização específica obtida pela API do provedor,
    // não um token bruto nem um protocolo OAuth reimplementado em Accounts.
    pub authorization: ProviderAuthorizationHandle,
}
pub struct AddAccountResult { pub account: Account, pub created: bool }

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
    pub fn get(&self, request: GetAccountRequest) -> AccountsResult<GetAccountResult>;
    pub fn list(&self, request: ListAccountsRequest) -> AccountsResult<ListAccountsResult>;
    pub fn update(&self, request: UpdateAccountRequest) -> AccountsResult<UpdateAccountResult>;
    pub async fn remove(&self, request: RemoveAccountRequest) -> AccountsResult<RemoveAccountResult>;
    pub async fn acquire_session(&self, request: AcquireSessionRequest) -> AccountsResult<AcquireSessionResult>;
    pub async fn update_authorization(&self, request: UpdateAuthorizationRequest) -> AccountsResult<UpdateAuthorizationResult>;
}
```

Os métodos usam **um objeto de requisição** e retornam objeto de resultado, no padrão da engine. A escolha de sincronicidade e integração concreta será validada antes de compilar; não atribuir operações de rede a `get`, `list` ou `update`.

## 5. Comportamento por método

| Método | Ação e retorno | Erros relevantes |
| --- | --- | --- |
| `new` | Preparar acesso ao cadastro e armazenamento seguro, sem iniciar OAuth | Configuração/storage indisponível |
| `add` | Consumir resultado autorizado pelo Provider, obter identidade externa verificada, cadastrar conta e guardar credenciais com referência segura; repetição comprovadamente idêntica é idempotente | Provider indisponível, credencial inválida, identidade duplicada, keyring inacessível |
| `get` | Consultar conta por ID estável, sem segredos | `NotFound` |
| `list` | Listar contas e filtrar opcionalmente pelo provedor, sem consultar rede | `StorageUnavailable` |
| `update` | Alterar metadados locais permitidos, mantendo identidade externa | `InvalidRequest`, `NotFound` |
| `remove` | Verificar vínculos externos por contrato, remover cadastro e segredos locais somente após validações; sem exclusões de arquivos | `AccountInUse`, storage/segredos indisponíveis |
| `acquire_session` | Recuperar referência de autorização necessária ao Provider específico, mantendo segredos encapsulados | `ReauthorizationRequired` como `AuthorizationFailed`, `CredentialMissing`, `SecureStorageUnavailable` |
| `update_authorization` | Atualizar material de credencial após novo fluxo do Provider, verificando **mesma identidade externa** antes de substituir | `IdentityMismatch`, `AuthorizationFailed`, storage indisponível |

`add`/atualizações credenciais são operações com **duas persistências distintas** (cadastro e keyring). Não há transação atômica universal: projetar compensação, idempotência e limpeza de registros parciais, sem expor segredos. Não declarar conta `Connected` com credencial não persistida.

## 6. Limites de integração

- **Provider:** operações OAuth/OIDC, identidades externas, tokens e renovação específicos; Accounts não duplica a API e não exige um catálogo/fábrica universal que o contrato Providers não possui.
- **Armazenamento seguro:** Secret Service/keyring (ou equivalente apropriado) guarda segredos em escopo do aplicativo. Accounts possui o vínculo e a política de acesso local. Não registrar tokens em SQLite, configurações em texto claro ou logs.
- **Conexões:** o cadastro de sincronizações ainda tem proprietário pendente; não criar tabelas, CRUD ou referências cruzadas implícitas em Accounts. Excluir conta exige conhecer uso por contrato explícito quando implementado.
- **Inventário, Scanner, Detector, Analisador:** não acessam armazenamento de contas ou tokens diretamente; recorrem às APIs autorizadas.
- **Recuperação:** falha de credenciais ou keyring indisponível não é "corrupção do inventário". A política de backup de [Configuration](../configuration/README.md) aplica-se apenas à persistência escolhida e sua propriedade, **nunca** a exportação de secrets.

## 7. Pendências antes da implementação

1. Tipo de passagem de autorização/sessão que não imponha uma API universal a todos os provedores.
2. Contrato efetivo do proprietário de conexões para bloquear remoção com sincronizações ativas.
3. Persistência escolhida para cadastro de contas e integração específica do Secret Service/Flatpak.
4. Política de erro, compensação e idempotência entre cadastro e keyring.
5. Forma de derivar `AccountState` sem exigir rede para consultas locais.

**Estado:** documentação contratual proposta, não implementação Rust.
