# Orchestrator — INTERFACES (API de coordenação interna)

## Natureza

Contrato **interno, conceitual e não implementado**. Não é a API pública de `facade/`; todas as integrações usam APENAS interfaces públicas dos módulos, mesmo que a implementação do Orchestrator possua funções privadas de coordenação. A API `facade/` não está documentada na branch atual (README vazio); portanto as assinaturas abaixo não são apresentadas como fachada externa validada.

O âmbito lógico de uma execução é um `SyncId`, associado a identidade de conta, raiz local e raiz remota validadas por seus proprietários. Não fixar instância por conta, instância global ou scheduler neste documento.

## 1. Portas dependentes (contratos existentes)

| Porta usada | Contrato existente | Finalidade da coordenação |
| --- | --- | --- |
| `Detector` | `new/start/pause/resume/stop/request_scan/status/take_events/acknowledge/release_events` | Observação e fila durável própria |
| `Scanner` | `new/scan/status` | Varredura **local**, com gravação via Inventory |
| `InventoryManager` | `open/get_sync_info/get_item/list_items/record_local/record_remote/link_item/update_item_status/confirm_sync/enqueue_operation/list_operations/claim_operation/update_operation/record_activity/list_activities/status` | Estado persistente por sincronização |
| `Analyzer` | `new/analyze/status` | Análise e execução de operações de **um** `SyncItem` |
| `Accounts` | `get/acquire_session/status` e operações próprias | Conta e autorização, quando necessário preparar uma sessão; sem acesso ao keyring |
| `Configuration` | `get` e substituições tipadas opcionais | Valores ajustáveis quando aplicáveis; defaults pertencem aos consumidores |

O Orchestrator **não chama `Operator`, `Providers`, `Detector.Local`, `Detector.Remote`, `Detector.Storage`, `Inventory.Storage` nem subalgoritmos do Analyzer diretamente**. O Analyzer é o único coordenador de comandos físicos. O Scanner já usa o Inventory para suas observações; o Orchestrator não repete essas gravações.

## 2. API de coordenação interna — esboço

```rust
// Nomes e ownership conceituais; não adicionar uma API externa nova sem revisar facade.
pub type CoordinationResult<T> = Result<T, CoordinationFailure>;

pub struct SyncCoordinationContext {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    // Identidades/raízes resolvidas e validadas pelos módulos proprietários;
    // handles internos dos contratos injetados, sem SQL/token.
}
pub struct CoordinationDependencies {
    pub detector: DetectorHandle,
    pub scanner: ScannerHandle,
    pub inventory: InventoryHandle,
    pub analyzer: AnalyzerHandle,
    // Accounts/Configuration podem participar da composição, sem obrigar cada operação a consultá-los.
}
pub struct StartCoordinationRequest {
    pub sync_id: SyncId,
}
pub struct StopCoordinationRequest {
    pub sync_id: SyncId,
    pub stop_detection: bool, // true apenas para interrupção física explicitamente solicitada
}
pub struct ProcessEventsRequest {
    pub sync_id: SyncId,
    pub max_events: u32,
    pub lease_duration: Duration,
}
pub struct ProcessEventsReport {
    pub observations_persisted: u32,
    pub impacted_items: Vec<SyncItemId>,
    pub acknowledged: bool,
    pub requires_reobservation: bool,
}
pub struct AnalyzeItemRequest {
    pub sync_id: SyncId,
    pub item_id: SyncItemId,
    pub resolution: Option<Resolution>,
}
pub struct AnalyzeItemResult {
    pub outcome: AnalysisOutcome,
}
pub struct CoordinationStatusRequest {
    pub sync_id: SyncId,
}
pub struct CoordinationStatus {
    pub sync_id: SyncId,
    pub detector: DetectorStatus,
    pub scanner: ScannerStatus,
    pub inventory: InventoryStatusResult,
    // Registro das análises ativas só se existir contrato de enumeração;
    // Analyzer.status() aceita item_id, não lista todas as análises.
}

pub(crate) trait SyncCoordinator {
    fn start(&self, request: StartCoordinationRequest) -> CoordinationResult<()>;
    fn stop(&self, request: StopCoordinationRequest) -> CoordinationResult<()>;
    fn process_events(&self, request: ProcessEventsRequest)
        -> CoordinationResult<ProcessEventsReport>;
    async fn analyze_item(&self, request: AnalyzeItemRequest)
        -> CoordinationResult<AnalyzeItemResult>;
    fn status(&self, request: CoordinationStatusRequest)
        -> CoordinationResult<CoordinationStatus>;
}
```

**Atenção à assincronicidade:** as assinaturas `start/stop/process_events` estão apresentadas como esboço de orquestração; se aguardarem Scanner, Detector ou armazenamento bloqueante, deverão ser `async` ou executadas fora da main thread GTK. O Orchestrator não decide aqui runtime, número de threads ou concorrência global. `start` não significa Scanner completo de forma síncrona; conclusão e falhas da etapa devem ser observáveis e/ou comunicadas por resultado próprio quando sua assinatura definitiva for escolhida.

**Atenção à composição:** `CoordinationDependencies` é um conjunto lógico de portas injetadas, **não** obrigação de construir um objeto completo por conta ou por sincronização. As portas concretas serão definidas após a revisão de instanciamento.

## 3. Primitivas de coordenação (funções internas, não contrato externo)

```text
initialize_sync(sync_id):
  confirmar escopo, identidade e disponibilidade do Inventory
  preparar Detector e Scanner para o mesmo SyncId
  Detector.start(delivery = Paused)  // sempre Local + Remoto
  Scanner.scan(sync_id)             // observação local; relatório pode ser parcial
  examinar cobertura e estado das fontes
  Detector.resume() quando for seguro consumir eventos
  consumir e reavaliar itens elegíveis; não assumir completude remota

process_detector_batch(sync_id):
  Detector.take_events(max_events, lease_duration)
  validar sync_id e preservar cada File integral
  para cada File:
    resolver identidade com Inventory.get_item / record_*;
    construir ObservationProof sem inventar cobertura
    Inventory.record_local OU record_remote
    em falha: release ou expiração; não acknowledge
  após todos os eventos duravelmente aceitos: Detector.acknowledge(Processed)
  ler SyncItems efetivos e pedir análise de itens elegíveis

analyze_item(sync_id, item_id, resolution):
  Inventory.get_item(Id(item_id)) -> SyncItem atual
  Analyzer.analyze(AnalyzeRequest { item, resolution })
  se Pending: comunicar necessidade de decisão sem bloquear
  se Failed: preservar indeterminação e caminho de recuperação
  se Completed: aceitar apenas confirmação realmente registrada por Analyzer

resume_pending(sync_id, pending_id, selected_option):
  recuperar pendência durável (API AINDA NÃO DEFINIDA)
  vincular SyncId/SyncItemId, revisão, fingerprint e opção válida
  construir Resolution e chamar analyze_item
```

**Não existe caminho confiável para `resume_pending` sem API durável de pendências.** Não implementar armazenamento improvisado em memória do Orchestrator.

## 4. Semântica de entrega e checkpoint

- `Detector.pause()` impede novos claims, **não** o registro local/remoto. `Detector.stop()` interrompe a observação; decisão de cobertura futura fica com o Orchestrator.
- `take_events` retorna um `FileBatch` único com origem preservada. Se vier vazio, não há lote para ack e isso não prova ausência de mudanças futuras.
- Processamento de `File` não é uma chamada de Analyzer. Primeiro é necessário registrar observação em Inventory e obter `SyncItem`.
- O ack é de **lote inteiro**. Se um item falhar, não confirmar lote inteiro; suportar redelivery seguro exige idempotência no Inventory por `File.id` **ainda ausente do contrato**.
- `Processed` significa aceitação durável das observações na fronteira Inventory, **não** upload/download verificado. `Discarded` não é uma forma automática de tratar erro ou conflito.
- `ObservationProof` e `PresenceState` precisam ser evidência extraída de `File`, escopo/cobertura e validação autorizada; não presumir `AbsentConfirmed` com base em evento `Deleted`.

## 5. Associação, pendências e estado

- Identidade: `AccountId`/ `SyncId`/ `SyncItemId`/ `RemoteFileRef`/ `File.id` têm semânticas distintas. Não usar nome ou caminho como prova de identidade global. Resolver ambiguidades requer reobservação ou `Pending` seguro.
- `Analyzer::status` aceita `AnalysisStatusRequest { item_id }` e informa apenas execução ativa naquela instância. **Não** declarar enumeração global de análises sem contrato adicional.
- `PendingAction` só tem retorno do Analyzer; persistência e listagem ainda **não** estão cobertas pelo Inventory. Exibir pendência transitória não é equivalente a garantir recuperação pós-crash.
- `Inventory::list_operations` registra intenções de sincronização; não executar diretamente a fila pelo Orchestrator.
- Status agregado deve carregar condições verdadeiras `Degraded`, `Unavailable`, `Pending` e cobertura parcial sem convertê-las em `Ready` fictício.

## 6. Erros, limites e teste de composição

Apenas erros de coordenação devem ser próprios (`InvalidSyncScope`, `DependencyUnavailable`, `InitialObservationIncomplete`, `PersistenceGap`, `NeedsUserDecision` e `Internal` são **categorias candidatas**, não novo enum compilado). Preservar erros tipados e origem dos módulos, sem esconder `DetectorFailure` ou `AnalysisFailure` nem expor segredo.

**Lacunas bloqueadoras:** (i) deduplicação durável por `File.id` e contrato de ack entre bancos, (ii) persistência/consulta de `PendingAction`/resoluções, (iii) diagnóstico de cobertura de Scanner no Inventory, (iv) evidência de cobertura remota inicial, (v) cadastro de conexão, (vi) recuperação de planos persistidos da fila. Não criar assinaturas fictícias de módulos proprietários para contornar essas lacunas.

**Estado:** contrato interno proposto para TDD; nenhum Rust compilado.
