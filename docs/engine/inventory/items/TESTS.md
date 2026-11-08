# Inventário / items — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Identidade lógica, vínculos, presença, status, revisão otimista e tombstones dos `SyncItem`. Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Não usar Google Drive real, credenciais ou arquivos pessoais.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| ITM-01 | Busca por ID lógico, ID local, referência remota e caminho relativo | Item correto ou ausência sem criar registro |
| ITM-02 | Mesmo ID remoto bruto em contas diferentes | Sem colisão entre `RemoteFileRef` de contas distintas |
| ITM-03 | Tentativa de associar referência já vinculada a outro item | Erro de unicidade e rollback |
| ITM-04 | `FieldUpdate::Keep/Set/Clear` nas associações | Preservar, alterar ou remover vínculo somente quando explícito |
| ITM-05 | Revisão esperada desatualizada | `VersionMismatch` sem mutação |
| ITM-06 | `Unknown` e `File.data=None` | Sem criação ou exclusão automática |
| ITM-07 | `AbsentConfirmed` sem prova de escopo completo | Rejeitar ausência não comprovada |
| ITM-08 | Renomear/mover item correlacionado | Manter `SyncItemId` estável |
| ITM-09 | Tombstone com histórico/operação vinculada | Preservar item e chaves estrangeiras |
| ITM-10 | Consulta paginada com filtros | Nenhuma duplicação sob ordenação definida |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; fakes do Detector/Analisador não substituem banco real.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

Correlação local sem `local_id`, política de tombstones, atualização de `ItemKind`, semântica de revisão em operações compostas, paginação.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)
