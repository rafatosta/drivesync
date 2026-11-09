# Detector — TESTS

## 1. Objetivo e independência

TDD do **coordenador real**, com **Local fake, Remoto fake e Storage fake** que implementam apenas os contratos internos. O Detector deve ser compilável e testável mesmo quando os três filhos concretos não existem. Testar cada filho real **em sua própria suíte**: Local (filesystem temporário), Remoto (Google API fake), Storage (SQLite real isolado). Um consumidor de teste utiliza somente os métodos públicos do Detector.

Escrever testes RED antes da implementação, completar GREEN por unidade funcional, REFACTOR e regressão; commits locais coesos.

## 2. Fixtures

| Fixture | Conteúdo |
| --- | --- |
| `two_producers` | Local e Remoto fake produzindo eventos individuais de maneira intercalada |
| `same_item_both_sides` | Duas fontes emitem eventos diferentes para o mesmo SyncItem |
| `slow_consumer` | Produtores continuam enfileirando enquanto o consumidor não chama `take_events` |
| `store_fake` | Implementa enqueue, claim, ack, release, status e leases sem SQLite |
| `store_failure` | Enqueue retorna erro/backpressure; produtor não recebe confirmação durável |
| `failed_availability_notification` | Sink opcional de aviso falha após enqueue concluído |
| `stopped_producers` | Fontes Stopped; Store ainda possui pendências |
| `two_syncs` | Instâncias isoladas por sync_id |
| `expired_lease` | Consumo interrompido, lease vence e o lote é reentregue |

## 3. Índice e casos por método

### DET-01 — `Detector::new`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Criar fachada com três portas fake independentes, sem construir dependências fora do detector.

**Negativos e opcionais:** Identidade inválida, store indisponível, notificação de disponibilidade omitida.

**Retorno e invariantes:** Retorna Stopped; não expõe tipos concretos dos filhos.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-02 — `Detector::start`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Iniciar **ambas** as origens em uma chamada, inclusive com entrega inicialmente pausada e sem janela de reserva.

**Negativos e opcionais:** Start repetido ou uma origem falha.

**Retorno e invariantes:** Estado Degraded quando somente uma origem falha; status individual preserva o diagnóstico da fonte que falhou e a atividade da outra.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-03 — `Detector::pause`

**Sucesso:** pausa entrega, Local e Remoto continuam emitindo e armazenando eventos.
**Negativos:** pausa repetida, lote já reservado, corrida pause/take_events.
**Invariantes:** nenhuma nova reserva após pausa confirmada; ack/release de lotes antigos permitidos.

### DET-04 — `Detector::resume`

**Sucesso:** libera consumo de lotes acumulados durante pausa.
**Negativos:** resume repetido, tentativa quando ambos observadores estão parados.
**Invariantes:** não dispara observação nem reconciliação e não modifica eventos.

### DET-05 — `Detector::stop`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Parar efetivamente as duas fontes em DrainInFlight ou StopNow e suspender entrega.

**Negativos e opcionais:** Fonte em falha, emissão concorrente, stop repetido.

**Retorno e invariantes:** Eventos já gravados continuam disponíveis; não executa acknowledge.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-06 — `Detector::request_scan`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Solicitar All ou Items ao coordenador, que repassa aos produtores conforme capacidades.

**Negativos e opcionais:** Items vazio, ID de outro sync, request_id ausente.

**Retorno e invariantes:** Apenas observação, sem ações de sincronização.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-07 — `Detector::status`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Consultar estado agregado, entrega, estados/atividades/erros individuais de ambas as origens e números pending/leased fornecidos pelo Store.

**Negativos e opcionais:** Nenhum produtor ativo, storage indisponível, uma fonte degradada.

**Retorno e invariantes:** Sem consumo nem SQL ou I/O interno do filho.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-08 — `Detector::take_events`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Reservar lote de até N eventos locais e remotos intercalados em ordem de enfileiramento.

**Negativos e opcionais:** N=0, lease inválido, fila vazia, Store falha, entrega pausada (`DeliveryPaused`) sem claim.

**Retorno e invariantes:** Lote pode conter mesmo item de ambas as origens; nenhuma eliminação por colisão; nenhum conflito decidido.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-09 — `Detector::acknowledge`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Confirmar Processed ou Discarded pelo batch_id.

**Negativos e opcionais:** Lease inválido, expirado, ack duplicado com disposição diferente.

**Retorno e invariantes:** Somente decisão explícita retira pendência; não confirma baseline.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


### DET-10 — `Detector::release_events`

**Contrato:** request/result e erros públicos documentados em [INTERFACES.md](INTERFACES.md).

**Sucesso:** Liberar lote reservado para reentrega.

**Negativos e opcionais:** Batch errado, release repetido, crash antes da confirmação.

**Retorno e invariantes:** Eventos ficam novamente elegíveis, sem perda.

**Verificação:** checar objeto público e chamadas às **portas fake**; inspecionar estado do Store fake por sua API, sem importar estruturas internas nem depender de implementações reais. Cada resultado é aferido com valores independentes dos algoritmos testados.


## 4. Eventos mistos e produtor–consumidor

| Caso | Cenário | Resultado obrigatório |
| --- | --- | --- |
| DET-B-01 | Evento Local isolado | Uma emissão gera um registro e lote válido |
| DET-B-02 | Evento Remoto isolado | Mesmo contrato de lote, sem campo obrigatório de origem interna |
| DET-B-03 | Local e Remoto emitem simultaneamente | Ambos entram na fila e podem sair **no mesmo FileBatch** |
| DET-B-04 | Mesmo item muda em ambas as origens | **Dois eventos preservados**; nenhuma prevalência ou conflito decidido pelo Detector |
| DET-B-05 | Eventos intercalados | Ordem da fila é de commit/enfileiramento, **não ordem causal das alterações** |
| DET-B-06 | Lote de 1 ou N eventos | Mesma API de consumo e confirmação; max_events respeitado |
| DET-B-07 | Consumidor lento | Pendências se acumulam sem perder eventos persistidos |
| DET-B-08 | Dois consumidores tentam reservar | Sem mesmo evento simultaneamente sob dois leases válidos |
| DET-B-09 | Lote reservado e falha antes do ack | Redelivery após release ou expiração |
| DET-B-10 | Confirmação de Discarded | Apenas lote explicitamente confirmado é descartado |
| DET-B-11 | StopNow após enqueues | Eventos persistidos continuam disponíveis mesmo com produtores parados |
| DET-B-12 | Sink de disponibilidade falha | Enqueue durável não desaparece; polling de take_events continua possível |
| DET-B-13 | Storage falha no enqueue | Produtor recebe falha, não sucesso fictício |
| DET-B-14 | Evento sem item conhecido | Mantém necessidade de reavaliação por escopo |
| DET-B-15 | Repetição do mesmo evento após resultado incerto | Idempotência por ID quando comprovada; não suprimir alteração distinta |

## 5. Casos de controle e estado

| ID | Situação | Resultado |
| --- | --- | --- |
| DET-C-01 | `start(delivery=Paused)` | Local e Remoto começam a observar; nenhuma reserva externa antes de `resume` |
| DET-C-02 | `pause` durante produção | Eventos persistidos continuam aumentando; consumo suspenso |
| DET-C-03 | `resume` após eventos acumulados | `take_events` entrega pendências sem descartá-las |
| DET-C-04 | `pause` concorrente com `take_events` | Nenhuma nova reserva após confirmação da pausa |
| DET-C-05 | Lote reservado antes da pausa | `acknowledge`, `release_events` e expiração continuam válidos |
| DET-C-06 | Uma origem falha, outra funciona | `Degraded` agregado e estados/erros individuais fiéis |
| DET-C-07 | Rodada de observação em andamento | `status` informa atividade e contadores por origem |
| DET-C-08 | `stop` | Local e Remoto interrompidos, entrega pausada; eventos/checkpoints preservados |
| DET-C-09 | Reiniciar depois de `stop` | Ambos iniciam; Detector não executa análise/Scanner por iniciativa própria |
| DET-C-10 | Notificação de eventos durante pausa | Nenhum lote entregue por callback; sinalização de disponibilidade não burla pausa |

## 6. Isolamento de interfaces

- Consumidor de teste só importa `Detector`, requests/results, `File`, `FileBatch` e tipos públicos.
- Não pode importar `LocalDetector`, `RemoteDetector`, `EventStore` ou tipos SQLite através da fachada.
- O coordenador só usa portas de cada filho, sem SQL, watcher, HTTP ou algoritmos da Google API.
- Local real é testável sem Remoto/Storage concretos; Remoto real sem Local/Storage concretos; Storage real sem ambos nem coordenador real.
- A substituição de qualquer filho por fake não obriga reescrever os demais contratos.

## 7. Erros, opcionalidade e aceite

Testar `batch_id=None` somente para lote vazio, intervalos opcionais vazios em lote vazio, `event_available_sink=None`, identidade de sincronização, tipos de status, lease expirado/errado, requests inválidos, backpressure e falha de fonte.

**Aceite integral do coordenador:** 10/10 métodos e casos DET-B aplicáveis executados no Flatpak, sem precisar executar filhos concretos. Integração real dos quatro requer fase separada. Cobertura declarada precisa de comandos/resultados efetivamente executados.

**Esta é uma especificação documental; nenhum teste foi executado.**

## 7. Conformidade de ponta a ponta com Models

| Caso | Fluxo | Resultado obrigatório |
| --- | --- | --- |
| DET-M-01 | Local fake emite `File` com `FileData::Local` | Storage fake recebe o mesmo File integral; batch devolve File idêntico |
| DET-M-02 | Remote fake emite `File` com `FileData::Remote` e provider_data | Metadados especializados não são descartados ou projetados |
| DET-M-03 | File com `Deleted` e `data=None` | Evento entregue e reavaliado, sem fabricar LocalFile/RemoteFile nem descartar automaticamente |
| DET-M-04 | Rename/Move com `previous/current` | Localizações preservadas após enqueue, claim, release e retry |
| DET-M-05 | Duas fontes alteram mesmo recurso | Dois Files de IDs próprios, sem mesclagem/conflicto decidido |
| DET-M-06 | FileBatch com N=1 ou vários | Mesma API, origem preservada sem interpretação no lote |
| DET-M-07 | Nova versão dos atributos de LocalFile/RemoteFile | Fachada não modifica código nem lê campos individuais; payload intacto |
| DET-M-08 | Falha após commit antes de ack | Mesmo `File.id`, evento e payload ao reentregar |

O coordenador usa apenas Model `File`/`FileBatch` e portas internas dos filhos. Nenhum teste pressupõe `SyncItemId` para notificar alterações.
