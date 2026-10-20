# Testes de queue

| ID | Situação | Oráculo |
|---|---|---|
| queue-01 | Execução nominal | Estado esperado |
| queue-02 | Entrada inválida | Erro tipado, sem efeitos |
| queue-03 | Falha temporária | Retry sem perda de estado |
| queue-04 | Falha após efeito externo | Reconciliação antes de repetir |
| queue-05 | Cancelamento | Registro recuperável |
| queue-06 | Concorrência | Preservar versões |

Definir fixtures e resultados concretos antes de gerar a implementação.
