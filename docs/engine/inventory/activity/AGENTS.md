# Inventário / activity — instruções para agentes

> Aplicar junto de [inventory/AGENTS.md](../AGENTS.md) e deste submódulo: [README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md). Este é **um contrato documental proposto**, não uma alegação de implementação.

## Missão e limite de edição

Histórico persistente por arquivo/diretório via `SyncActivity`, distinto dos eventos técnicos do Detector.

**Pode atuar futuramente:** apenas nos arquivos Rust do submódulo `inventory/activity/` e testes pertinentes, **quando a implementação for explicitamente solicitada**. Nesta fase, não modificar código compilado, dados de usuário ou interfaces de outros módulos.

**Interfaces recebidas:** `NewActivity`, `ActivityFilter`, `Pagination`, `SyncItemId` e `OperationId` opcional.

**Dependências aceitas:** Tipos do Inventário, FK e transações de `storage/`. A fachada valida associação com `items/` e `operations/`.

**Proibições:** Não detecta eventos, não decide conflitos, não executa ações, não registra progresso por chunk nem eventos globais sem `item_id`.

## Regras contratuais

- `append` — inserir atividade validada e associada ao item lógico.
- `list` — consultar histórico de itens, operações e períodos com paginação.
- Participar da transação coordenada de `confirm_sync` quando uma atividade for obrigatória.
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

Idempotência de atividade, retenção, sanitização de descrição e desempate da ordenação temporal.

Caso algum ponto seja essencial, registrar `BLOCKED` e pedir decisão técnica antes de implementar. O planejamento não autoriza destruir arquivos locais/remotos nem trabalhar com credenciais reais.
