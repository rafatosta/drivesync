# Detector Remoto — INTERFACES

## 1. Visibilidade e tipos

Contrato **interno** consumido pelo `Detector`, não reexportado ao Orquestrador. O RemoteDetector é o contrato funcional que especialistas por provedor implementam; eles podem ter estruturas/algoritmos diferentes entre si e em relação ao Local. Não depende de uma implementação real do pai nem de `storage/`; recebe portas tipadas injetadas.

```rust
pub(super) type RemoteResult<T> = Result<T, RemoteFailure>;
pub(super) enum RemoteState { Stopped, Starting, Running, Degraded, Stopping, Failed }
pub(super) enum RemotePollMode { Initial, Incremental, Recovery }
pub(super) enum RemoteCoverage { Complete, Partial, Unavailable }
pub(super) enum RemoteError {
    InvalidScope, InvalidState, RemoteUnavailable, PermissionDenied,
    CursorExpired, PageIncomplete, SinkUnavailable, CheckpointUnavailable,
    Cancelled, Internal,
}
pub(super) struct RemoteFailure {
    pub kind: RemoteError,
    pub retryable: bool,
    pub message: String,
}
// File/FileData/RemoteFile/RemoteFileRef/FileEvent/FileScope pertencem a Models.
pub(super) trait RemoteEventSink: Send + Sync {
    // Recebe um File completo por emissão; Ok = enqueue durável no Detector pai.
    fn emit(&self, file: File) -> RemoteResult<()>;
}
pub(super) trait RemoteCheckpoint: Send + Sync {
    fn read(&self, sync_id: SyncId) -> RemoteResult<Option<String>>;
    fn commit(&self, sync_id: SyncId, checkpoint: String) -> RemoteResult<()>;
}
```

A localização do checkpoint pertence a seu backend, não ao Detector Remoto. A garantia **não avançar após emissão não persistida** exige testar os limites por página e replays idempotentes. O sink do pai persiste o objeto `File` integralmente sem reinterpretar atributos nem reconstruir evento reduzido. O Remoto não conhece `EventStore`.

### Porta estável para metadados remotos

O Detector Remoto recebe um **`RemoteFile` já produzido pelo especialista/adaptador do provedor**, sem conhecer os atributos internos ou a estrutura `GoogleFile`. Cada especialista **obtém a API específica por [`providers/`](../../providers/INTERFACES.md)**, utilizando suas funções e tipos sem construir endpoints, clientes HTTP ou protocolos externos próprios; a interface comum exigida é apenas a do `RemoteDetector` e a saída `File`/`RemotePollResult`. O contrato de adaptação é **interno ao especialista** e deve permanecer estável quando `RemoteFile` ganhar ou perder campos.

```rust
// Contrato conceitual; RemoteFile pertence a models.
// O objeto bruto específico do provedor é conhecido apenas no especialista.
pub(super) trait RemoteMetadataAdapter: Send + Sync {
    fn observe(&self, request: RemoteMetadataRequest)
        -> RemoteResult<RemoteMetadataResult>;
}
pub(super) struct RemoteMetadataRequest {
    pub reference: RemoteFileRef, // conta + ID remoto verificado
}
pub(super) struct RemoteMetadataResult {
    pub file: Option<RemoteFile>,
    pub coverage: RemoteCoverage,
}
```

Esse contrato descreve a porta de aquisição/adaptação **sem exigir que a fachada genérica importe GoogleFile**. A implementação especializada pode consultar APIs e delegar metadados mais caros a auxiliares; `models/` continua sem I/O. Caso uma resposta de mudanças não traga ID suficiente, emitir uma reavaliação de escopo, **sem criar RemoteFile fictício**. Um `removed` também pode significar perda de acesso, não exclusão.

**Critério de isolamento:** alterar os campos `RemoteFile` pode alterar adaptador, testes e consumidores interessados, **não** polling, cursor, notificações e fila do Detector Remoto. A interface do Detector pai transporta o mesmo `File` ao Orquestrador por `FileBatch`, sem transformar esta proposta em código já implementado.

## 2. Requests/results

```rust
pub(super) struct RemoteCreateRequest {
    pub sync_id: SyncId,
    pub remote_root_id: String,
    pub session: RemoteSessionPort,
    pub remote_api: RemoteApiPort,
    pub sink: RemoteEventSinkPort,
    pub checkpoints: RemoteCheckpointPort,
    pub polling_interval: Option<Duration>,
}
pub(super) struct RemoteCreateResult { pub detector: RemoteDetector, pub state: RemoteState }
pub(super) struct RemoteStartRequest { pub mode: RemotePollMode }
pub(super) struct RemoteStartResult { pub state: RemoteState, pub scheduled: bool }
pub(super) struct RemotePollRequest {
    pub mode: RemotePollMode,
    pub page_budget: Option<u32>,
}
pub(super) struct RemotePollResult {
    pub coverage: RemoteCoverage,
    pub emitted: u64,
    pub has_more: bool,
    pub checkpoint_confirmed: Option<String>,
}
pub(super) struct RemoteStopRequest {}
pub(super) struct RemoteStopResult { pub state: RemoteState }
pub(super) struct RemoteStatusRequest {}
pub(super) struct RemoteStatusResult {
    pub state: RemoteState,
    pub last_observed_at: Option<Timestamp>,
    pub last_error: Option<RemoteError>,
}
pub(super) struct RemoteApiPort { /* interface tipada do provedor correspondente */ }
pub(super) struct RemoteSessionPort { /* sessão provida pela conta configurada */ }
pub(super) struct RemoteEventSinkPort { /* RemoteEventSink */ }
pub(super) struct RemoteCheckpointPort { /* RemoteCheckpoint */ }
```

As declarações de portas acima são **esboços tipados**, não API compilada; ao implementar, escolher ownership/async apropriados sem expor concretudes ao pai. o especialista deve utilizar a sessão do provedor conforme o contrato de `accounts/` e sua API.

## 3. Métodos internos

```rust
impl RemoteDetector {
    pub(super) fn new(request: RemoteCreateRequest) -> RemoteResult<RemoteCreateResult>;
    pub(super) fn start(&self, request: RemoteStartRequest) -> RemoteResult<RemoteStartResult>;
    pub(super) fn poll_once(&self, request: RemotePollRequest) -> RemoteResult<RemotePollResult>;
    pub(super) fn stop(&self, request: RemoteStopRequest) -> RemoteResult<RemoteStopResult>;
    pub(super) fn status(&self, request: RemoteStatusRequest) -> RemoteResult<RemoteStatusResult>;
}
```

## 4. Contrato por método

### REM-01 — `new`

**Finalidade:** criar detector remoto isolado com raiz, sessão de provedor selecionado e portas.
**Parâmetros:** sync_id/remote_root_id e APIs/sink/checkpoint obrigatórios; intervalo opcional significa padrão interno.
**Retorno:** instância Stopped, sem HTTP.
**Limites e erros:** identidade inválida ou dependência incompatível falha; não inicia OAuth por conta própria.

### REM-02 — `start`

**Finalidade:** iniciar scheduler remoto e programar leitura segura.
**Parâmetros:** modo Initial, Incremental ou Recovery.
**Retorno:** estado e agendamento, não leitura já concluída.
**Limites e erros:** não presume push; checkpoint inválido degrada observação e requer recuperação.

### REM-03 — `poll_once`

**Finalidade:** ler alterações conforme o especialista e **emitir um sinal por chamada de `RemoteEventSink.emit`**, confirmando checkpoint depois da aceitação durável.
**Parâmetros:** modo da leitura e page_budget opcional; ausência de limite não significa página única.
**Retorno:** cobertura, número emitido, continuação e checkpoint efetivamente confirmado (opcional).
**Limites e erros:** falha do sink não avança cursor; sinal de remoção do provedor não confirma exclusão; `has_more` implica cobertura parcial da rodada.

### REM-04 — `stop`

**Finalidade:** interromper polling sem apagar checkpoint.
**Parâmetros:** request vazio, identidade da instância.
**Retorno:** estado parado.
**Limites e erros:** não descarta eventos já recebidos pelo pai nem tenta fazer reconciliação física.

### REM-05 — `status`

**Finalidade:** expor diagnóstico interno para o Detector pai.
**Parâmetros:** consulta vazia.
**Retorno:** estado, último instante e erro opcionais.
**Limites e erros:** não inicia HTTP; erro sanitizado sem token/corpo de resposta.

## 5. Semântica dos opcionais e pendências

- Sem ID remoto confiável, emitir `File` de reavaliação com `data=None` e escopo confiável, sem criar `RemoteFileRef` fictício. Não significa exclusão.
- `page_budget=None`: política interna; `Some(n)` limita esforço sem declarar completude antes da última página.
- `polling_interval=None`: intervalo padrão escolhido pelo Remoto, não imposto ao Local.
- `checkpoint_confirmed=None`: nenhum avanço confirmado.
- `last_observed_at=None`: ainda não houve leitura válida.

**Pendências:** ownership e assincronicidade reais; localização da persistência do checkpoint; cobertura de árvores, papéis de `changes` vs `files.list` e deduplicação de replay. Resolver dentro do módulo sem alterar interface pública do Detector nem obrigar implementar Local/Storage.

## 6. Eventos tipados e encapsulamento em Models

- Um sinal por `RemoteEventSink.emit(File)`; quando há metadados seguros, `File.data=Some(FileData::Remote(remote_file))` preserva o objeto integral.
- `File.event.kind` utiliza `FileEventKind` de Models: Created, Modified, Deleted, Renamed, Moved, MetadataChanged, RecheckRequired ou ScopeUnavailable. Não inventar uma categoria sem evidência.
- Para renomeação/movimento preservar `File.event.previous/current` quando correlacionados com segurança; `FileLocation::Remote` identifica conta/ID, pai e nome quando conhecidos.
- `GoogleChange.removed=true`, perda de acesso, HTTP 403/404, cursor inválido ou paginação incompleta não comprovam exclusão; solicitar reavaliação ou registrar escopo indisponível.
- O adaptador especializado sabe converter `GoogleFile` em `RemoteFile`; o Detector não acessa seus atributos internos nem muda com sua evolução.
- O checkpoint só avança depois da persistência durável do `File` completo; replay preserva identidade da ocorrência. Não realizar operações de sincronização nem exigir `SyncItem`.

## 7. Propriedade exclusiva do checkpoint remoto

**Decisão aprovada:** o Detector Remoto é o proprietário lógico do checkpoint de observação (`RemoteCheckpoint.read/commit`). A porta `RemoteCheckpointPort` já declarada continua inalterada; o backend concreto e sua localização física ainda precisam de validação. O Inventário **não** mantém `remote_cursor`, não avança o checkpoint, não lê o cursor de detecção e não expõe métodos equivalentes.

O especialista somente confirma a posição da página **depois de todos os `File` daquela página terem recebido aceite de persistência durável** pelo sink do Detector, que grava por `EventStore.enqueue`. Em falha parcial ou dúvida sobre persistência, não confirmar checkpoint: recuperar/reler página; replay precisa ser idempotente e preservar a integridade das ocorrências, com o protocolo de deduplicação a definir no Detector. O avanço do cursor significa *observação entregue duravelmente*, **não** análise concluída, baseline confirmado ou sincronização finalizada.

**Fronteira de falhas:** sem transação atômica compartilhada entre storage de eventos e backend de checkpoint, a ordenação `enqueue durável → commit do checkpoint` impede avançar antes de persistir, mas admite replay da mesma página após falha de checkpoint. Não prometer entrega exatamente uma vez nem exclusão de duplicatas sem protocolo testado. Não criar acesso direto do Detector ao SQLite do Inventário.

A confirmação de `FileBatch` pelo Orquestrador acontece pela fachada do Detector, separadamente do checkpoint de leitura remota. O `InventoryManager` mantém somente estados de `SyncItem`, baseline, fila de operações e atividade, após o Analisador interpretar `File`.
