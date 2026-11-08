# Inventário / activity — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Histórico persistente por arquivo/diretório via `SyncActivity`, distinto dos eventos técnicos do Detector. Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Não usar Google Drive real, credenciais ou arquivos pessoais.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| ACT-01 | Registrar atividade com item existente | Vínculo FK obrigatório preservado |
| ACT-02 | Atividade sem `item_id` válido | Rejeição sem escrita |
| ACT-03 | `operation_id=None` em conflito informativo | Registro permitido com item válido |
| ACT-04 | `operation_id` de outro item | Rejeitar associação incoerente |
| ACT-05 | Renomear arquivo após atividade | Nome/caminho históricos preservados |
| ACT-06 | Caminho local em bytes não UTF-8 | Sem perda na persistência |
| ACT-07 | Paginação/filtros de atividade | Ordenação consistente e resultados esperados |
| ACT-08 | Confirmação repetida após perda de resposta | Não duplicar histórico mediante política de idempotência |
| ACT-09 | Falha no append durante `confirm_sync` | Rollback de operação e baseline |
| ACT-10 | Tombstone referenciado por atividade | Preservar integridade referencial |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; fakes do Detector/Analisador não substituem banco real.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

Idempotência de atividade, retenção, sanitização de descrição e desempate da ordenação temporal.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)
