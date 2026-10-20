# Módulo planner

## Objetivo
Comparar versão-base, estado local e remoto e propor plano seguro.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
coordinator, queue.

## Saídas e compromissos
Plano sem efeitos colaterais ou conflito explícito.

## Limites: o que NÃO faz
Não executar upload, download ou deletar dados.

## Algoritmos previstos
PLAN-001 comparação tripla; SYNC-002 pré-condições.

## Erros e cenários a verificar
Mudança simultânea; remoção; renomeação; dados incompletos.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
