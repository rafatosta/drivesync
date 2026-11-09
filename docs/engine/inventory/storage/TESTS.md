# Inventário / storage — plano de testes

> **Status:** cenários especificados, **não executados**. O contrato a testar é [INTERFACES.md](INTERFACES.md), e a API externa permanece em [inventory/INTERFACES.md](../INTERFACES.md).

## Escopo e estratégia

Única infraestrutura de acesso ao SQLite de cada sincronização, incluindo abertura segura, integridade e transações. Exercitar o submódulo por interfaces internas com `inventory.sqlite3` **real e isolado**, fixtures determinísticas, contas/raízes fictícias e injeção controlada de falhas. Utilizar somente dados sintéticos e armazenamento de teste isolado.

## Matriz de cenários

| ID | Ação / situação | Resultado esperado |
| --- | --- | --- |
| STO-01 | Primeira abertura em pasta vazia autorizada | Banco/identidade consistentes |
| STO-02 | Pasta ocupada sem inventário válido | Não inicializar silenciosamente |
| STO-03 | Conta/raiz remota/sync_id divergente | `InvalidIdentity`, sem alterar banco |
| STO-04 | Banco corrompido ou esquema incompatível | Erro; não recriar/resetar |
| STO-05 | FK/UNIQUE/CHECK violados | Rollback e erro público tipado |
| STO-06 | Falha entre baseline e atividade na transação | Nenhum commit parcial |
| STO-07 | Reabrir após commit concluído | Dados e relações íntegros |
| STO-08 | Duas sincronizações separadas | Dados não se misturam |
| STO-09 | Disco cheio/permissão negada/banco bloqueado | Erro sanitizado e sem sucesso fictício |
| STO-10 | Inspeção do esquema | Esquema contém apenas tabelas previstas para o inventário |

## Critérios transversais

1. Verificar retornos de sucesso e erro; nenhum erro deixa escrita parcial.
2. Fechar e reabrir SQLite após persistência para verificar durabilidade, IDs, vínculos e estados.
3. Isolar duas sincronizações com `SyncId` e `AccountId` distintos; não misturar itens.
4. Validar `InventoryFailure` sanitizado, sem tokens, SQL ou caminhos pessoais nos erros.
5. Cobrir concorrência, idempotência e rollback quando aplicável; usar SQLite real para comprovar persistência.
6. Não declarar `PASS` para um caso sem executá-lo. Distinguir testes `BLOCKED` pelos pontos ainda não especificados.

## Pontos bloqueadores e aceite

DDL final, WAL/locking, timeouts, provisionamento com raiz inacessível, índices, conexões e teste de crash em subprocesso.

Aceite somente após a revisão dos contratos pendentes, implementação Rust e execução dos cenários com evidências. Atualizar [TESTS.md do Inventário](../TESTS.md) com o resultado consolidado. **Não alterar contratos públicos para fazer um teste passar.**

## Referências

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Inventário / TESTS](../TESTS.md)

## Casos adicionais — backup e recuperação

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| STO-R01 | Corrupção comprovada, backup íntegro | Erro e mensagem ao usuário, restauração física pelo mecanismo comum de Configuration, com validação pelo Storage, reabertura |
| STO-R02 | Backup mais recente inválido, anterior íntegro | Nunca restaurar arquivo inválido; recuperar cópia válida conforme política |
| STO-R03 | Falha de todas as tentativas de restauração | Preservar banco danificado e retornar falha; **não** criar estrutura vazia automaticamente |
| STO-R04 | Erro também na reconstrução | Erro estrutural e sincronização bloqueada; nenhum sucesso simulado |
| STO-R05 | Banco com WAL ativo | Cópia consistente, isolamento de conexões e validação pós-restauração |
| STO-R06 | `SQLITE_BUSY`, disco cheio, permissão negada, identidade/esquema incompatível | Não classificar como corrupção nem reconstruir |
| STO-R07 | Banco restaurado defasado dos Detectores | Reobservação local/remota; não executar exclusão automática devido a perda de histórico |
| STO-R08 | Banco reconstruído após autorização separada | Scanner e Analisador operam sem mudança de algoritmo; `Unknown` não vira `AbsentConfirmed` |

Executar com SQLite real isolado, corrupção e falhas injetadas, dados sintéticos e ambiente Flatpak. **Testes documentados, ainda não executados.**

## Testes de disponibilidade operacional

| ID | Situação | Resultado esperado |
| --- | --- | --- |
| STO-S01 | `open` normal | Resultado `Normal`, disponibilidade `Ready` após inicialização |
| STO-S02 | Recuperação por backup consistente | Resultado `Restored`, sem inventar baseline novo |
| STO-S03 | Reconstrução segura após falha de backups | Resultado `Rebuilt`, estrutura vazia sem exclusões presumidas |
| STO-S04 | Recuperação impossível | Erro tipado e indisponibilidade; não liberar operações inseguras |
| STO-S05 | Leitura de snapshot de status durante acesso SQLite concorrente | Não executar SQL nem aguardar término da transação |
| STO-S06 | Preferências ausentes | Infraestrutura de recuperação utiliza parâmetros padrão sem exigir preferências salvas |
| STO-S07 | Identidade divergente / esquema incompatível / SQLITE_BUSY | Não classificar corrupção nem recriar banco |

Os testes são propostas, não foram executados.
