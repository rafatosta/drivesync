# Módulo notifications

## Objetivo
Expor alterações de estado, falhas e progresso do motor.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
ui, ipc.

## Saídas e compromissos
Eventos observáveis, não sensíveis.

## Limites: o que NÃO faz
Não alterar estado de arquivos ou credenciais.

## Algoritmos previstos
NOTIFY-001 publicar evento; NOTIFY-002 agrupar progresso.

## Erros e cenários a verificar
Eventos perdidos; congestionamento; vazamento de dados.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
