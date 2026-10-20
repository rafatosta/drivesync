# Testes de notifications

| ID | Situação | Oráculo |
|---|---|---|
| notifications-01 | Execução nominal | Estado esperado |
| notifications-02 | Entrada inválida | Erro tipado, sem efeitos |
| notifications-03 | Falha temporária | Retry sem perda de estado |
| notifications-04 | Falha após efeito externo | Reconciliação antes de repetir |
| notifications-05 | Cancelamento | Registro recuperável |
| notifications-06 | Concorrência | Preservar versões |

Definir fixtures e resultados concretos antes de gerar a implementação.
