# Algoritmos — remote

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
REMOTE-001 paginação; REMOTE-002 operações; REMOTE-003 retry.

## Objetivo
Adaptador REST da Drive API v3.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Obter token.
2. enviar request com timeout.
3. paginar.
4. converter IDs/revisões para modelo interno.
5. classificar status.
6. backoff com jitter.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
403/404/429/5xx são erros distintos. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.
