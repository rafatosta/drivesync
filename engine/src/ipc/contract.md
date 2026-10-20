# Contrato IPC — status externo

## Propósito e consumidores

Interface externa **opcional** no D-Bus de sessão para extensão nativa do Nautilus. O aplicativo GTK chama o motor diretamente pela fachada Rust.

## Operações conceituais

- `GetFileStatus(uri) → FileSyncStatus`
- `GetFileStatuses(uris[]) → FileSyncStatus[]` (otimização de lote)
- `GetSyncRoots() → SyncRootSummary[]`
- `FileStatusChanged(uri, status, generation)` (sinal)

Status: `SYNCED`, `SYNCING`, `PENDING`, `CONFLICT`, `ERROR`, `UNKNOWN`, `UNAVAILABLE`, `NOT_MANAGED`.

## Pré-condições

Nome D-Bus adquirido, motor ativo e snapshot conhecido. Requisições de cliente externo não autenticam ou modificam contas.

## Restrições e garantias

Resposta deve ser rápida, em memória/estado local, sem chamada de rede, sem exposição de conteúdo ou tokens. Se o estado é obsoleto/desconhecido, nunca devolver `SYNCED`. API versionada e tipada, com limites de lote e respostas a falhas claras. Sinais são indicativos; consulta de snapshot é fonte autoritativa.

## Falhas

`UNAVAILABLE` quando o processo não existir; `UNKNOWN` para estado inconclusivo; `NOT_MANAGED` para caminhos externos. Cliente deve aceitar timeout e desconexão sem bloquear Nautilus.

## Escopo

Não implementar comandos de sincronização ou decisões destrutivas neste contrato inicial.
