# Inventário / baseline — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Leitura e persistência do último estado local/remoto comprovadamente sincronizado, representado por `SyncBaseline`. Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Não usar Google Drive real, credenciais ou arquivos pessoais.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| BAS-01 | Item novo sem sincronização concluída | `load` retorna `None` |
| BAS-02 | `record_local` / `record_remote` após mudança | Baseline permanece anterior |
| BAS-03 | Confirmação verificada de operação | Baseline substituído como parte da transação |
| BAS-04 | Falha ao registrar atividade | Rollback integral incluindo baseline |
| BAS-05 | Falha ao concluir operação | Baseline e status não avançam |
| BAS-06 | Revisão esperada incompatível | Rejeição sem alterações |
| BAS-07 | Diretório/documento nativo sem checksum | Não fabricar hash ou tamanho |
| BAS-08 | Reabertura após confirmação | Retomar exato último estado verificado |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; fakes do Detector/Analisador não substituem banco real.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

Tipagem de `BaselineSide.parent_identity`, critérios de verificação, política de baseline para tombstone e persistência de campos opcionais.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)
