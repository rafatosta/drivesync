# Casos de teste — planner

| ID | Entrada/condição | Resultado obrigatório |
|---|---|---|
| PLANNER-T01 | Fluxo nominal | Saída tipada conforme contrato |
| PLANNER-T02 | Entrada inválida | Nenhum efeito e erro classificado |
| PLANNER-T03 | Rede/disco indisponível | Estado preservado, retry controlado |
| PLANNER-T04 | Crash antes de commit | Recuperação e reconciliação segura |
| PLANNER-T05 | Operação repetida | Idempotência ou revalidação |
| PLANNER-T06 | Alteração concorrente | Conflito, nenhuma perda silenciosa |

**Pendente:** concretizar fixtures, versões iniciais e pós-condições para testes executáveis antes de codificar.
