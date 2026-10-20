# Módulo ui

## Objetivo
Exibir contas, pasta, configuração, progresso, estados e conflitos.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
usuário.

## Saídas e compromissos
Ações dirigidas ao motor via contrato.

## Limites: o que NÃO faz
Nunca executar lógica de sincronização dentro da janela GTK.

## Algoritmos previstos
UI-001 iniciar interface; UI-002 exibir estado.

## Erros e cenários a verificar
Motor indisponível; operação demorada; interface encerrada.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
