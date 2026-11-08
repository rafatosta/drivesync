# Inventário / metadata — instruções para agentes

> Aplicar junto de [inventory/AGENTS.md](../AGENTS.md) e deste submódulo: [README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md). Este é **um contrato documental proposto**, não uma alegação de implementação.

## Missão e limite de edição

Persistência e reconstrução dos atributos selecionados de `LocalFile` e `RemoteFile` definidos em `models/`.

**Pode atuar futuramente:** apenas nos arquivos Rust do submódulo `inventory/metadata/` e testes pertinentes, **quando a implementação for explicitamente solicitada**. Nesta fase, não modificar código compilado, dados de usuário ou interfaces de outros módulos.

**Interfaces recebidas:** `LocalFile`, `RemoteFile`, `SyncItemId`, `MetadataValue<T>`, `RemoteFileRef`, `ProviderFileData` quando autorizado.

**Dependências aceitas:** `models/` fornece tipos e semântica de metadados; `storage/` fornece transação; associação com `SyncItem` é coordenada por `InventoryManager`.

**Proibições:** Não acessa filesystem, `statx`, Google API, `GoogleFile` nem credenciais; não classifica eventos, não decide presença/conflitos nem avança baseline.

## Regras contratuais

- `LocalMetadataRepository.apply/load` — armazenar e reconstruir metadados locais aprovados.
- `RemoteMetadataRepository.apply/load` — armazenar e reconstruir metadados remotos aprovados.
- Preservar estados de coleta/ausência de conhecimento sem inventar atributos.
- Delegar o mapeamento SQL ao contexto interno de armazenamento.
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

Lista manter/sob demanda/descartar, representação de estados de coleta, DDL, limites, codec versionado e sanitização do envelope especializado.

Caso algum ponto seja essencial, registrar `BLOCKED` e pedir decisão técnica antes de implementar. O planejamento não autoriza destruir arquivos locais/remotos nem trabalhar com credenciais reais.
