# Providers

## Objetivo

Disponibilizar as APIs de serviços externos por meio de interfaces próprias, abstraindo seus endpoints, parâmetros, requisições e respostas.

Cada submódulo representa a API de um provedor.

## Execução e resultados

A interface técnica de cada provedor pode envolver **operações de rede demoradas**, fluxos de bytes e chamadas locais rápidas de validação ou adaptação. Cada método continua responsável apenas pela **operação externa específica** que representa: recebe parâmetros, executa a chamada e devolve resposta ou erro tipado. A implementação Rust não deve bloquear o loop principal GTK. A escolha de `async`, streaming e workers será feita conforme a natureza do I/O, sem exigir que todos os métodos sejam idênticos.

O provedor **não gerencia jobs**, não acompanha estados de `SyncItem`, não oferece `start/pause/resume/stop/status` global e não implementa progresso de sincronização da aplicação. Quando a API externa oferece respostas parciais, offset/range confirmado, páginas, operações remotas ainda pendentes ou `Retry-After`, o provedor preserva esses dados técnicos para o consumidor. A coordenação, as tentativas de transferência e o progresso de execução pertencem aos módulos que usam essas funções.

Falhas de autenticação, permissão, rede, limite de taxa, timeout, sessão expirada e resultado incerto devem ser distinguíveis por erros próprios de cada provedor, sem persistir credenciais ou decidir política de retry.

## Submódulos

- [Google](google/README.md) — API do Google Drive e serviços de autenticação Google associados.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)
