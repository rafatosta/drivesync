# Casos de teste — download

| ID | Entrada/condição | Resultado obrigatório |
|---|---|---|
| DOWNLOAD-T01 | Fluxo nominal | Saída tipada conforme contrato |
| DOWNLOAD-T02 | Entrada inválida | Nenhum efeito e erro classificado |
| DOWNLOAD-T03 | Rede/disco indisponível | Estado preservado, retry controlado |
| DOWNLOAD-T04 | Crash antes de commit | Recuperação e reconciliação segura |
| DOWNLOAD-T05 | Operação repetida | Idempotência ou revalidação |
| DOWNLOAD-T06 | Alteração concorrente | Conflito, nenhuma perda silenciosa |

**Pendente:** concretizar fixtures, versões iniciais e pós-condições para testes executáveis antes de codificar.
