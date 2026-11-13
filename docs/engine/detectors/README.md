# Detector

## Objetivo

Detectar alterações em arquivos locais e remotos e disponibilizar essas ocorrências por uma interface única, com entrega durável e confirmação explícita.

## Funcionamento

O módulo reúne observações locais e remotas como eventos individuais, preserva os dados de cada ocorrência e os organiza em uma fila persistente. Eventos podem ser reservados em lotes, confirmados após processamento ou liberados para nova entrega. A retomada mantém a identidade e a ordem de enfileiramento dos registros.

## Fachada unificada prevista

- `new`: cria o detector.
- `start`: inicia **sempre Local e Remoto juntos**, com entrega inicial habilitada ou pausada.
- `pause`: suspende a entrega externa de eventos, **sem interromper observação e persistência**.
- `resume`: reabilita a entrega dos eventos acumulados.
- `stop`: interrompe efetivamente **ambos** os observadores e suspende entrega; não apaga fila nem checkpoints.
- `request_scan`: solicita uma nova observação.
- `status`: informa estado agregado, estado/atividade/erro de cada fonte, entrega e fila.
- `take_events`: reserva um lote de eventos disponíveis.
- `acknowledge`: confirma o processamento de um lote.
- `release_events`: devolve eventos reservados à fila.

Cada ocorrência é representada por `File`, incluindo identidade, contexto da alteração e dados observados disponíveis. `FileBatch` representa o lote entregue. Uma observação não equivale a uma alteração já aplicada.

## Controle de observação e entrega

`start` solicita o início das duas origens. Uma falha não impede relatar a situação da outra: o Detector pode ficar `Degraded`, mantendo estados individuais. O estado de entrega (`Enabled` ou `Paused`) é **independente** do estado operacional (`Stopped`, `Starting`, `Running`, `Degraded`, `Stopping`, `Failed`).

Durante `pause`, as origens continuam registrando na fila persistente própria do Detector; `take_events` responde `DeliveryPaused` sem reservar novos eventos. `acknowledge` e `release_events` continuam operando sobre lotes já reservados. Ao `resume`, eventos pendentes tornam-se disponíveis para consumo. Não há fila paralela obrigatória no Orquestrador.

`stop` é excepcional: suspende entrega e interrompe a observação, mas preserva eventos/checkpoints existentes. **O Orquestrador é responsável pela decisão de interromper e por qualquer verificação adicional necessária após a retomada**. O Detector não executa Scanner nem reconciliação para compensar esse período.

## Componentes

- [Local](local/README.md): observação do sistema de arquivos.
- [Remoto](remote/README.md): observação de alterações em serviços remotos.
- [Storage](storage/README.md): persistência e entrega dos eventos detectados.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** API de coleta local publicada e testada; LocalDetector::new/start/stop/status e observação nativa local implementados; fachada unificada, entrega durável e observador remoto continuam futuros.

## Persistência recuperável

A fila e os checkpoints pertencem ao [Detector Storage](storage/INTERFACES.md), com identidade, schema e transações privados. Para bancos SQLite registrados, [Configuration](../configuration/INTERFACES.md) oferece o **backup/restauração física comum**; o Storage garante exclusividade e valida o resultado antes de retomar seu uso.

Corrupção comprovada não pode apagar silenciosamente eventos. Se a restauração não for possível, o banco permanece indisponível; eventual reconstrução vazia é decisão distinta da coordenação e não comprova ausência de alterações. Após perda de fila ou checkpoints, o Orquestrador deve coordenar a recuperação de cobertura antes da continuidade segura.


## Interface pública disponível para coleta local

Usar `drivesync_engine::detectors::LocalMetadataCollector` com `LocalDetectorScope` para construir o coletor, `scope()` para consultar a configuração e `collect(LocalMetadataRequest)` para obter uma entrada. Cancelamento, erros, cobertura e resultados necessários são reexportados pelo mesmo módulo; LocalFile e caminhos permanecem em Models.

A API usa o coletor real já implementado e não exige trait interna ou importação de submódulos. Construção/coleta são síncronas e devem ocorrer fora da thread da GUI. A publicação inicial do coletor foi ampliada posteriormente com LocalDetector::new/start/stop/status e eventos File, conforme Local INTERFACES § 12. Fila e integração automática com a aplicação permanecem futuras. O contrato exato está na [seção 8 de INTERFACES](INTERFACES.md#8-api-pública-compilada--coleta-local).
