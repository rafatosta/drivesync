# Operador Google — INTERFACES

## Interface implementada

O executor Google implementa as dez funções da [interface `OperationExecutor`](../INTERFACES.md#7-contrato-de-implementação-especializada), utilizando os mesmos tipos de requisição, resultados e erros do Operador. A API de execução permanece única e as funções específicas do Google ficam encapsuladas nesta implementação.

## Execução dos comandos

| Método | Entrada relevante | Resultado esperado |
| --- | --- | --- |
| `create_file` | Pasta pai remota, nome e conteúdo opcional | ID e metadados do novo arquivo |
| `create_directory` | Pasta pai remota e nome | ID e metadados da nova pasta |
| `upload` | Arquivo local, pasta remota, nome | ID do novo arquivo e evidência da transferência |
| `download` | ID de arquivo remoto, diretório local, nome | Arquivo local íntegro e evidências de transferência |
| `update_content` | ID remoto existente, fonte de bytes, precondições | Mesmo ID remoto com metadados atualizados |
| `rename` | ID remoto, nome novo, precondições | Mesmo ID remoto com novo nome |
| `move_item` | ID remoto, pasta pai de destino, precondições | Mesmo ID remoto com localização atualizada |
| `delete` | ID remoto, precondições | Resultado verificável da exclusão |
| `trash` | ID remoto, precondições | Estado remoto de lixeira quando disponível |
| `restore` | ID remoto e precondições | Identidade e estado após restauração |

## Garantias de execução

- Utilizar a sessão/conta associada à requisição e manter as referências remotas por IDs, inclusive após renomear e mover.
- Encaminhar acesso remoto somente pelas funções de [Providers/Google](../../providers/google/INTERFACES.md).
- Validar as capacidades e as precondições de cada recurso antes de modificá-lo; preferir verificações condicionais atômicas quando oferecidas pela API.
- Utilizar funções privadas do Operador para abrir, gravar, verificar e publicar arquivos locais durante downloads e uploads.
- Processar transferências em fluxo e preservar evidências de progresso; oferecer retomada somente quando existir uma forma comprovadamente segura.
- Diferenciar sucesso verificado, falha confirmada e efeito indeterminado na resposta.
- Retornar erro tipado de operação não suportada quando o Drive ou o tipo de recurso não permitir a ação.

Os métodos e parâmetros concretos do Google Drive utilizados para cada operação devem ser escolhidos na implementação conforme o contrato do Provider. Não são introduzidas assinaturas públicas adicionais.

## Definições técnicas pendentes

Validar durante a implementação as condições de sobrescrita, operações em recursos nativos Google Workspace, capacidades por tipo de arquivo, preservação de versões e disponibilidade de mecanismos condicionais ou retomáveis.
