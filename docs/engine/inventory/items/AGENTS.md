# Inventário / items — instruções para agentes

> Aplicar junto de [inventory/AGENTS.md](../AGENTS.md) e deste submódulo: [README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md). Este é **um contrato documental proposto**, não uma alegação de implementação.

## Missão e limite de edição

Identidade lógica, vínculos, presença, status, revisão otimista e tombstones dos `SyncItem`.

**Pode atuar futuramente:** apenas nos arquivos Rust do submódulo `inventory/items/` e testes pertinentes, **quando a implementação for explicitamente solicitada**. Nesta fase, não modificar código compilado, dados de usuário ou interfaces de outros módulos.

**Interfaces recebidas:** `SyncItemId`, `ItemSelector`, `ItemFilter`, `ItemLinksChange`, `ItemStatusChange`, `ObservationProof`, `PresenceState`, `LocalFileId`, `RemoteFileRef`.

**Dependências aceitas:** `models/` para identidades; `inventory/storage/` para contexto de transação. `metadata/` e `baseline/` são coordenados pela fachada, não dependências diretas.

**Proibições:** Não interpreta `FileEvent`, não realiza detecção, não compõe `LocalFile`/`RemoteFile`, não grava baseline ou histórico.

## Regras contratuais

- `resolve` — localizar o identificador lógico sem inferir identidade por nome.
- `list_ids` e `load_header` — consultar itens/atributos estruturais para reconstrução pela fachada.
- `create` — estabelecer identidade lógica única, sem fabricar presença.
- `apply_links` — aplicar Keep/Set/Clear com verificação de revisão.
- `set_presence` — registrar estado com prova de observação.
- `set_status` — guardar diagnóstico definido pelo consumidor autorizado.
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

Correlação local sem `local_id`, política de tombstones, atualização de `ItemKind`, semântica de revisão em operações compostas, paginação.

Caso algum ponto seja essencial, registrar `BLOCKED` e pedir decisão técnica antes de implementar. O planejamento não autoriza destruir arquivos locais/remotos nem trabalhar com credenciais reais.
