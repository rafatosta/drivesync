# Matriz de testes — local

| ID | Cenário | Resultado esperado |
|---|---|---|
| LOCAL-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| LOCAL-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| LOCAL-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| LOCAL-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| LOCAL-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| LOCAL-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.
