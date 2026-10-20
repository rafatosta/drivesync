# Módulo ipc

## Objetivo
Oferecer protocolo estável entre GUI e motor, futuramente por D-Bus.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
ui, CLI.

## Saídas e compromissos
Comandos tipados e eventos.

## Limites: o que NÃO faz
Não implementar lógica de sincronização.

## Algoritmos previstos
IPC-001 solicitar operação; IPC-002 acompanhar progresso.

## Erros e cenários a verificar
Cliente desconectado; versão de protocolo divergente.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
