# Módulo state

## Objetivo
Persistir estado-base, versões, cursores, operações e suas transições.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
coordinator, changes, planner, queue.

## Saídas e compromissos
Registro consistente, recuperável e auditável.

## Limites: o que NÃO faz
Nunca inferir exclusão pela ausência de estado ilegível.

## Algoritmos previstos
STATE-001 transação; STATE-002 recuperação de inconsistência.

## Erros e cenários a verificar
Falhas de disco; corrupção; crash após efeito externo.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
