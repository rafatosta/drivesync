# Testes de coordinator

| ID | Situação | Oráculo |
|---|---|---|
| coordinator-01 | Execução nominal | Estado esperado |
| coordinator-02 | Entrada inválida | Erro tipado, sem efeitos |
| coordinator-03 | Falha temporária | Retry sem perda de estado |
| coordinator-04 | Falha após efeito externo | Reconciliação antes de repetir |
| coordinator-05 | Cancelamento | Registro recuperável |
| coordinator-06 | Concorrência | Preservar versões |

Definir fixtures e resultados concretos antes de gerar a implementação.
