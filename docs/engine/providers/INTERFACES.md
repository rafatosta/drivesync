# Providers — INTERFACES: catálogo e fornecimento de APIs específicas

> **Contrato conceitual, não compilado.** O módulo `providers/` **não define interface universal de arquivos, OAuth, transferência ou mudanças**. Fornece seleção e acesso à API própria de cada serviço. A interface funcional padronizada é definida em cada consumidor e implementada por seus especialistas.

## 1. Fronteira única de acesso a serviços

- Apenas clientes/API fornecidos por `providers/` atravessam a fronteira HTTP/OAuth de serviços remotos.
- `accounts/auth`, Detector e Operações escolhem o provedor **por identidade de conta ou configuração** e obtêm uma API; não criam cliente HTTP avulso.
- Um especialista **pode importar/usar tipos do serviço**, como `GoogleFile`, `GoogleChange` e `GoogleSession`, quando necessário ao seu trabalho. Ele não pode expor esses tipos na interface comum do módulo pai.
- `ProviderId` e `AccountId` pertencem aos modelos compartilhados; não redefini-los.
- `example/` não entra no catálogo enquanto não houver implementação.

## 2. Interface pública mínima

```rust
// Proposta de superfície, nomes ilustrativos; retorno tipado concreto a revisar.
pub trait Providers {
    fn list(&self) -> Vec<ProviderInfo>;
    fn get(&self, provider: ProviderId)
        -> ProviderResult<ProviderHandle>;
    fn open(&self, account: AccountId)
        -> ProviderResult<ProviderSession>;
}

pub struct ProviderInfo {
    pub id: ProviderId,
    pub display_name: String,
}

pub enum ProviderHandle {
    Google(GoogleApiHandle),
    // Variantes futuras somente quando houver implementação real.
}

pub enum ProviderSession {
    Google(GoogleAccountApiHandle),
}

pub struct GoogleApiHandle {
    // Referência segura à interface especializada providers/google/api/.
}
pub struct GoogleAccountApiHandle {
    // API Google e contexto autenticado vinculados a AccountId.
}
```

`GoogleApiHandle`, `GoogleAccountApiHandle` e as variantes são **esboços**, não novas implementações de API. Referenciam o contrato já documentado em [Google API — INTERFACES](google/api/INTERFACES.md). Um `enum` permite representar retornos de tipos diferentes sem fingir que possuem os mesmos métodos. Outra estratégia tipada é aceitável se preservar a seleção explícita e o erro de incompatibilidade.

**Semântica:**
- `list()`: somente provedores instalados/registrados; informações de apresentação, **não** promessa de capacidades inexistentes.
- `get(provider)`: obtém a interface concreta para operações que independem de uma conta autenticada (por exemplo, iniciar OAuth do Google). Não autentica nem registra conta.
- `open(account)`: resolve o provedor da conta cadastrada por um **contexto autorizado fornecido por Accounts**, valida par `AccountId/ProviderId` e retorna o cliente/sessão específicos. A forma de fornecer esse contexto sem circularidade entre Accounts e Providers **ainda precisa ser definida**; não permitir que Providers leia diretamente a persistência privada de contas.
- Não criar `ProviderRemoteObservation`, `ProviderRemoteFiles`, `ProviderTransfers` ou `ProviderAuthentication` como requisitos comuns de todos os serviços.

## 3. Exemplo de obtenção por especialista

```rust
// Pseudocódigo Rust — apenas demonstra a dependência tipada.
let api = match providers.open(account_id)? {
    ProviderSession::Google(google) => google,
};

let changes = api.changes_list(/* GoogleSession, opções Google */)?;
let normalized = google_specialist.to_remote_file(changes);
// O especialista satisfaz o contrato de seu módulo pai.
```

**Não** exigir que `providers/` transforme `GoogleFile` em `RemoteFile` por meio de API comum. A transformação de modelos pertence ao especialista que consome a API, e o módulo pai exige apenas sua saída padronizada. O contrato técnico de Google API continua acessível ao especialista **após obtenção pela fábrica**.

## 4. Contrato obrigatório pertence ao módulo consumidor

| Módulo pai | Entrada do especialista | Saída obrigatória |
| --- | --- | --- |
| `accounts/auth` | Provedor selecionado, contexto de consentimento/identidade | Resultado autenticado, identidade verificada e credencial protegida conforme contrato de Accounts |
| `detectors/remote` | `SyncId`, raiz, sessão/API e checkpoint injetados | `File` completo via `RemoteEventSink`; `RemotePollResult` conforme [RemoteDetector](../detectors/remote/INTERFACES.md) |
| `operations/upload` | Operação autorizada e conta correspondente | Resultado/progresso de upload conforme contrato do operador (a definir) |
| `operations/download` | Operação autorizada e conta correspondente | Resultado/progresso de download conforme contrato do operador (a definir) |

O especialista Google pode usar os recursos do Google Drive livremente **dentro dos limites funcionais e de segurança** de seu módulo pai. Um futuro especialista OneDrive pode ter estratégia e API totalmente diferentes. Não gerar interfaces universais artificiais só para padronizar a chamada ao provedor.

## 5. Autenticação e segredos

`accounts/auth/` coordena fluxo da conta, autorização e armazenamento de credenciais no keyring. `providers/google/api/` implementa `authorization_request`, `exchange_authorization_code`, `refresh_access_token` e outros métodos específicos do Google. O especialista de autenticação os utiliza pela API obtida de Providers.

`get` **não retorna tokens**; `open` retorna sessão vinculada à conta, sem revelar segredos a módulos que não precisam deles. Se uma implementação exigir contexto credencial, ele será injetado via contrato seguro com `accounts/auth`, não buscado no SQLite de Inventory/Accounts por acesso direto. Evitar logs, clones e serialização de segredos; tratar cancelamento, callback inválido e reautorização.

## 6. Identidades, erros e comportamento

```rust
pub type ProviderResult<T> = Result<T, ProviderLookupError>;

pub enum ProviderLookupError {
    UnknownProvider,
    UnsupportedProviderForAccount,
    AccountNotAuthorized,
    AccountProviderMismatch,
    SessionUnavailable,
}
```

São **erros de catálogo/seleção**, não substituem os erros ricos da API Google ou do contrato do módulo funcional. O especialista mapeia os erros de protocolo para o contrato do seu módulo, sem traduzir falsamente 403/404 em exclusão.

- `ProviderId` identifica o tipo de serviço; `AccountId` identifica uma conexão autenticada com aquele serviço.
- Falha de seleção/autorização nunca retorna uma sessão de outra conta, nem outro provedor como fallback silencioso.
- A fábrica não avança checkpoint, não grava `FileBatch`, não executa upload/download por conta própria nem decide conflito.
- O consumidor não pode contornar Providers criando outro cliente diretamente, mesmo que conheça a API específica.

## 7. Limites e pendências da implementação Rust

1. Escolher `enum`, handle tipado ou factory genericamente parametrizada, considerando dependências e ownership de Rust.
2. Resolver autorização de conta para `open(AccountId)` sem fazer Providers depender da persistência de Accounts. Uma porta de sessão/identity pode ser injetada na composição.
3. Separar objeto API de sessão por conta sem expor credenciais ou tokens.
4. Decidir `Send/Sync`, assincronicidade, cancelamento e comportamento das sessões em renovação/revogação.
5. Documentar contrato funcional concreto de `accounts/auth`, `operations/upload` e `operations/download`; nenhum deles pode ser inventado aqui.

Nenhuma assinatura acima representa código implementado.
