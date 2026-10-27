# Módulo conflicts

## Objetivo
Classificar conflitos e propor preservação de versões.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
planner, coordinator, ui.

## Saídas e compromissos
Decisão explícita com versões preservadas.

## Limites: o que NÃO faz
Nunca sobrescrever silenciosamente versões divergentes.

## Algoritmos previstos
CONFLICT-001 preservar ambas; CONFLICT-002 resolver após escolha.

## Erros e cenários a verificar
Consultar o [checklist de validação de conflitos e falhas](validation-checklist.md), com matriz B/L/R, cenários C01–C25, erros E01–E10 e políticas pendentes. Os itens não indicam implementação ou teste concluído.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
