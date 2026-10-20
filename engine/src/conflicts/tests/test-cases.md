# Casos de teste — conflicts

| ID | Entrada/condição | Resultado obrigatório |
|---|---|---|
| CONFLICTS-T01 | Fluxo nominal | Saída tipada conforme contrato |
| CONFLICTS-T02 | Entrada inválida | Nenhum efeito e erro classificado |
| CONFLICTS-T03 | Rede/disco indisponível | Estado preservado, retry controlado |
| CONFLICTS-T04 | Crash antes de commit | Recuperação e reconciliação segura |
| CONFLICTS-T05 | Operação repetida | Idempotência ou revalidação |
| CONFLICTS-T06 | Alteração concorrente | Conflito, nenhuma perda silenciosa |

**Pendente:** concretizar fixtures, versões iniciais e pós-condições para testes executáveis antes de codificar.
