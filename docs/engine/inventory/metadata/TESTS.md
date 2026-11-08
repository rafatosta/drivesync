# Inventário / metadata — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Persistência e reconstrução dos atributos selecionados de `LocalFile` e `RemoteFile` definidos em `models/`. Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Não usar Google Drive real, credenciais ou arquivos pessoais.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| MET-01 | Round-trip do conjunto aprovado de campos locais | Mesmo valor e estado de observação após reabrir banco |
| MET-02 | Round-trip dos campos remotos selecionados | Recuperar identidade qualificada e metadados válidos |
| MET-03 | `NotCollected` após `Known(v)` | Não apagar valor anterior |
| MET-04 | `Known(false)`, `Known(0)` e `Known([])` | Persistir explicitamente os valores observados |
| MET-05 | `Unsupported`, `Unavailable`, `PermissionDenied` | Não substituir evidência conhecida por valor fictício |
| MET-06 | Caminho local com bytes não UTF-8 | Preservar bytes sem conversão com perda |
| MET-07 | Resposta de API parcialmente preenchida | Não limpar grupos ausentes nem criar `RemoteFileRef` fictício |
| MET-08 | Relações com múltiplos pais remotos | Preservar somente relações comprovadas |
| MET-09 | `ProviderFileData` aprovado e sanitizado | Codec/versionamento e ausência de credenciais |
| MET-10 | Falha de gravação durante atualização parcial | Rollback do item e metadados |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; fakes do Detector/Analisador não substituem banco real.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

Lista manter/sob demanda/descartar, representação de estados de coleta, DDL, limites, codec versionado e sanitização do envelope especializado.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)
