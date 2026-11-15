# Orchestrator — fluxo da aplicação

## API e composição

Os métodos existentes `new`, `prepare`, `preparation_status`, `stop_detector`,
`detector_status`, `local_status`, `remote_diagnostics`, `diagnostics` e `connector`
mantêm as assinaturas. `OrchestratorCreateRequest` continua recebendo somente
`DetectorCreateRequest`. Detector, Scanner, Inventory e Analyzer são utilizados
pelas suas fachadas públicas, sem alteração de suas interfaces ou esquemas.

O callback existente `EventAvailableSink` é composto com um sinal interno de
condvar: o sink configurado pelo chamador continua recebendo disponibilidade.
O sinal não contém arquivos, não acessa SQL e não chama os controles bloqueantes
do Detector. A única fila de observações permanece no Detector.

## Preparação e estado

`prepare` inicia Local e Remote com `Paused`, abre Inventory/Scanner, aguarda scan
completo e solicita análise inicial. Só então inicia o consumidor e libera entrega.
O worker aguarda a publicação da preparação antes de consumir, evitando uma corrida
entre estados iniciais e o processamento contínuo. Scan Partial/falha não libera a
barreira. Chamadores GLib não precisam fornecer runtime Tokio: os workers o hospedam.

`PreparationState` conserva os valores anteriores e acrescenta `ProcessingEvents`.
`ReadyForAnalyzer` representa a preparação completa e o consumidor aguardando
observações; `ProcessingEvents` representa ingestão/rechecagem/análise. Nenhum deles
significa upload/download ou sincronização concluída.

`PreparationStatus` conserva seus campos e acrescenta `events_processed: u64`,
contador cumulativo em memória dos eventos confirmados depois de ingestão e análise
por esta instância. O contador não mede operações físicas nem é histórico oficial;
o Detector mantém os contadores duráveis. A UI usa a mudança desse snapshot para
reler os recibos do Analyzer, sem polling SQL no loop GTK.

`PreparationFailure` acrescenta `InvalidObservation(FileId)` para entrada inválida,
escopo errado ou recurso remoto sem associação/snapshot suficientes. Display não
expõe o payload. Erros de Inventory/Scanner/Analyzer permanecem tipados e visíveis.
Conflito ou evidência insuficiente retornados pelo Analyzer são resultados válidos,
não falhas de ingestão.

## Projeção das observações

- Validar `File`, `SyncId` e conta da referência remota.
- Resolver local por ID fornecido, ou pelo índice público de caminho relativo;
  resolver remoto por referência qualificada. Não correlacionar lados pelo nome.
- Registrar o `File` integral com seu ID original, revisão atual e prova de origem
  local/remota. `Some(data)` indica presença observada; `None` permanece `Unknown`.
  Um evento isolado nunca cria `AbsentConfirmed`.
- Reentrega reaproveita o recibo por `File.id` do Inventory, inclusive quando a revisão
  já foi avançada por análise anterior. Nenhum SQL é emitido pelo Orchestrator.
- Local com snapshot mais antigo que a observação persistida, diretório, lacuna ou
  ausência de metadados exige scan. Isso evita manter um estado local antigo após
  ingerir eventos acumulados durante o scan inicial.
- Rechecagem de escopo local exige Scanner completo. Observações ausentes sem item
  associado não fabricam registros vazios; a rechecagem trata os itens conhecidos.
- Avisos técnicos da raiz remota não inventam SyncItem e não provam cobertura. O
  estado técnico permanece no Detector; itens existentes são reavaliados.
- Evento remoto sem snapshot e sem item associado gera erro e continua pendente.

## Análise e ack

Observações remotas comuns entregam ao Analyzer os IDs afetados, recarregando seus
snapshots atuais. Rechecagem local/aviso global visita os itens atuais por paginação,
inclusive status Synced ou Conflict. O Scanner já persiste sua cobertura e suas
observações; o Orchestrator não reimplementa a coleta nem suas provas de ausência.

A montagem de `AnalysisEvidence` permanece igual à integração anterior: presença
conhecida por item, baseline confirmado pelo Inventory, conteúdo desconhecido sem
hash local e pais por referências/raízes explícitas. Esta entrega **não redefine
como o Analyzer compara ou resolve os dados**.

`acknowledge(Processed)` ocorre após análise bem-sucedida e persistente. Um guard
libera o lote inteiro em erro/cancelamento/Partial. Se a lease expirou ou o processo
caiu, a fila permite reentrega após expiração. Não há ack parcial ou descarte para
ocultar falhas. Os efeitos de ingestão já gravados são recuperáveis pelos recibos;
o ack nunca autoriza executar uma operação do Inventory.

## Ciclo de vida

Um worker é mantido por sessão preparada. Novo prepare invalida o epoch, sinaliza
e retira o worker anterior e aguarda sua saída fora de GTK antes de substituir os
módulos. Parada/cancelamento acorda o worker mesmo ocioso; scan ativo recebe pedido
de cancelamento e sua saída é aguardada. Uma análise já despachada pode terminar a
transação, mas o epoch é revalidado antes de confirmar eventos ou publicar estado.
O lock de controle impede processamento simultâneo com setup/parada.

Wakeups são incrementais e não se perdem entre consulta de fila e espera. Um
heartbeat de um segundo cobre leases abandonados. Lotes usam o limite público de
64 eventos e lease de 300 segundos; não há renovação/roubo de lease por SQL. Falha
técnica pausa entrega e publica Failed; Partial pausa sem fingir cobertura completa.
As fontes continuam capturando até uma parada física solicitada.

## Fora do escopo

Nenhum Operator, comando físico, resolução ou confirmação de baseline é chamado.
Não se libera `SyncOperation::Blocked`, não se faz replay de operações Running e
não se altera política de identidade/conflito/conteúdo do Analyzer. Cobertura
remota global e medição de conteúdo permanecem responsabilidade de seus módulos.
