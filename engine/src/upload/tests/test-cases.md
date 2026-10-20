# Testes de upload

| ID | Situação | Oráculo |
|---|---|---|
| upload-01 | Execução nominal | Estado esperado |
| upload-02 | Entrada inválida | Erro tipado, sem efeitos |
| upload-03 | Falha temporária | Retry sem perda de estado |
| upload-04 | Falha após efeito externo | Reconciliação antes de repetir |
| upload-05 | Cancelamento | Registro recuperável |
| upload-06 | Concorrência | Preservar versões |

Definir fixtures e resultados concretos antes de gerar a implementação.
