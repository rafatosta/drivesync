# Providers — INTERFACES

## Papel da interface

`providers/` organiza a exposição das APIs específicas dos serviços externos. Não existe obrigação de um contrato universal de métodos, `trait`, catálogo dinâmico, fábrica, `ProviderId` ou `ProviderApi`. A forma concreta de exportação em Rust só será escolhida durante a implementação.

Cada submódulo expõe as operações correspondentes à API oficial que representa. Seus métodos são descritos em sua própria documentação de interface; não devem ser replicados aqui.

## Fronteira técnica

Uma função do provedor deve corresponder a uma operação ou mecanismo técnico documentado do serviço: requisição, parâmetros, autenticação exigida, resposta e erro. Lógica de orquestração, decisão funcional, gerenciamento de transferência ou armazenamento de credenciais permanece fora da API.

## API documentada

- [Google — INTERFACES](google/INTERFACES.md): operações e tipos específicos Google Drive REST v3, OAuth/OIDC e transporte necessário às chamadas, conforme os contratos documentados.

**Estado:** interface documental; não presume que assinaturas já estejam implementadas em Rust.
