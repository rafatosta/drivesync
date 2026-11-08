# Google — INTERFACES

> Contrato **de disponibilização** da API Google, não uma segunda definição da API. Estado proposto, ainda não compilado.

## 1. Escopo

Quando o catálogo seleciona `ProviderId("google")`, este submódulo entrega uma interface concreta para as funções documentadas em [`api/INTERFACES.md`](api/INTERFACES.md).

```rust
// Representação ilustrativa; tipos concretos e ownership a revisar.
pub(crate) trait GoogleProviderFactory {
    fn create_api(&self) -> Result<GoogleApiHandle, GoogleProviderError>;
}

pub struct GoogleApiHandle {
    // Cliente tipado correspondente ao contrato existente em api/.
}

pub enum GoogleProviderError {
    InvalidConfiguration,
    InitializationFailed,
}
```

O `GoogleApiHandle` mencionado na [fábrica Providers](../INTERFACES.md) é **o mesmo conceito**, não um tipo a ser duplicado. O nome e o mecanismo Rust final dependem da implementação.

## 2. Interface técnica exposta

O módulo **não adiciona nem substitui** métodos técnicos. A interface disponibilizada corresponde às funções e estruturas já especificadas em [`api/INTERFACES.md`](api/INTERFACES.md):

| Área | Contrato existente |
| --- | --- |
| Drive REST v3 | 59 métodos em 13 recursos |
| OAuth 2.0 / OIDC | 9 métodos |
| Transferências e conveniências | 10 métodos |
| Tipos | `GoogleSession`, `GoogleFile`, `GoogleChange`, opções, requisições, respostas e erros Google |

A API específica recebe os parâmetros exigidos por cada método, inclusive contexto autenticado quando necessário. Não há necessidade de um método universal de sessão em `providers/` ou `providers/google/`.

## 3. Garantias

- A API fornecida pertence ao provedor Google e utiliza sua configuração técnica correta.
- A inicialização falha explicitamente quando inválida; não devolver handle sem funcionalidade.
- A implementação utiliza uma única definição técnica do cliente Google.
- Nenhum segundo conjunto de wrappers replica OAuth, Files, Changes ou transferências nesta pasta.
- O submódulo não armazena contas, credenciais nem estados funcionais.
- A interface não impõe métodos, tipos ou comportamentos às APIs de outros serviços.

## 4. Pendências

Representação concreta do handle, instalação/configuração técnica, ownership, concorrência e assincronicidade. Essas decisões não alteram a assinatura dos métodos já especificados em `api/`.
