# configuration — contrato

## Objetivo
Veja README do módulo. Interface conceitual independente de linguagem.

## Entradas
Comandos tipados, IDs e configuração validada, versões esperadas onde relevantes.

## Saídas
Resposta tipada ou erro (invalid, forbidden, transient, conflict, unknown, cancelled).

## Limites
Mudar configuração não deve produzir deleção automática. Nenhum outro módulo deverá conhecer sua implementação interna.

## Garantias
Definir concorrência, idempotência, efeitos colaterais e pós-condições antes do código. Respeitar os invariantes globais.
