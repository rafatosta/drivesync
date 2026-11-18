# Detector

## Objetivo

Detectar alterações em arquivos locais e remotos e disponibilizar essas ocorrências por uma interface única, com entrega durável e confirmação explícita.

## Funcionamento

O módulo reúne observações locais e remotas como eventos individuais, preserva os dados de cada ocorrência e os organiza em uma fila persistente. Eventos podem ser reservados em lotes, confirmados após processamento ou liberados para nova entrega. A retomada mantém a identidade e a ordem de enfileiramento dos registros.

## Fachada unificada

- `new`: cria o detector.
- `start`: inicia **sempre Local e Remoto juntos**, com entrega inicial habilitada ou pausada.
- `pause`: suspende a entrega externa de eventos, **sem interromper observação e persistência**.
- `resume`: reabilita a entrega dos eventos acumulados.
- `stop`: interrompe efetivamente **ambos** os observadores e suspende entrega; não apaga fila nem checkpoints.
- `request_scan`: solicita reobservação localizada aos produtores ativos.
- `status`: informa estado agregado, estado/atividade/erro de cada fonte, entrega e fila.
- `take_events`: reserva um lote de eventos disponíveis.
- `acknowledge`: confirma o processamento de um lote.
- `release_events`: devolve eventos reservados à fila.

Cada ocorrência é representada por `File`, incluindo identidade, contexto da alteração e dados observados disponíveis. `FileBatch` representa o lote entregue. Uma observação não equivale a uma alteração já aplicada.

## Controle de observação e entrega

`start` solicita o início das duas origens. Uma falha não impede relatar a situação da outra: o Detector pode ficar `Degraded`, mantendo estados individuais. O estado de entrega (`Enabled` ou `Paused`) é **independente** do estado operacional (`Stopped`, `Starting`, `Running`, `Degraded`, `Stopping`, `Failed`).

Durante `pause`, as origens continuam registrando na fila persistente própria do Detector; `take_events` responde `DeliveryPaused` sem reservar novos eventos. `acknowledge` e `release_events` continuam operando sobre lotes já reservados. Ao `resume`, eventos pendentes tornam-se disponíveis para consumo. Não há fila paralela obrigatória no Orquestrador.

`stop` suspende entrega e interrompe a observação, mas preserva eventos e checkpoints existentes. Mudanças locais feitas enquanto o aplicativo está fechado ficam fora do escopo atual; uma observação ausente não prova exclusão. O Detector oferece reobservação localizada e mantém reconciliação fora de sua responsabilidade.

Uma estratégia futura poderá identificar alterações locais retrospectivamente como terceiro produtor interno do Detector. Ela será implementada do zero sob o contrato genérico de observações. O Orquestrador não iniciará nem coordenará essa estratégia diretamente.

## Componentes

- [Local](local/README.md): observação do sistema de arquivos.
- [Remoto](remote/README.md): observação de alterações em serviços remotos.
- [Storage](storage/README.md): persistência e entrega dos eventos detectados.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** fachada `Detector`, captura local nativa, observador remoto Google e fila durável unificada implementados. Consulte o [fechamento e seus limites](CLOSURE.md).

## Persistência recuperável

A fila e os checkpoints pertencem ao [Detector Storage](storage/INTERFACES.md), com identidade, schema e transações privados. Para bancos SQLite registrados, [Configuration](../configuration/INTERFACES.md) oferece o **backup/restauração física comum**; o Storage garante exclusividade e valida o resultado antes de retomar seu uso.

Corrupção comprovada não pode apagar silenciosamente eventos. Se a restauração não for possível, o banco permanece indisponível; eventual reconstrução vazia é decisão distinta da coordenação e não comprova ausência de alterações. Após perda de fila ou checkpoints, o Orquestrador deve coordenar a recuperação de cobertura antes da continuidade segura.


## Interface pública disponível para coleta local

Usar `drivesync_engine::detectors::LocalMetadataCollector` com `LocalDetectorScope` para construir o coletor, `scope()` para consultar a configuração e `collect(LocalMetadataRequest)` para obter uma entrada. Cancelamento, erros, cobertura e resultados necessários são reexportados pelo mesmo módulo; LocalFile e caminhos permanecem em Models.

A API usa o coletor real já implementado e não exige trait interna ou importação de submódulos. Construção/coleta são síncronas e devem ocorrer fora da thread da GUI. LocalDetector::new/start/stop/status fornece os eventos File usados pela fachada unificada. O contrato exato está na [seção 8 de INTERFACES](INTERFACES.md#8-api-pública-compilada--coleta-local).

**Estado atual:** a fachada `Detector` compõe ambas as origens e uma fila física única, incluindo lotes mistos. A aplicação e o Developer observam a mesma sessão. [Estado, validação e limites](CLOSURE.md).
