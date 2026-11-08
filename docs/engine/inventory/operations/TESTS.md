# Inventário / operations — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Fila persistente `SyncOperation` de comandos de sincronização, Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Utilizar somente dados sintéticos e armazenamento de teste isolado.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| OPS-01 | Enfileirar ação com app offline | Operação sobrevive à reabertura |
| OPS-02 | Dois workers reivindicam o mesmo ID simultaneamente | No máximo um claim válido |
| OPS-03 | Estado esperado incompatível | Rejeitar transição sem escrita |
| OPS-04 | Retry com agendamento futuro | Não listar como elegível antes do horário |
| OPS-05 | `Running` após interrupção abrupta | Reavaliar resultado antes de repetir ação |
| OPS-06 | Deduplicação com mesma chave e mesmo item | Aplicar política aprovada sem criar operações indevidas |
| OPS-07 | Mesma chave em itens/contas distintos | Nenhuma fusão indevida |
| OPS-08 | `update_operation` tenta concluir sem confirmação | Não oferecer transição `Complete` |
| OPS-09 | `confirm_sync` válido | Concluir operação junto com baseline/status/atividade |
| OPS-10 | Falha na confirmação conjunta | Rollback; nenhuma conclusão parcial |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; usar SQLite real para comprovar persistência.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

Elegibilidade e claim após crash, proteção de operações com resultado desconhecido, política de requeue e chave de idempotência.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)
