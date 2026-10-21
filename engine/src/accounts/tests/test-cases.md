# Matriz de testes — accounts

| ID | Cenário | Resultado esperado |
|---|---|---|
| ACCOUNTS-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| ACCOUNTS-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| ACCOUNTS-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| ACCOUNTS-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| ACCOUNTS-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| ACCOUNTS-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.

## ACCT-003 — oráculos implementados

- Reabrir índice com duas contas, atualizar e-mail sem duplicar e isolar clientes.
- Recusar corrupção, versão futura, duplicatas, identidade divergente e excesso
  de tamanho; preservar arquivo original e nunca sobrescrevê-lo silenciosamente.
- Falha de escrita é erro; nenhum sucesso apenas em memória.
- Serializar atualizações concorrentes e ignorar temporário não confirmado.
- Offline/cofre bloqueado/reautorização afetam só a conta correspondente; retry
  revalida e identidade divergente nunca vira sucesso.
- Metadados em disco não incluem tokens/secret nem estado autenticado.

Execução com cofre e conta reais após reinício permanece aceitação manual.
