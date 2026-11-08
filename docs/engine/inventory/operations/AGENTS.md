# Inventário / operations — instruções para agentes

> Aplicar junto de [inventory/AGENTS.md](../AGENTS.md) e deste submódulo: [README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md). Este é **um contrato documental proposto**, não uma alegação de implementação.

## Missão e limite de edição

Fila persistente `SyncOperation` de comandos de sincronização, separada da fila de observações do Detector.

**Pode atuar futuramente:** apenas nos arquivos Rust do submódulo `inventory/operations/` e testes pertinentes, **quando a implementação for explicitamente solicitada**. Nesta fase, não modificar código compilado, dados de usuário ou interfaces de outros módulos.

**Interfaces recebidas:** `NewSyncOperation`, `OperationFilter`, `Pagination`, `ClaimOperationRequest`, `UpdateOperationRequest`, `OperationId`, `SyncItemId`.

**Dependências aceitas:** Tipos do Inventário, contexto transacional de `storage/`, confirmação coordenada pela fachada.

**Proibições:** Não executa upload/download, não monitora filesystem ou API remota, não define política de conflito, não marca `Completed` por simples retorno positivo de executor.

## Regras contratuais

- `enqueue` — inserir operação persistente com revisão e chave de deduplicação, quando definida.
- `list` — consultar filas e estados com filtros e paginação.
- `claim` — reivindicar atomicamente uma operação elegível.
- `transition` — aplicar retry, bloqueio, falha ou requeue conforme contrato.
- `complete_verified` — conclusão somente durante `InventoryManager.confirm_sync`, na mesma transação de baseline e histórico.
- Manter a única API pública em `InventoryManager` (16 métodos). As assinaturas internas de `INTERFACES.md` são propostas; validar ownership/lifetimes de Rust antes de transformar em código.
- O armazenamento físico só pode ser manipulado pelo contexto interno de `inventory/storage/`. Em operação multi-repositório, **um único commit/rollback**.
- Não criar compatibilidade retroativa, banco de eventos remotos, checkpoint do Detector ou integração direta com provedor neste submódulo.
- Tratar falha como erro tipado; não alegar sucesso ou eliminar metadados/observações quando a prova for incompleta.
- Não mudar `models/`, `detectors/`, `SyncItem` ou os 16 métodos públicos para acomodar uma implementação parcial sem revisão arquitetural explícita.

## Desenvolvimento e TDD

1. Consultar [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md), identificando lacunas.
2. Implementar pequeno conjunto coeso por vez, sem stubs que retornem sucesso fictício.
3. Executar os testes de persistência em SQLite isolado e simular falhas/reabertura.
4. Verificar compatibilidade com `InventoryManager` e transações compartilhadas; rejeitar dependências circulares.
5. Informar quais cenários foram executados, aprovados, falharam ou continuam bloqueados.

## Bloqueios que não podem ser inventados

Elegibilidade e claim após crash, proteção de operações com resultado desconhecido, política de requeue e chave de idempotência.

Caso algum ponto seja essencial, registrar `BLOCKED` e pedir decisão técnica antes de implementar. O planejamento não autoriza destruir arquivos locais/remotos nem trabalhar com credenciais reais.
