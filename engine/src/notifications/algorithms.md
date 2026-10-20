# Algoritmos notifications

**Estado:** especificação proposta v0.2, independente de linguagem.

## IDs
NOTIFY-001 eventos, NOTIFY-002 progresso.

## Pré-condições
Entradas validadas, estado conhecido, pré-condições registradas e recursos disponíveis.

## Algoritmo
1. Receber evento de motor.
2. filtrar segredos.
3. agrupar atualizações de progresso.
4. publicar para assinantes.
5. disponibilizar snapshot.

## Pós-condições
Resultado confirmado e durável ou erro tipado com etapas parciais recuperáveis.

## Falhas
Rede, autorização, disco, versão concorrente, estado parcial, cancelamento e interrupção entre efeito e commit.

## Verificar
Identidade, versões, integridade e invariantes antes de ações destrutivas, e efeitos confirmados depois.

## Não inferir
Timeout não prova ausência; nome não prova identidade; erro parcial não confirma sucesso; estado UNKNOWN bloqueia exclusão.

## Invariante
Nenhum token em evento ou log.

## Complexidade
Metadados por item aproximadamente O(1); n operações O(n); B bytes transferidos O(B), excluídas latências externas.
