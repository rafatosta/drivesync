# Providers — INTERFACES

## Papel da interface

`providers/` organiza a exposição das APIs específicas dos serviços externos. Não existe obrigação de um contrato universal de métodos, `trait`, catálogo dinâmico, fábrica, `ProviderId` ou `ProviderApi`. A forma concreta de exportação em Rust só será escolhida durante a implementação.

Cada submódulo expõe as operações correspondentes à API oficial que representa. Seus métodos são descritos em sua própria documentação de interface; não devem ser replicados aqui.

## Fronteira técnica

Uma função do provedor deve corresponder a uma operação ou mecanismo técnico documentado do serviço: requisição, parâmetros, autenticação exigida, resposta e erro. Lógica de orquestração, decisão funcional, gerenciamento de transferência ou armazenamento de credenciais permanece fora da API.

## Sincronia, I/O e estado técnico

A **assinatura de cada operação pertence ao provedor específico**. Não adicionar `ProviderState`, `ProviderStatus`, `ProgressReporter`, job manager ou `trait` compartilhados. Uma chamada pode ser assíncrona por realizar HTTP, ser síncrona se for transformação pura, ou devolver um stream cuja leitura permanece assíncrona/bloqueante conforme a implementação. Nenhuma função que espera I/O externo deve executar diretamente na thread GTK.

O estado observável do provedor é **o resultado técnico da chamada**, não um estado operacional permanente do serviço: resposta, erro tipado, páginas/cursor, progresso de upload confirmado pela API, intervalo de download e estado de operação remota quando estes forem previstos oficialmente. Não transformar `HTTP 308`, `operation.done=false` ou bytes recebidos em conclusão da sincronização.

Parâmetros de cancelamento, timeouts ou interrupção do transporte serão especificados por cada API conforme necessidades concretas. O provedor reporta eventual cancelamento sem prometer reversão de efeitos externos. Não manter uma segunda fila, baseline ou acompanhamento por arquivo.

## API documentada

- [Google — INTERFACES](google/INTERFACES.md): operações e tipos específicos Google Drive REST v3, OAuth/OIDC e transporte necessário às chamadas, conforme os contratos documentados.

**Estado:** interface documental; não presume que assinaturas já estejam implementadas em Rust.
