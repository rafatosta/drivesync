# Inventory — Items

## Objetivo

Manter a identidade lógica e o estado dos arquivos e diretórios registrados no inventário por meio de `SyncItem`.

## Funcionamento

Cada item recebe uma identidade persistente, com associações locais e remotas, indicadores de presença, estado, revisão e tombstones. As atualizações preservam integridade e verificam a revisão aplicável.

## Interface

- `resolve`: localiza a identidade de um item.
- `list_ids` e `load_header`: consultam identificadores e atributos estruturais.
- `create`: cria uma identidade lógica.
- `apply_links`: atualiza associações locais e remotas.
- `set_presence`: registra presença com evidência de observação.
- `set_status`: atualiza o estado do item.

**Entradas principais:** `SyncItemId`, `ItemSelector`, `ItemFilter`, `ItemLinksChange`, `ItemStatusChange`, `ObservationProof` e `PresenceState`.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
