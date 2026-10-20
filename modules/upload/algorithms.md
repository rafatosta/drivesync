# Algoritmos — upload

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
UPLOAD-001,UPLOAD-002.

## Objetivo
Enviar arquivos com sessão retomável e confirmar revisão.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Observar versão local estável.
2. criar sessão retomável.
3. enviar blocos.
4. retomar offset após interrupção.
5. consultar resultado remoto antes de repetir operação ambígua.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Se arquivo mudar durante envio, reagendar sua versão atual.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.
