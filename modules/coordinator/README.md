# Módulo coordinator

## Objetivo
Orquestrar ciclos: autenticar, observar, planejar, executar, confirmar.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
ui, ipc, CLI.

## Saídas e compromissos
Estado do serviço e conclusão de ciclos.

## Limites: o que NÃO faz
Não conter regras específicas OAuth, HTTP ou conflitos.

## Algoritmos previstos
COORD-001 ciclo; RECOVERY-002 retomar após interrupção.

## Erros e cenários a verificar
Interrupção arbitrária; acesso concorrente; reconciliação.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
