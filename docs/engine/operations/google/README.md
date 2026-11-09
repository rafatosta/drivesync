# Operador — Google

## Objetivo

Executar comandos de manipulação e transferência de arquivos e diretórios do Google Drive, por meio da API pública do Operador e das funções disponibilizadas pelo [Provider Google](../../providers/google/README.md).

## Funcionamento

A implementação Google recebe uma requisição vinculada a uma sessão e a um recurso do Google Drive, executa a operação solicitada e devolve a identidade, os metadados e as evidências disponíveis. Em transferências, utiliza as funções privadas de filesystem do Operador para tratar de modo seguro os arquivos locais.

## Contrato implementado

A implementação deve disponibilizar integralmente o [contrato `OperationExecutor`](../INTERFACES.md#7-contrato-de-implementação-especializada):

| Operação | Execução esperada |
| --- | --- |
| `create_file` | Criar arquivo no Drive |
| `create_directory` | Criar pasta no Drive |
| `upload` | Criar arquivo remoto transferindo conteúdo local |
| `download` | Criar arquivo local a partir do recurso remoto |
| `update_content` | Atualizar conteúdo preservando o ID remoto |
| `rename` | Alterar nome do recurso pelo ID remoto |
| `move_item` | Alterar a pasta pai do recurso |
| `delete` | Excluir definitivamente o recurso identificado |
| `trash` | Enviar recurso à lixeira |
| `restore` | Restaurar recurso elegível |

As operações precisam respeitar as capacidades reais de cada recurso. Quando uma operação não for suportada, deve ser retornado um erro tipado, sem produzir efeito inesperado.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** contrato documental de implementação especializada; não comprova existência de código Rust.
