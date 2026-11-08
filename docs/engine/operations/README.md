# Operador

## Objetivo

Disponibilizar uma API pública para executar comandos de manipulação e transferência de arquivos e diretórios em ambientes locais e remotos.

## Funcionamento

Cada chamada informa a operação, o recurso envolvido, o destino quando necessário e as precondições para execução segura. O Operador seleciona a implementação compatível com o ambiente e, nas operações remotas, com o provedor associado à sessão. A execução retorna o resultado observado, os metadados disponíveis e a situação da verificação.

As operações sobre serviços externos utilizam as interfaces disponibilizadas por [Providers](../providers/README.md). As operações locais utilizam o sistema de arquivos.

Upload e download representam **criação por transferência de conteúdo**: upload cria o recurso remoto a partir do arquivo local; download cria a representação local a partir do recurso remoto. Quando o recurso de destino já existe e seu conteúdo precisa ser substituído, a operação é `update_content`.

As manipulações de nome e localização preservam a identidade do recurso sempre que a plataforma permitir. `move_item` atua dentro de um mesmo ambiente; transferências entre local e remoto são solicitações explícitas.

## Interface pública

| Operação | Finalidade |
| --- | --- |
| `create_file` | Criar arquivo local ou remoto |
| `create_directory` | Criar diretório local ou remoto |
| `upload` | Criar arquivo remoto transferindo conteúdo local |
| `download` | Criar arquivo local transferindo conteúdo remoto |
| `update_content` | Substituir conteúdo de arquivo existente |
| `rename` | Renomear arquivo ou diretório |
| `move_item` | Mover arquivo ou diretório no mesmo ambiente |
| `delete` | Excluir recurso definitivamente |
| `trash` | Enviar recurso à lixeira quando suportado |
| `restore` | Restaurar recurso quando suportado |

Cada método recebe uma requisição tipada e retorna um resultado tipado. As assinaturas, precondições, estados de execução e regras de verificação são detalhados em [INTERFACES.md](INTERFACES.md).

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** contrato arquitetural proposto; não implica implementação Rust existente.
