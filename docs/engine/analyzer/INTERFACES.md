# Analisador — INTERFACES

## 1. Contrato público e tipos proprietários

O Analisador oferece **construção, processamento individual e consulta de execução em andamento por item**. A API pública recebe um `SyncItem` e uma resolução opcional; não recebe lotes nem delega ao chamador a execução das operações.

Os tipos importados abaixo **não devem ser redefinidos**:
- De [Inventário](../inventory/INTERFACES.md): `SyncItem`, `SyncItemId`, `SyncBaseline`, `PresenceState`, `SyncStatus`, `OperationId`, `SyncOperation`, `InventoryManager`, estruturas de operações, confirmação e atividades.
- De [Operador](../operations/INTERFACES.md): `Operator`, `OperatorError`, `CreateFileRequest`, `CreateDirectoryRequest`, `UploadRequest`, `DownloadRequest`, `UpdateContentRequest`, `RenameRequest`, `MoveRequest`, `DeleteRequest`, `TrashRequest`, `RestoreRequest`, `ItemResult`, `TransferResult`, `DeleteResult`, `VerificationStatus`.
- De Models, indiretamente via Inventário: `LocalFile`, `RemoteFile`, identidades e metadados observados.

O código Rust abaixo representa a **assinatura-alvo para TDD**, não uma implementação existente. `AnalyzerDependencies` e demais tipos próprios serão definidos no módulo, preservando os contratos já documentados.

```rust
pub type AnalyzerResult<T> = Result<T, AnalyzerError>;

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> AnalyzerResult<Self>;

    pub async fn analyze(
        &self,
        request: AnalyzeRequest,
    ) -> AnalyzerResult<AnalysisOutcome>;

    pub fn status(&self, request: AnalysisStatusRequest)
        -> AnalyzerResult<AnalysisStatusResult>;
}
```

## 2. Estado consultável de análises em andamento

```rust
pub enum AnalysisPhase {
    Validating,
    Reconciling,
    EvaluatingConflict,
    Executing,
    Verifying,
    Confirming,
}

pub struct AnalysisStatusRequest {
    pub item_id: SyncItemId,
}

pub struct AnalysisStatusResult {
    pub active: Option<ActiveAnalysis>,
}

pub struct ActiveAnalysis {
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub phase: AnalysisPhase,
    pub started_at: Timestamp,
    pub current_operation: Option<OperationKind>,
    pub completed_operations: u32,
    pub planned_operations: Option<u32>,
    pub transfer: Option<TransferProgressSnapshot>,
}

pub struct TransferProgressSnapshot {
    pub bytes_transferred: u64,
    pub total_bytes: Option<u64>,
}
```

O Analisador é **assíncrono por chamada**, não um serviço contínuo: `analyze()` executa um item e termina com `Completed`, `Pending` ou `Failed`. `status(item_id)` consulta **apenas uma execução ativa**, não uma fila de trabalho e não um histórico permanente. `active=None` significa que não há chamada ativa para aquele item **nesta instância**, não que ele está sincronizado, ausente ou livre de conflitos. O resultado terminal é sempre devolvido por `analyze()`, e o estado durável dos itens/operações pertence ao Inventário.

`status()` deve ler uma fotografia interna coerente, rápida e sem I/O remoto, SQLite ou bloqueio até o final da tarefa. `phase` identifica a etapa corrente, não a conclusão do algoritmo; `current_operation` usa a categoria `OperationKind` já pertencente ao Inventário, sem inventar enum paralelo. A classificação deverá distinguir comandos como criação de diretório quando necessário na implementação; se não houver correspondência fiel, **não fabricar uma categoria**, deixando `current_operation=None` e usando a etapa `Executing`. `planned_operations=None` quando a lista ainda não foi construída. `transfer` só é preenchido quando existe progresso real reportado pelo Operador; não calcular porcentagens ou ETA sem total comprovado.

**Concorrência:** permitir execuções de itens independentes, mas recusar imediatamente outra `analyze()` para o **mesmo `SyncItemId`** na mesma instância com `AnalyzerErrorKind::AlreadyRunning`, sem iniciar operação duplicada. Além dessa proteção local, toda execução mantém revisão e precondições do Inventário para prevenir concorrência entre instâncias. Não introduzir `pause`, `resume`, `stop` ou fila própria no Analyzer. Ao devolver `Pending`, não reter tarefa ativa aguardando usuário; uma nova invocação com `Resolution` revalida as evidências.

## 3. Configuração e entrada

```rust
pub struct AnalyzerConfig {
    pub dependencies: AnalyzerDependencies,
}

// Contêiner conceitual: dependências por interfaces injetadas.
// A representação concreta (Arc, lifetimes, traits) é definida em Rust.
pub struct AnalyzerDependencies {
    pub inventory: InventoryAccess,
    pub operator: OperatorAccess,
}

pub struct AnalyzeRequest {
    pub item: SyncItem,
    pub resolution: Option<Resolution>,
}

pub struct Resolution {
    pub pending_id: PendingId,
    pub selected_option: ResolutionOptionId,
    pub expected_item_revision: u64,
    pub expected_state: StateFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PendingId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResolutionOptionId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateFingerprint {
    pub baseline: StateToken,
    pub local: StateToken,
    pub remote: StateToken,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateToken {
    Verified(String),
    Unknown,
}
```

**`AnalyzerConfig.dependencies`:** recebe os acessos já configurados ao inventário e ao operador, vinculados ao contexto da sincronização. Não entrega conexões SQLite, tokens de autenticação ou clientes HTTP ao Analisador. As interfaces injetadas permitem substituir as dependências por fakes em testes.

**`AnalyzeRequest.item` (obrigatório):** fotografia de um único `SyncItem`, com ID lógico, revisão otimista, snapshots `local` e `remote`, presenças e baseline opcionais. A ausência de `local` ou `remote` não prova exclusão: consultar também `local_presence` e `remote_presence`. Um baseline `None` significa que não há estado confirmado anterior; não fabricar uma versão de referência.

**`AnalyzeRequest.resolution` (opcional):** `None` inicia ou reavalia o item sem intervenção externa; `Some` corresponde à resposta a uma pendência identificada anteriormente. A resolução deve apontar para `PendingId` e para uma opção oferecida para aquele mesmo item, além da revisão e evidências que embasaram a escolha. Resposta obsoleta não autoriza executar comandos.

**`StateFingerprint`:** identifica o conjunto de evidências verificado na análise anterior. `StateToken::Verified` é um identificador determinístico de evidência do estado (não um hash inventado); `Unknown` representa insuficiência de prova. A estratégia de composição e persistência é decisão de implementação; o token nunca substitui o controle de revisão do Inventário.

## 3. Retorno principal

```rust
pub enum AnalysisOutcome {
    Completed(CompletionReport),
    Pending(PendingAction),
    Failed(AnalysisFailure),
}

pub enum CompletionKind {
    Unchanged,
    OperationsApplied,
}

pub struct CompletionReport {
    pub item_id: SyncItemId,
    pub final_item_revision: u64,
    pub kind: CompletionKind,
    pub operations: Vec<ExecutedOperation>,
}

pub struct ExecutedOperation {
    pub command: OperationCommand,
    pub result: OperationExecutionResult,
    pub operation_id: Option<OperationId>,
}

pub enum OperationExecutionResult {
    Item(ItemResult),
    Transfer(TransferResult),
    Delete(DeleteResult),
}
```

**`Completed`:** o item já estava consistente (`Unchanged`, lista vazia) ou todas as operações requeridas foram aplicadas, verificadas e confirmadas no Inventário (`OperationsApplied`, lista não vazia). `final_item_revision` é a revisão realmente obtida após persistência, não uma revisão prevista.

**`ExecutedOperation.command`:** comando completo efetivamente solicitado, na ordem de execução. **`result`:** retorno tipado emitido pelo Operador e validado antes da confirmação. **`operation_id`:** identidade persistida quando a operação foi registrada em fila; `None` não permite inferir ausência de histórico.

**Semântica da verificação:** `OperatorResult::Ok` com `VerificationStatus::Unverified` não autoriza tratar a sincronização como confirmada. O Analisador deve obter evidência suficiente antes de avançar o baseline.

## 4. Comandos de operação

```rust
pub enum OperationCommand {
    CreateFile(CreateFileRequest),
    CreateDirectory(CreateDirectoryRequest),
    Upload(UploadRequest),
    Download(DownloadRequest),
    UpdateContent(UpdateContentRequest),
    Rename(RenameRequest),
    Move(MoveRequest),
    Delete(DeleteRequest),
    Trash(TrashRequest),
    Restore(RestoreRequest),
}
```

O `OperationCommand` **pertence conceitualmente à integração com o Operador** e reutiliza seus dez tipos de requisição. A definição Rust pode ser atribuída ao próprio módulo Operador na integração, para evitar ciclo de dependências; o Analisador não criará uma segunda cópia dos comandos. Cada variante exige exatamente a requisição tipada correspondente.

A ordem de operações e suas dependências são definidas no plano produzido pelo algoritmo de reconciliação. O Analisador deve respeitá-las. Uma solicitação sem precondições suficientes para uma ação potencialmente destrutiva retorna `Pending` ou falha segura; não completar precondições por adivinhação.

## 5. Pendência e resolução

```rust
pub struct PendingAction {
    pub id: PendingId,
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub reason: PendingReason,
    pub context: PendingContext,
    pub options: Vec<ResolutionOption>,
    pub operations: Vec<ExecutedOperation>,
}

pub enum PendingReason {
    Conflict,
    InsufficientEvidence,
    ExternalDecisionRequired,
}

pub struct PendingContext {
    pub baseline: Option<SyncBaseline>,
    pub local: Option<LocalFile>,
    pub remote: Option<RemoteFile>,
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub fingerprint: StateFingerprint,
    pub explanation: String,
}

pub struct ResolutionOption {
    pub id: ResolutionOptionId,
    pub label: String,
    pub meaning: ResolutionMeaning,
}

pub enum ResolutionMeaning {
    KeepLocal,
    KeepRemote,
    KeepBoth,
    RetryAfterObservation,
    Defer,
}
```

**`PendingAction.id`:** identifica uma pendência concreta e precisa ser estável durante sua apresentação e recuperação. **`item_revision`:** versão do item na criação da pendência. **`operations`:** efeitos já verificados durante a chamada, se ocorreram, para permitir retomada sem repetição. **`reason`:** distingue conflito, observação insuficiente e decisão externa.

**`context`:** snapshots e presenças usados para explicar a pendência; campos ausentes permanecem desconhecidos. **`options`:** somente alternativas efetivamente seguras naquele estado e suportadas pelas capacidades do recurso. Se a única alternativa é coletar evidências, devolver `RetryAfterObservation`; uma pendência pode conter uma ou mais opções e não deve oferecer uma escolha que implique perda silenciosa de dados.

**`ResolutionMeaning`:** representa a intenção da escolha; os comandos físicos derivados continuam responsabilidade do Analisador. `KeepBoth` exige plano que preserve os dois conteúdos com nomes/identidades não conflitantes e confirmação antes de apagar qualquer cópia. `Defer` conserva a pendência sem executar operações.

**Reanálise:** `Resolution` não é comando de execução. O Analisador revalida `PendingId`, opção, identidade, revisão e evidências correntes, só então produz os comandos aplicáveis. Se as condições mudaram, retorna uma nova pendência ou falha tipada; não aplica uma decisão antiga ao estado novo. O armazenamento durável da pendência e de suas opções precisa ser acordado na integração com o Inventário; o contrato atual de `InventoryManager` não declara uma tabela/API específica para `PendingAction`.

## 6. Falha de análise e erros

```rust
pub struct AnalysisFailure {
    pub item_id: SyncItemId,
    pub stage: AnalysisStage,
    pub reason: AnalysisFailureReason,
    pub operations: Vec<ExecutedOperation>,
    pub interrupted_command: Option<OperationCommand>,
    pub retry: RetryAdvice,
}

pub enum AnalysisStage {
    Validation,
    Reconciliation,
    ConflictEvaluation,
    Execution,
    Confirmation,
}

pub enum AnalysisFailureReason {
    InvalidState,
    InconsistentEvidence,
    OperationFailed,
    OperationOutcomeUnknown,
    InventoryUpdateFailed,
    Internal,
}

pub enum RetryAdvice {
    RetryAnalysis,
    ReobserveBeforeRetry,
    WaitForExternalRecovery,
    DoNotRetryAutomatically,
}

pub struct AnalyzerError {
    pub kind: AnalyzerErrorKind,
    pub message: String,
}

pub enum AnalyzerErrorKind {
    InvalidConfiguration,
    InvalidRequest,
    AlreadyRunning,
    WrongSyncScope,
    DependencyUnavailable,
    StaleResolution,
    Internal,
}
```

**`AnalysisOutcome::Failed`** é um desfecho válido de processamento que precisa preservar o histórico de efeitos já observados. `interrupted_command` identifica a operação cujo resultado foi falho ou incerto. Se seu efeito remoto for indeterminado, informar `OperationOutcomeUnknown` e `ReobserveBeforeRetry`, nunca iniciar replay automático.

**`AnalyzerResult::Err`** representa erro de construção ou impossibilidade de iniciar a chamada com um contrato válido, como configuração inválida, `SyncItem` de outra sincronização ou resolução obsoleta. Após iniciar o processamento, falhas operacionais com efeitos conhecidos ou incertos devem preferir `AnalysisOutcome::Failed`, que preserva as operações executadas.

A mensagem de erro é sanitizada e não inclui tokens, credenciais, conteúdo do usuário ou caminhos desnecessários. Tipos técnicos subjacentes permanecem disponíveis ao diagnóstico em um mecanismo seguro, sem vazar detalhes sensíveis na API de apresentação.

## 7. Métodos públicos — contrato funcional

### `Analyzer::status`

**Assinatura:** `pub fn status(&self, request: AnalysisStatusRequest) -> AnalyzerResult<AnalysisStatusResult>`.

**Finalidade:** oferecer o estado transitório da execução atual de um `SyncItem` sem interferir na análise. Pode ser chamado enquanto `analyze()` está em andamento. Não emite decisões, não reserva eventos, não consulta Inventário nem executa operações.

**Resultado:** `ActiveAnalysis` quando o item está ativo nesta instância; `None` em caso contrário. A resposta é uma fotografia temporal, não um evento durável. Para o desfecho e histórico, utilizar `AnalysisOutcome` e o Inventário, respectivamente.

**Limites:** rejeitar requisições inválidas ou de escopo incorreto. A implementação deve assegurar progresso observável durante I/O longo, inclusive quando o Operador fornece `ProgressReporter`. Para comandos que ainda não oferecem progresso, informar etapa e contador de comandos, sem simular bytes ou porcentagem.


### `Analyzer::new`

**Assinatura:** `pub fn new(config: AnalyzerConfig) -> AnalyzerResult<Self>`.

**Finalidade:** construir o Analisador com as interfaces necessárias para leitura/confirmação do Inventário e execução pelo Operador.

**Parâmetros:** `config.dependencies.inventory` identifica o inventário da sincronização; `config.dependencies.operator` disponibiliza o executor da sessão correspondente. Ambos são obrigatórios e devem estar em contextos compatíveis.

**Retorno:** instância `Analyzer` pronta para receber chamadas individuais.

**Limites e erros:** valida dependências e identidade de sincronização quando verificável; retorna `InvalidConfiguration`, `WrongSyncScope` ou `DependencyUnavailable` conforme aplicável. Construir a instância não executa upload, download, exclusão ou reconciliação.

### `Analyzer::analyze`

**Assinatura:** `pub async fn analyze(&self, request: AnalyzeRequest) -> AnalyzerResult<AnalysisOutcome>`.

**Finalidade:** analisar um `SyncItem`, coordenar os algoritmos privados e executar as operações autorizadas em ordem, terminando em conclusão, pendência ou falha.

**Parâmetros:** `request.item` é obrigatório, com `id`, `revision`, snapshots, estados de presença e baseline. `request.resolution=None` solicita análise comum; `Some(resolution)` retoma pendência, exigindo `pending_id`, opção selecionada, revisão e evidências compatíveis.

**Retorno:**
- `Completed`: ausência de ação ou execução inteiramente verificada e confirmada, incluindo relatórios e revisão final.
- `Pending`: decisão externa ou reobservação necessária; devolve evidências, opções e operações já verificadas sem aguardar usuário.
- `Failed`: falha operacional ou de persistência durante processamento, com etapa, operações concluídas, comando interrompido e orientação de retry.

**Limites e erros:** solicitações sem identidade válida, fora do escopo ou com resolução inválida falham por `AnalyzerResult::Err`. Conflito não resolvido e evidência insuficiente normalmente produzem `Pending`, não erro técnico. Falhas de operação ou confirmação após início produzem `Failed`.

**Observabilidade:** atualizar fase, contadores e progresso interno durante a execução sem exigir que o Orquestrador conheça algoritmos privados. Reutilizar o progresso efetivo que o Operador fornece quando existir; nenhuma etapa de reconciliação/conflitos precisa produzir callbacks próprios. O estado ativo é retirado ao terminar a chamada, seja `Completed`, `Pending`, `Failed` ou `Err`.

**Efeitos colaterais:** pode executar **várias operações para um item**, mas nunca processa um lote. Antes de executar, deve verificar que a decisão permanece compatível com o estado e a revisão. A confirmação deve seguir as regras de `InventoryManager::confirm_sync`. Se uma operação foi aplicada e a confirmação falhou, a próxima chamada deve reobservar o efeito, não simplesmente repetir o comando.

**Concorrência:** análises simultâneas do mesmo `SyncItem` precisam ser serializadas por item ou rejeitadas por revisão/precondições, sem bloquear itens independentes. A forma concreta de sincronização é detalhe interno.

## 8. Coordenação interna e integração com Inventário

O Reconciliador e o algoritmo de Conflitos são **independentes** e retornam dados. Seus contratos serão detalhados em suas próprias documentações posteriormente; o Analisador não fixa nesta etapa a assinatura interna deles.

A saída do Reconciliador é uma **lista de transformações propostas** com ordem/dependências. O algoritmo de Conflitos avalia uma divergência contextualizada entre versões, devolvendo ausência de conflito, solução possível ou necessidade de decisão. O Analisador coordena ambos e transforma decisões aprovadas em comandos do Operador.

O Inventário é proprietário da fila, `SyncItem.revision`, status, baseline e atividades. O Analisador utiliza os métodos do `InventoryManager` e não executa SQL diretamente. Para comandos que serão persistidos, a ordem segura é registrar/reservar a intenção, executar, verificar efeitos e confirmar os estados em transação, respeitando os contratos vigentes. A API de `NewSyncOperation` atualmente expressa direção e tipo, mas **não carrega todos os parâmetros de `OperationCommand`**; persistência/reconstrução segura do plano concreto permanece questão explícita de integração, não uma capacidade já existente.

## 9. Garantias e decisões de implementação

1. A interface pública contém somente `new`, `analyze` e `status`; a análise continua por item, sem lote ou fila própria.
2. A resolução não congela a chamada nem dá autorização irrestrita para sobrescrita.
3. Nenhuma alteração destrutiva sem identidade, estado e precondições suficientes.
4. Desfechos parciais preservam as operações realmente executadas e verificadas.
5. `Completed` exige confirmação do baseline no Inventário; `OperatorResult::Ok` isolado não é suficiente.
6. `PendingAction` não é uma mera string de erro; contém identidade, evidências e alternativas aplicáveis.
7. A modelagem precisa resolver a persistência de pendências e comandos completos antes de prometer retomada após reinício.
8. Tipos e contratos acima são previstos para a primeira implementação; modelos externos seguem suas fontes proprietárias, sem duplicações.
