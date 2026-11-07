# Detector — INTERFACES

## 1. Princípio e visibilidade

**Somente o módulo `detectors/` disponibiliza API pública para a engine.** O Orquestrador utiliza `Detector` e recebe `DetectorEvent`; **não importa, constrói, invoca ou configura** `LocalDetector` nem `RemoteDetector`. Os dois são componentes internos com implementações, modelos de dados, ciclos de leitura, estratégias de recuperação e testes próprios.

Esta especificação define a **fronteira funcional** a ser implementada em Rust, não código já compilado. A forma concreta de ownership, runtime e canais será definida durante a implementação preservando o significado dos tipos.

A fachada mantém uma API pequena: criar, iniciar, pausar, retomar, parar, solicitar observação e consultar status. **Sincronizar arquivos, selecionar versões, resolver conflitos ou executar operações físicas não são métodos do Detector.**

## 2. Tipos públicos do Detector

```rust
pub type DetectorResult<T> = Result<T, DetectorFailure>;

// Tipos de identidade são reutilizados do Inventário.
// SyncId, SyncItemId e Timestamp não são redefinidos aqui.

pub enum DetectorState {
    Stopped, Starting, Running, Paused, Degraded, Stopping, Failed
}
pub enum DetectorStartMode { Initial, ResumeAfterRestart }
pub enum DetectorPauseReason { UserRequest, Policy }
pub enum DetectorStopReason { UserRequest, Shutdown }
pub enum DetectorScanScope {
    All,
    Items(Vec<SyncItemId>),
}
pub enum DetectorScanReason {
    Startup, Recovery, Notification, Manual, ConsistencyCheck,
}
pub enum DetectorEventKind {
    ChangeObserved,
    RecheckRequired,
    ScopeUnavailable,
    ObservationRecovered,
}
pub enum DetectorCoverage {
    Complete, Partial, Unavailable,
}
pub enum DetectorError {
    InvalidScope, InvalidState, Unavailable, PermissionDenied,
    PartialObservation, DependencyUnavailable, Cancelled,
    NotificationFailed, InvalidIdentity, Internal,
}
pub struct DetectorFailure {
    pub kind: DetectorError,
    pub message: String,  // sanitizada
    pub retryable: bool,
}
pub struct DetectorEvent {
    pub sync_id: SyncId,
    pub kind: DetectorEventKind,
    pub affected_items: Vec<SyncItemId>,
    pub coverage: DetectorCoverage,
    pub observed_at: Timestamp,
    pub persisted: bool,
    pub correlation_id: Option<String>,
}
pub struct DetectorStatus {
    pub state: DetectorState,
    pub last_observation_at: Option<Timestamp>,
    pub last_error: Option<DetectorError>,
    pub pending_checks: u32,
    pub degraded_sources: u32, // contagem; sem expor implementações
}
```

**Evento unificado:** `DetectorEvent` não contém estruturas do filesystem, recursos JSON do Google, handles, cursores, caminhos ou tipos internos. **O Orquestrador não precisa conhecer a origem local/remota para decidir que o item deve ser reavaliado**; ao receber `affected_items`, encaminha os IDs à análise. Sem ID seguro, uma lista vazia significa reavaliar escopo, não ausência de mudança.

`persisted=true` significa **apenas** que os fatos necessários para aquela notificação foram confirmados pelo Inventário. `persisted=false` admite diagnóstico sem observação persistida; o Orquestrador não deve assumir estado atualizado. Nem um nem outro significa sincronização concluída. O detalhamento da situação deve ser consultado no Inventário.

## 3. Portas de integração e requests/results

```rust
pub trait DetectorEventSink: Send + Sync {
    fn publish(&self, event: DetectorEvent) -> DetectorResult<()>;
}

// Os handles abaixo designam dependências tipadas; não são storage bruto.
// Tipo Rust concreto de ownership/trait object será fechado na integração.
pub struct DetectorInventoryHandle { /* API pública do Inventário */ }
pub struct DetectorEventSinkHandle { /* DetectorEventSink */ }

pub struct DetectorCreateRequest {
    pub sync_id: SyncId,
    pub inventory: DetectorInventoryHandle,
    pub event_sink: DetectorEventSinkHandle,
}
pub struct DetectorCreateResult {
    pub detector: Detector,
    pub state: DetectorState,
}
pub struct DetectorStartRequest {
    pub mode: DetectorStartMode,
    pub requested_at: Timestamp,
}
pub struct DetectorStartResult {
    pub state: DetectorState,
    pub check_scheduled: bool,
}
pub struct DetectorPauseRequest {
    pub reason: DetectorPauseReason,
    pub requested_at: Timestamp,
}
pub struct DetectorPauseResult { pub state: DetectorState }
pub struct DetectorResumeRequest { pub requested_at: Timestamp }
pub struct DetectorResumeResult {
    pub state: DetectorState,
    pub recovery_scheduled: bool,
}
pub struct DetectorStopRequest {
    pub reason: DetectorStopReason,
    pub requested_at: Timestamp,
}
pub struct DetectorStopResult {
    pub state: DetectorState,
    pub in_flight: u32,
}
pub struct DetectorScanRequest {
    pub scope: DetectorScanScope,
    pub reason: DetectorScanReason,
    pub request_id: Option<String>,
}
pub struct DetectorScanResult {
    pub accepted: bool,
    pub correlation_id: Option<String>,
}
pub struct DetectorStatusRequest {}
pub struct DetectorStatusResult { pub status: DetectorStatus }
```

**Encapsulamento:** o `Detector` compõe suas fontes **internamente** a partir da identidade/configuração disponível e das portas já injetadas. `DetectorCreateRequest` não expõe `LocalDetector`, `RemoteDetector`, `InternalObservationSource`, `DetectorInternalSources` ou seus handles. A forma concreta de obtenção de configurações/credenciais das fontes é dependência a resolver sem alterar essa regra de visibilidade.

## 4. Assinaturas públicas consolidadas

```rust
pub struct Detector { /* implementação privada */ }

impl Detector {
    pub fn new(request: DetectorCreateRequest) -> DetectorResult<DetectorCreateResult>;
    pub fn start(&self, request: DetectorStartRequest) -> DetectorResult<DetectorStartResult>;
    pub fn pause(&self, request: DetectorPauseRequest) -> DetectorResult<DetectorPauseResult>;
    pub fn resume(&self, request: DetectorResumeRequest) -> DetectorResult<DetectorResumeResult>;
    pub fn stop(&self, request: DetectorStopRequest) -> DetectorResult<DetectorStopResult>;
    pub fn request_scan(&self, request: DetectorScanRequest) -> DetectorResult<DetectorScanResult>;
    pub fn status(&self, request: DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>;
}
```

**Sete métodos públicos**, além de `DetectorEventSink.publish` como porta de emissão assíncrona para o Orquestrador. Não criar versões públicas `local_start`, `remote_poll` ou `watch_path`.

## 5. Contrato funcional de cada método público

### DET-01 — `new`

**Assinatura:** `new(DetectorCreateRequest) -> DetectorResult<DetectorCreateResult>`.

**Finalidade:** Criar a fachada de detecção de uma sincronização, conectando-a ao Inventário, às implementações internas disponíveis e ao destino de eventos do Orquestrador.

**Parâmetros:** `sync_id`: identidade da sincronização; `inventory`: porta pública do Inventário; `event_sink`: destino de mensagens; fontes internas são construídas pelo próprio `Detector`, sem argumentos públicos específicos de Local/Remoto.

**Retorno:** Devolve `Detector` em estado `Stopped` e as capacidades disponíveis; criar não inicia inspeções.

**Limites e erros:** Não expõe handles Local/Remoto. Falhar para identidade inválida, nenhuma origem disponível quando exigida ou dependências incompatíveis.

### DET-02 — `start`

**Assinatura:** `start(DetectorStartRequest) -> DetectorResult<DetectorStartResult>`.

**Finalidade:** Iniciar o acompanhamento das fontes configuradas e coordenar a leitura de retomada necessária.

**Parâmetros:** `mode`: inicialização ou retomada; `requested_at`: instante da solicitação. O Orquestrador não determina mecanismo de watcher ou polling.

**Retorno:** Estado agregado e informação de trabalho agendado; iniciar não equivale a completar leitura.

**Limites e erros:** O estado pode ser `Starting`/`Degraded`; falha de uma fonte não é exclusão dos arquivos daquela fonte.

### DET-03 — `pause`

**Assinatura:** `pause(DetectorPauseRequest) -> DetectorResult<DetectorPauseResult>`.

**Finalidade:** Suspender novas observações automáticas mantendo os fatos já persistidos e a configuração da sincronização.

**Parâmetros:** `reason`: justificativa de controle; `requested_at`: momento da solicitação.

**Retorno:** Estado `Paused` ou transição/erro declarado; dados já persistidos continuam disponíveis.

**Limites e erros:** Não limpa eventos, não apaga cursor nem baseline e não cancela operações do Analisador.

### DET-04 — `resume`

**Assinatura:** `resume(DetectorResumeRequest) -> DetectorResult<DetectorResumeResult>`.

**Finalidade:** Retomar acompanhamento e organizar recuperação do intervalo em que não houve observação.

**Parâmetros:** `requested_at`: instante de retomada; o plano concreto de recuperação pertence a cada fonte interna.

**Retorno:** Estado agregado e indicação de verificação de recuperação agendada.

**Limites e erros:** Não supõe que eventos perdidos sejam recuperáveis apenas por watcher; não afirma sincronização concluída.

### DET-05 — `stop`

**Assinatura:** `stop(DetectorStopRequest) -> DetectorResult<DetectorStopResult>`.

**Finalidade:** Encerrar os mecanismos internos de detecção e liberar recursos da instância.

**Parâmetros:** `reason`: parada solicitada ou encerramento; `requested_at`: instante.

**Retorno:** Estado `Stopped` ou erro e trabalho ainda pendente, quando houver.

**Limites e erros:** Não descarta observações persistidas, histórico nem estruturas de sincronização do Inventário.

### DET-06 — `request_scan`

**Assinatura:** `request_scan(DetectorScanRequest) -> DetectorResult<DetectorScanResult>`.

**Finalidade:** Solicitar reavaliação, independentemente do acompanhamento automático, sem dizer como cada origem irá observar.

**Parâmetros:** `scope`: `All` ou conjunto de `SyncItemId` conhecidos; `reason`: justificativa; `request_id?`: correlação opcional.

**Retorno:** Identificador de solicitação e aceitação/agendamento, sem prometer resultado de leitura síncrona.

**Limites e erros:** Não executa upload/download; item desconhecido ou escopo inseguro gera erro tipado ou aviso de reavaliação por escopo, conforme contrato.

### DET-07 — `status`

**Assinatura:** `status(DetectorStatusRequest) -> DetectorResult<DetectorStatusResult>`.

**Finalidade:** Consultar o estado agregado da detecção e diagnóstico de funcionamento.

**Parâmetros:** Objeto de consulta vazio; a instância já identifica a sincronização.

**Retorno:** `status`: estado agregado, última observação, último erro e verificações pendentes quando conhecidos.

**Limites e erros:** Não revela implementação de watcher/polling, não exige conhecer Local/Remoto nem provoca observação.

## 6. Obrigatoriedade e ausência

| Campo | Obrigatoriedade | Significado de ausência ou valor |
| --- | --- | --- |
| `sync_id` | Obrigatório | Isola notificações e solicitações de uma sincronização |
| `affected_items` | Lista obrigatória, possivelmente vazia | Vazia exige reavaliação geral do escopo, não significa zero mudanças |
| `coverage` | Obrigatório | `Partial`/`Unavailable` não autorizam inferir exclusão |
| `persisted` | Obrigatório | Distingue observação confirmada e aviso diagnóstico sem persistência |
| `correlation_id`/`request_id` | Opcionais | Sem ID significa ausência de correlação explícita, não evento inválido |
| `last_observation_at` | Opcional | Sem observação registrada ainda |
| `last_error` | Opcional | Nenhum erro conhecido, **não** garantia de disponibilidade atual |
| `pending_checks` | Obrigatório | Contagem de solicitações ainda não finalizadas |
| `degraded_sources` | Obrigatório | Quantidade de fontes internas degradadas, sem expor implementação |
| `DetectorScanScope::Items` | Requer IDs válidos | Lista vazia é inválida: usar `All` para escopo completo |

## 7. Interface interna entre Detector e fontes

Esta interface é **privada ao módulo Detector**, não reexportada pela engine:

```rust
// Contrato conceitual PRIVADO, não API do Orquestrador:
trait InternalObservationSource {
    fn start(&self, context: InternalStartContext) -> InternalResult<()>;
    fn pause(&self) -> InternalResult<()>;
    fn resume(&self) -> InternalResult<()>;
    fn stop(&self) -> InternalResult<()>;
    fn request_observation(&self, request: InternalObservationRequest)
        -> InternalResult<()>;
    fn status(&self) -> InternalSourceStatus;
}
```

A interface compartilhada padroniza **apenas a comunicação/controle mínimo**. O Local escolhe sua organização de watcher, varredura, symlinks e bytes Unix. O Remoto escolhe paginação, cursor, polling e gestão de respostas Google. Podem manter tipos e funções internas diferentes e adaptar-se ao contrato comum. **Não forçar as duas implementações a terem a mesma arquitetura interna.**

As fontes produzem eventos internos, que o pai traduz em `DetectorEvent`. Não fornecem diretamente o `DetectorEventSink` do Orquestrador. O pai coordena validação, persistência quando necessária e emissão externa; detalhes de quem persiste cada observação devem ser definidos sem violar a API do Inventário.

## 8. Segurança, estados e limites

- `request_scan` solicita **observação**, não sincronização física.
- O funcionamento de uma fonte não implica funcionamento da outra; o status agregado pode ser `Degraded`. Não esconder falhas por entregar uma notificação de sucesso.
- Eventos internos podem ser duplicados, agrupados ou perdidos. Após reinício, o Detector exige recuperação pelos mecanismos apropriados de cada fonte e estado durável do Inventário.
- Falha de persistência deve produzir diagnóstico sem `persisted=true`.
- `Complete` exige evidência de cobertura dentro do escopo; erro de leitura não prova exclusão.
- O Detector não acessa SQLite internamente por SQL, não modifica baseline nem fila e não decide conflitos.

## 9. Pendências explícitas antes de implementar

Definir ownership concretamente compilável de `DetectorCreateRequest`, `DetectorInternalSources` e do sink; coordenação async/worker/cancelamento; critérios de transição e estado agregado; fonte das solicitações de reavaliação por escopo; responsabilidades exatas de persistência entre pai e filhos. Nenhuma dessas decisões dá ao Orquestrador acesso aos tipos internos. Não inventar assinaturas públicas adicionais para contornar lacunas.

[README.md](README.md) descreve o módulo; [TESTS.md](TESTS.md) define sua verificação; [AGENTS.md](AGENTS.md) regula a implementação. **Esta é uma proposta documental, ainda não compilada nem testada.**
