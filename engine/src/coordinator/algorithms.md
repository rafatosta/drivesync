# Algoritmos coordinator

**Estado:** especificação proposta v0.2, independente de linguagem.

## IDs
COORD-001 ciclo, RECOVERY-002 recuperação.

## Pré-condições
Entradas validadas, estado conhecido, pré-condições registradas e recursos disponíveis.

## Algoritmo
1. Obter lock exclusivo por raiz.
2. validar estado.
3. recuperar in-flight.
4. receber alterações.
5. pedir plano puro.
6. persistir.
7. executar com versões esperadas.
8. confirmar efeitos e base.

## Pós-condições
Resultado confirmado e durável ou erro tipado com etapas parciais recuperáveis.

## Falhas
Rede, autorização, disco, versão concorrente, estado parcial, cancelamento e interrupção entre efeito e commit.

## Verificar
Identidade, versões, integridade e invariantes antes de ações destrutivas, e efeitos confirmados depois.

## Não inferir
Timeout não prova ausência; nome não prova identidade; erro parcial não confirma sucesso; estado UNKNOWN bloqueia exclusão.

## Invariante
Falha de estado bloqueia operações destrutivas.

## Complexidade
Metadados por item aproximadamente O(1); n operações O(n); B bytes transferidos O(B), excluídas latências externas.
