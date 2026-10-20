# Algoritmos — download

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
DOWNLOAD-001,DOWNLOAD-002.

## Objetivo
Baixar em staging e validar antes de instalar.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Consultar ID/revisão remotos.
2. verificar espaço.
3. baixar por streaming para arquivo temporário.
4. validar hash e tamanho quando possível.
5. sincronizar e renomear atomicamente.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Arquivo incompleto não substitui arquivo válido.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.
