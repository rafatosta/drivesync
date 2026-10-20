# Algoritmos — conflicts

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
CONFLICT-001,CONFLICT-002.

## Objetivo
Preservar versões e resolver divergências.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Congelar item.
2. preservar as duas versões com nomes sem colisões.
3. registrar origem das cópias.
4. obter decisão.
5. revalidar versões antes de resolver.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Não sobrescrever dados não preservados.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.
