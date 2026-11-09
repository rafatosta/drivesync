# Operador Google — TESTS

## Objetivo

Demonstrar a conformidade do executor Google com todos os métodos do `OperationExecutor`, incluindo seleção de sessão, execução de comandos, respostas e preservação de dados.

## Ambiente

Utilizar um Provider Google simulado, com IDs remotos, versões, metadados, capacidades, falhas e fluxos de bytes controlados. As operações locais utilizam diretórios temporários reais. Validar chamadas encaminhadas ao Provider e efeitos locais por asserções independentes.

## Casos por método

| Caso | Método | Resultado e validação |
| --- | --- | --- |
| OPR-G-01 | `create_file` | Cria arquivo com pai e nome corretos; retorna ID real; rejeita recurso incompatível |
| OPR-G-02 | `create_directory` | Cria pasta com pai válido; propaga conflito e falta de permissão |
| OPR-G-03 | `upload` | Transfere bytes corretos; retorna ID e metadados; trata falha parcial |
| OPR-G-04 | `download` | Grava e publica arquivo local íntegro; trata interrupção e conteúdo indisponível |
| OPR-G-05 | `update_content` | Atualiza versão do arquivo identificado mantendo seu ID; rejeita precondição divergente |
| OPR-G-06 | `rename` | Altera o nome do mesmo ID remoto; não retransfere conteúdo |
| OPR-G-07 | `move_item` | Atualiza o pai do mesmo ID remoto; valida destino e não retransfere conteúdo |
| OPR-G-08 | `delete` | Aplica exclusão ao ID exato; diferencia falha de efeito desconhecido |
| OPR-G-09 | `trash` | Marca recurso elegível na lixeira; devolve estado real ou erro tipado |
| OPR-G-10 | `restore` | Restaura recurso elegível; devolve estado real ou erro tipado |

## Testes transversais

- OPR-G-11: duas contas Google com arquivos de IDs semelhantes preservam isolamento de sessão e contexto.
- OPR-G-12: erro de capacidade indisponível retorna `UnsupportedOperation` sem efeitos indevidos.
- OPR-G-13: perda da resposta após efeito remoto produz estado indeterminado, preservando evidências.
- OPR-G-14: falhas e cancelamento durante transferência preservam arquivos locais existentes.
- OPR-G-15: a seleção e a chamada da API Google passam pelo Provider simulado.
- OPR-G-16: as funções privadas do Operador protegem o destino local contra caminhos inseguros e publicação parcial.

Executar também a [suíte compartilhada de conformidade](../TESTS.md#conformidade-dos-executores-especializados). Um mock do próprio executor Google não substitui o teste de sua implementação.

**Estado:** testes planejados; não foram executados.
