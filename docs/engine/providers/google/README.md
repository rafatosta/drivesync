# Google

## Objetivo

Disponibilizar a API do Google Drive, incluindo as operações de autenticação Google necessárias ao seu acesso, por meio de funções que abstraem a comunicação com os serviços externos.

## Execução e acompanhamento técnico

O catálogo documentado está implementado: **59 métodos REST, 9 OAuth/OIDC e 10 de transferências/conveniência**. Os 15 traits públicos são implementados por `GoogleDriveClient`; as chamadas de rede retornam futures `Send` e usam `reqwest` assíncrono sobre Tokio. Construção de parâmetros e URL de autorização permanecem síncronas, sem abrir navegador ou criar servidor de callback.

Metadados têm timeout total de 30 s e limite de 8 MiB. Downloads/exportações entregam chunks sob demanda, com falhas de leitura tipadas. Uploads retomáveis preservam intervalos confirmados em `308` e só retornam conclusão após resposta final coerente. A configuração OAuth é lida assincronamente, sem persistir tokens; ID tokens são verificados criptograficamente com chaves oficiais. Endpoints alternativos e ajustes de limites são exclusivos do feature de testes e restritos a loopback.

Os modelos são as projeções tipadas definidas em [INTERFACES.md](INTERFACES.md), não a reprodução de todos os campos do Discovery. O relatório de testes e seus limites estão em [TESTS.md](TESTS.md).

**Não há estado global de execução do Google Provider.** O status de cada requisição é sua resposta ou `GoogleApiFailure`. Paginação de listas e mudanças, `GoogleUploadProgress`, `GoogleDownloadStream` e `GoogleOperation` refletem apenas estados técnicos retornados pelo protocolo. O progresso completo de upload/download e o gerenciamento de operações são responsabilidades do Operador; este provedor não registra jobs, não faz reconciliação nem mantém fila.

Timeout, quota, `Retry-After`, autenticação expirada e efeitos remotos incertos devem ser traduzidos de forma fiel, sem supor que nova tentativa é sempre segura.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)
