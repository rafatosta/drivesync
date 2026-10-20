# ui — testes

| ID | Situação | Esperado |
|---|---|---|
| ui-01 | Normal | Saída conforme contrato |
| ui-02 | Entrada inválida | Erro tipado sem efeito |
| ui-03 | Falha temporária | Estado preservado |
| ui-04 | Interrupção | Recuperação ou reconexão |
| ui-05 | Repetição | Idempotência |
| ui-06 | Concorrência | Sem conflitos ocultos |

Completar fixtures e oráculos específicos antes da implementação.
