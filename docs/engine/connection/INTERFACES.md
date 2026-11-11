# Connection — contratos públicos

## Superfície

`Engine`, configuração, requisições/resultados e erros são reexportados por `drivesync_engine`. O front-end não importa `accounts`, `providers`, modelos de tokens ou suas implementações. Os módulos existentes continuam com seus contratos de componente para testes/consumidores já existentes; essa não é uma autorização para GTK acessá-los diretamente. Não alterar sua visibilidade e quebrar contratos de Providers neste trabalho.

```rust
pub struct EngineConfig {
    pub data_directory: PathBuf,
    pub application_id: String,
    pub google_oauth_file: Option<PathBuf>,
}
pub struct GoogleSignInId { /* opaque */ }
pub struct BeginGoogleSignInRequest { pub account_id: Option<AccountId> }
pub struct BeginGoogleSignInResult {
    pub connection_id: GoogleSignInId,
    pub authorization_url: String,
}
pub struct FinishGoogleSignInRequest { pub connection_id: GoogleSignInId }
pub struct FinishGoogleSignInResult { pub account: Account, pub created: bool }
pub struct CancelGoogleSignInRequest { pub connection_id: GoogleSignInId }
pub struct CancelGoogleSignInResult { pub cancelled: bool }

impl Engine {
    pub async fn open(config: EngineConfig) -> EngineResult<Self>;
    pub async fn begin_google_sign_in(&self, request: BeginGoogleSignInRequest) -> EngineResult<BeginGoogleSignInResult>;
    pub async fn finish_google_sign_in(&self, request: FinishGoogleSignInRequest) -> EngineResult<FinishGoogleSignInResult>;
    pub fn cancel_google_sign_in(&self, request: CancelGoogleSignInRequest) -> CancelGoogleSignInResult;
    pub async fn list_accounts(&self, request: ListAccountsRequest) -> EngineResult<ListAccountsResult>;
    pub async fn update_account(&self, request: UpdateAccountRequest) -> EngineResult<UpdateAccountResult>;
}
```

## Semântica

- `open`: prepara executor e Accounts, sem autenticar usuário, abrir navegador ou acessar keyring. Diretório absoluto e namespace não vazio são obrigatórios. Erros locais são tipados; não resetar o cadastro. OAuth ausente/inválido torna conexão indisponível, preservando consulta local. Configuração e credenciais não são retornadas.
- `begin`: `account_id=None` adiciona uma conta; `Some(id)` prepara reautorização da mesma conta e exige sua existência. Gera identificadores aleatórios e PKCE S256 privados, abre listener somente loopback e obtém a URL por Providers. O navegador é responsabilidade de GTK. Uma tentativa expira em cinco minutos, mesmo se finish nunca for chamado. Não solicita escopos de arquivos: somente openid/email/profile.
- `finish`: uma única chamada pode consumir a tentativa. Aguarda callback local sem bloquear GTK, valida método/caminho/estado e rejeita parâmetros duplicados ou ambíguos. Retornos inválidos não acionam a troca de código. A troca é feita uma vez por Providers, sem retry automático de resultado incerto. Depois encapsula a resposta para Accounts::add ou update_authorization. Essa entrega é o ponto a partir do qual cancelamento não promete interromper trabalho durável. Só retorna sucesso após a persistência segura. Contas repetidas retornam created=false. Tokens nunca chegam ao front-end.
- `cancel`: idempotente; retorna false se já encerrado ou se a etapa de cadastro/atualização já foi entregue a Accounts. Antes disso, interrompe espera/troca e impede iniciar cadastro. Não revoga tokens, remove conta nem promete rollback de escrita em andamento. IDs são opacos e específicos da instância.
- `list_accounts`: consulta local, sem sessão Google, rede ou keyring. Filtro None inclui todas; ausência retorna lista vazia.
- `update_account`: modifica exclusivamente nome local; None limpa o nome. E-mail e identidade não são editáveis. Salvar é ação explícita; falha mantém o rascunho na interface.

`Account`, `AccountId`, `AccountState`, requisições e resultados locais conservam os contratos de Accounts, reexportados na raiz. Não há novo modelo de conta duplicado.

## Erros

`EngineError { kind, retryable, message }` usa categorias sanitizadas: InvalidConfiguration, AuthorizationUnavailable, AttemptNotFound, AlreadyInProgress, Cancelled, TimedOut, AuthorizationDenied, InvalidCallback, ProviderUnavailable, Accounts(AccountsError), Internal. Não incorpora URL privada, código, token, corpo HTTP, arquivo de configuração nem erro textual de dependências.

A URL pública contém os parâmetros de autorização necessários ao navegador, nunca access/refresh token ou verificador PKCE. Não registrar a URL nem o callback em logs. `GoogleSignInId` não oferece construtor público nem expõe seu conteúdo.
