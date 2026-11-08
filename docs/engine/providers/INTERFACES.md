# Providers — INTERFACES

> Contrato **conceitual** do catálogo e da obtenção de APIs específicas, ainda não compilado em Rust. Os métodos técnicos de cada serviço são definidos **somente no contrato de sua API**.

## 1. Responsabilidade da interface

O catálogo deve permitir:

1. Listar implementações de serviços externos **realmente disponíveis**.
2. Selecionar um serviço por `ProviderId`.
3. Obter a interface tipada específica desse serviço.

```rust
// Pseudocódigo contratual; representação concreta de Rust a decidir.
pub trait Providers {
    fn list(&self) -> Vec<ProviderInfo>;
    fn get(&self, provider: ProviderId)
        -> ProviderResult<ProviderApi>;
}

pub struct ProviderInfo {
    pub id: ProviderId,
    pub display_name: String,
}

pub enum ProviderApi {
    Google(GoogleApiHandle),
    // Somente provedores implementados.
}

pub struct GoogleApiHandle {
    // Interface já descrita em google/api/INTERFACES.md.
}

pub type ProviderResult<T> = Result<T, ProviderLookupError>;

pub enum ProviderLookupError {
    UnknownProvider,
    ProviderUnavailable,
    InitializationFailed,
}
```

`ProviderId` pertence aos modelos compartilhados. `GoogleApiHandle` é um **nome ilustrativo**, não uma interface alternativa. O código poderá empregar `enum`, fábrica tipada ou outra solução sem modificar o contrato funcional de cada API.

**Não exigir `open(AccountId)` como método universal de Providers.** Dados ou sessões para chamadas autenticadas são definidos pela API específica (por exemplo, `GoogleSession` nas assinaturas Google); Providers não mantém cadastro de contas nem resolve armazenamento de credenciais.

## 2. Interface entregue pelo provedor

O resultado de `get` fornece acesso às funções, tipos e erros **próprios do serviço**. A camada do provedor:

- Mapeia as operações oficiais para funções do aplicativo.
- Valida e transforma parâmetros e resultados de transporte.
- Encapsula construção de URLs, HTTP, serialização e peculiaridades do protocolo.
- Expõe erros técnicos que preservam a semântica do serviço.
- Implementa, quando a API oferece, operações de autenticação e uso de sessão.
- Não exige equivalência artificial com outros provedores.

A definição completa da API Google encontra-se em [`google/api/INTERFACES.md`](google/api/INTERFACES.md): **59 métodos Drive REST v3, 9 OAuth/OIDC e 10 de transferências/conveniências** já documentados. Não copiá-los para o catálogo.

## 3. Exemplo de seleção

```rust
// Pseudocódigo para demonstrar seleção, não API Rust compilada.
let api = match providers.get(provider_id)? {
    ProviderApi::Google(google) => google,
};
// A partir daqui estão disponíveis os métodos específicos Google.
// Cada chamada usa os argumentos descritos em google/api/INTERFACES.md.
```

A seleção entrega um cliente/interface; não executa automaticamente nenhuma operação externa, não autentica implicitamente e não armazena credenciais.

## 4. Responsabilidades excluídas

Providers não define o que fazer com resultados, quando realizar chamadas, qual estado funcional persistir ou quais passos compõem um fluxo do aplicativo. Não recebe nem interpreta estados ou decisões de sincronização.

## 5. Pendências de implementação

- Representação Rust de APIs heterogêneas (enum/handles/fábrica tipada), ownership, `Send/Sync` e async.
- Inicialização e injeção de configurações técnicas específicas de cada serviço.
- Como expor interfaces de clientes autenticados **sem gerenciar armazenamento de credenciais**.
- Política de falhas na inicialização dos provedores.

Essas pendências **não** autorizam criar uma camada universal de métodos de API.
