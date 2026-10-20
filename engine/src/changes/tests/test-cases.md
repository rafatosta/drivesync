# Matriz de testes — changes

| ID | Cenário | Resultado esperado |
|---|---|---|
| CHANGES-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| CHANGES-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| CHANGES-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| CHANGES-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| CHANGES-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| CHANGES-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.
