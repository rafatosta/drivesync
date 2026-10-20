# Matriz de testes — remote

| ID | Cenário | Resultado esperado |
|---|---|---|
| REMOTE-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| REMOTE-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| REMOTE-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| REMOTE-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| REMOTE-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| REMOTE-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.
