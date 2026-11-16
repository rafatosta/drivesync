# Operador

> Política obrigatória: toda exclusão deve ir para a lixeira. Exclusão permanente
> é proibida. O método conceitual `delete` não autoriza execução e, se preservado
> no contrato futuro, deve recusar localmente sem efeitos. Nunca usar delete como
> fallback quando lixeira não estiver disponível. No Provider Google, os métodos
> destrutivos já retornam `UnsupportedOperation` sem HTTP.


## Objetivo

Executar comandos delimitados de arquivos locais/remotos, com contratos para as
demais capacidades. A implementação atual cobre upload/download de novos blobs.

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
| `delete` | Proibido: recusa local sem efeitos |
| `trash` | Enviar recurso à lixeira quando suportado |
| `restore` | Restaurar recurso quando suportado |

Cada método recebe uma requisição tipada e retorna um resultado tipado. As assinaturas, precondições, estados de execução e regras de verificação são detalhados em [INTERFACES.md](INTERFACES.md).

## Acompanhamento e cancelamento

O Operador executa **uma ação assíncrona delimitada por chamada**, sem fila ou gerenciador próprio de tarefas. Todas as dez requisições aceitam `ExecutionOptions` com **progresso e cancelamento opcionais**, inclusive operações curtas como renomear ou excluir.

O acompanhamento informa fases reais (`Preparing`, `Transferring`, `Applying`, `Verifying`, `Finalizing`) e bytes efetivamente transferidos, quando aplicável. O resultado final da operação continua sendo o retorno tipado; nenhuma atualização de progresso prova que o arquivo foi sincronizado ou verificado.

Cancelamento é cooperativo e ocorre apenas em pontos seguros: não desfaz efeitos já aplicados, não interrompe publicação atômica de forma insegura e não transforma resultado remoto indeterminado em cancelamento confirmado. O Analyzer consome as atualizações e pode incorporá-las à consulta de estado de seu `SyncItem`; o Operador não conhece Orquestrador, GTK ou interface gráfica e não oferece `status()` global nem `pause/resume/stop`.

## Implementações especializadas

- [Google](google/README.md) — executor de operações sobre arquivos e diretórios do Google Drive.
- [Example](example/README.md) — referência ilustrativa para uma implementação futura.

Cada executor segue o [contrato comum](INTERFACES.md#7-contrato-de-implementação-especializada). A biblioteca privada de funções locais permanece no próprio Operador.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** `DefaultOperatorFactory` e `GoogleOperator` executam upload/download
de novos blobs com staging privado, verificação de integridade e recuperação
durável. Veja [TRANSFERS.md](TRANSFERS.md). Outras mutações retornam capacidade não
suportada; exclusão permanente recusa sem efeitos. A execução automática no
Analyzer/Orchestrator e confirmação de baseline permanecem pendentes.
