# Orchestrator — interfaces da preparação e análise

## Superfície preservada

As assinaturas públicas compiladas continuam iguais:

- `new(OrchestratorCreateRequest) -> DetectorResult<Orchestrator>`;
- `prepare(&self) -> Result<Arc<ScanReport>, PreparationFailure>` (assíncrono);
- `preparation_status(&self) -> PreparationStatus`;
- `detector`, `stop_detector`, `detector_status`, `local_status`,
  `remote_diagnostics`, `diagnostics`, `connector`.

`OrchestratorCreateRequest` continua contendo apenas `detector: DetectorCreateRequest`.
Nenhuma interface de Detector, Scanner ou Inventory é alterada. A configuração
existente das fontes permanece como antes; não se introduz configuração de
transferências, pool global, runtime do chamador ou política automática de resolução.

## Estado e falha

`PreparationState` contém `Idle`, `Starting`, `Scanning`, **`Analyzing`**,
`ReadyForAnalyzer`, `Partial`, `Failed`, `Cancelled` e `Stopped`.
`Analyzing` é a passagem real de produção/persistência de decisões, sem execução.
O valor existente `ReadyForAnalyzer` passa a ser publicado após a varredura completa
e a passagem de análise bem-sucedida. Ele não significa sincronização concluída,
conteúdo equivalente ou ausência de conflitos: pendências duráveis são resultados
válidos da análise.

`PreparationFailure` conserva variantes de Detector, Inventory, Scanner,
AlreadyRunning e Internal e acrescenta `Analyzer(AnalyzerError)` para erro técnico
durante produção/persistência de decisões. Conflito ou evidência insuficiente são
recibos válidos no Inventory, não falhas da preparação.

`PreparationStatus` conserva `state`, `scanner`, `report` e `error`. Durante
`Analyzing`, `scanner` já pode estar `Completed` e `report` oferece o relatório
completo; não indicar Scanner ainda ativo. Falha do Analyzer preserva esse relatório.
A consulta lê apenas snapshots de memória, sem SQL ou espera pelo worker.

## Composição privada

`prepare` mantém Detector com entrega pausada, cria Inventory/Scanner pelas APIs
existentes, aguarda o scan e só então chama a passagem privada de análise.
O mesmo `Arc<InventoryManager>` é compartilhado pelo Scanner e pela fachada Analyzer.
Nenhum handle SQL, Provider, Operador ou algoritmo privado é exposto.

A passagem privada executa em thread finita, com runtime Tokio próprio em que o
Analyzer pode despachar seu trabalho bloqueante. Portanto `prepare` continua
funcionando sem runtime Tokio no chamador, inclusive sob GLib. Não é consumidor
permanente de eventos, worker de transferências ou fila de execução paralela.

Para cada página do Inventory (`list_items` com filtro de todos os status), a
passagem recarrega cada item via `get_item`, constrói `AnalysisEvidence` e chama
`Analyzer::analyze(AnalyzeRequest { sync_id, item, evidence, recorded_at })`.
As revisões e snapshots são revalidados pelo Analyzer e na transação do Inventory.
Uma observação concorrente pode gerar erro tipado de revisão; o rescan seguinte
permite retomar com estado novo, sem sobrescrever a observação.

## Montagem das evidências

- `baseline_confirmed`: baseline existente no Inventory, cujo contrato permite
  gravá-lo apenas por confirmação explícita;
- `local_scope_complete`/`remote_scope_complete`: presença do item diferente de
  `Unknown`, sem alegar cobertura global nem medição recente da árvore remota;
- comparações de conteúdo: sempre `Unknown`, pois a composição ainda não dispõe
  de prova de conteúdo local compatível com hash remoto;
- `local_parent`: consulta por caminho relativo do pai no Inventory, aceitando
  somente diretório presente;
- `current_parents`: comparação de referências qualificadas quando o pai lógico
  possui vínculo remoto ou quando o arquivo está na raiz local explicitamente
  vinculada à raiz remota. Zero/múltiplos pais ou mapeamento ausente ficam desconhecidos. Divergência
  textual contra a raiz configurada também fica desconhecida, pois a configuração
  pode usar um alias do provedor que esta camada não resolve.

Esta montagem fornece contexto e evidências, não reimplementa decisões de
reconciliação ou conflito. Nome/caminho igual nunca associa um item local a remoto.
Ausência de snapshot ou falha de fonte não é convertida em `AbsentConfirmed`.

## Parada, cancelamento e recuperação

O epoch existente invalida preparação antiga. A passagem verifica esse token antes
de inicializar, entre itens e depois de cada chamada. O lock de controle serializa
setup/análise com a parada física. Uma análise já despachada pode gravar seu recibo;
parada/cancelamento não promete rollback, mas impede iniciar o próximo item quando
o token invalidado é observado. Nenhum desses caminhos executa operação física.

Erro técnico termina em `Failed` com causa. Decisões de um prefixo já processado
permanecem duráveis; a passagem inteira não é uma transação global. Um próximo
`prepare` refaz observação e visita todos os itens, inclusive aqueles cujo status
anterior era `Synced`, conflito ou pendência.

## Funcionalidades não ativadas

A fila Detector continua `Paused`, sem `take_events`, ack, release ou descarte
pelo Orchestrator. Ingestão contínua `Detector → Inventory → Analyzer`, política
de redelivery/ack e recuperação de gaps serão tratadas separadamente, sem alterar
interfaces de outros módulos por conveniência. Não criar consumidor parcial que
reconheça eventos antes de sua ingestão durável.

Comandos, execução, progresso de transferências, opções de resolução, confirmação
de baseline e recuperação de efeitos físicos ficam para o Operator e sua integração
posterior. A única saída do Analyzer nesta etapa é o recibo persistente de decisões.
