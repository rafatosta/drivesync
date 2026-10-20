# Matriz de testes — auth

| ID | Cenário | Resultado esperado |
|---|---|---|
| AUTH-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| AUTH-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| AUTH-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| AUTH-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| AUTH-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| AUTH-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.
