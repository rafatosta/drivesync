# Detector Local

## Objetivo

Identificar alterações no sistema de arquivos de uma raiz monitorada e produzir eventos que descrevam as ocorrências observadas.

## Funcionamento

Combina notificações do sistema de arquivos com varreduras de verificação. Ao encontrar uma ocorrência, registra sua identidade, localização, tipo de alteração e metadados disponíveis. A observação deve distinguir resultados completos de leituras parciais ou inacessíveis, preservando incertezas em vez de presumir exclusões.

As ocorrências são emitidas individualmente em objetos `File`, com dados locais quando disponíveis.

## Interface

- `new`: prepara a observação de uma raiz.
- `start`: inicia a captura de mudanças.
- `scan_once`: executa uma varredura de verificação.
- `stop`: interrompe a observação.
- `status`: apresenta estado, **atividade atual**, contadores de rodada, cobertura e falhas.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** coletor físico implementado, com statx/fstat e testes isolados; os cinco métodos do Detector e o watcher continuam documentais.

## Controle pelo Detector pai

A pausa de **entrega ao Orquestrador** não pausa o observador Local: as alterações continuam sendo emitidas ao sink persistente do Detector. Apenas `stop` interrompe efetivamente a observação. O Local não avalia as consequências de períodos interrompidos nem chama Scanner por iniciativa própria; o Orquestrador decide eventuais verificações. A interface de status permite ao pai informar a condição local independentemente da condição remota.


## Fronteiras e estado da especificação

O Local não possui banco, fila durável ou baseline. A saída única é o sink individual do pai; a confirmação desse sink representa aceite, não sincronização concluída. Somente a pendência de entrega em trânsito e as pistas de observação são mantidas em memória, de forma limitada.

A coleta de metadados é injetada por uma porta estável e recebe cancelamento cooperativo. O detector transporta `LocalFile` integralmente, sem conhecer sua lista de atributos. Os tipos e campos compartilhados pertencem a Models. O perfil local e sua validação pura já foram implementados naquele módulo; a coleta física também está implementada; observação contínua e eventos ainda não. A revisão remota ainda é dependência do envelope compartilhado completo.

`scan_once` é a varredura de verificação que **emite eventos**. Não é o Scanner independente, que registra observações no Inventário. O scan de cobertura programado por `start` verifica a captura atual; não promete compensar o período parado ou reconstruir histórico.

A implementação atual cobre o adaptador físico, suas portas e o cancelamento, com testes reais em diretórios temporários e falhas de syscall controladas. As regras operacionais estão na seção 8 de INTERFACES e a rastreabilidade por método/porta está nas seções 9–13 de TESTS.


## Coleta física implementada

FilesystemMetadataCollector vincula a configuração a um descritor da raiz, reabre e revalida os ancestrais de cada entrada e devolve LocalFile pelo contrato estável. Usa apenas descritores O_PATH para metadados e readlinkat para alvos literais, sem abrir conteúdo ou seguir symlinks. Resultados inconclusivos não equivalem a ausência comprovada.

A coleta usa statx com máscaras de preenchimento e fallback para fstat, preservando btime/flags quando disponíveis e seus estados de ausência quando não. O controle de coleta não enumera campos do modelo; a montagem fica isolada em metadata.rs. Não há locks de sincronização, banco, File/RemoteFile, percurso recursivo ou watcher. Os submódulos continuam privados, mas o coletor é publicado como detectors::LocalMetadataCollector. A permissão geral de dead_code foi removida; somente a trait interna de composição ainda tem uma permissão localizada enquanto a fachada não existe.


## Acesso público pelo módulo pai

O aplicativo ou outra crate pode usar `drivesync_engine::detectors::LocalMetadataCollector`, com new/scope/collect inerentes e os tipos necessários reexportados. O consumidor pode criar/cancelar tokens; não precisa importar a trait interna. A implementação física e as portas de injeção de testes continuam privadas. Essa publicação foi restrita à coleta existente; não implementa os cinco métodos de controle do Detector Local.
