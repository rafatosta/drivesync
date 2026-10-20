# Matriz de testes — state

| ID | Cenário | Resultado esperado |
|---|---|---|
| STATE-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| STATE-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| STATE-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| STATE-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| STATE-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| STATE-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.
