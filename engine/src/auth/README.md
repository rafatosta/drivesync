# Módulo auth

## Objetivo
Gerenciar autorização OAuth 2.0 com PKCE, renovação e revogação de tokens.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
accounts, remote.

## Saídas e compromissos
Credenciais autenticadas ou erros de autenticação.

## Limites: o que NÃO faz
Não listar, enviar, baixar ou apagar arquivos.

## Algoritmos previstos
AUTH-001 login; AUTH-002 renovação; AUTH-003 revogação.

## Erros e cenários a verificar
Cancelamento; expiração; token revogado; concorrência de renovação.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
