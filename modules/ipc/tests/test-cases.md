# ipc — testes

| ID | Situação | Esperado |
|---|---|---|
| ipc-01 | Normal | Saída conforme contrato |
| ipc-02 | Entrada inválida | Erro tipado sem efeito |
| ipc-03 | Falha temporária | Estado preservado |
| ipc-04 | Interrupção | Recuperação ou reconexão |
| ipc-05 | Repetição | Idempotência |
| ipc-06 | Concorrência | Sem conflitos ocultos |

Completar fixtures e oráculos específicos antes da implementação.
