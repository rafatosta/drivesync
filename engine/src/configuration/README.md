# Módulo configuration

## Objetivo
Validar preferências, pastas, seleção e políticas de sincronização.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
coordinator, accounts, ui.

## Saídas e compromissos
Configuração validada.

## Limites: o que NÃO faz
Não executar transferência.

## Algoritmos previstos
CONFIG-001 validar perfil; CONFIG-002 migrar configuração.

## Erros e cenários a verificar
Pasta inexistente; permissões; configuração incompatível.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
