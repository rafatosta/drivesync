# Detector Storage — INTERFACES

## 1. Fronteira interna e tipos

API interna de persistência de eventos, substituível por implementação simulada nos testes. O contrato encapsula as operações de fila e reserva de eventos. Os nomes abaixo representam contratos de projeto, ainda não compilados.

```rust
pub(super) type StoreResult<T> = Result<T, StoreFailure>;

pub(super) struct StoreIdentity { pub sync_id: SyncId }
// File é o objeto completo de models/. Não existe File paralelo.
pub(super) enum DeliveryDisposition { Processed, Discarded }
pub(super) enum StoreError {
    IdentityMismatch, Unavailable, Corrupt, SchemaUnsupported,
    Conflict, LeaseExpired, InvalidLease, InvalidRequest, CapacityExceeded, Internal,
}
pub(super) struct StoreFailure {
    pub kind: StoreError,
    pub retryable: bool,
    pub message: String,
}
pub(super) struct StoreStats {
    pub pending: u64,
    pub leased: u64,
    pub total_confirmed: u64,
}
```

`File`, `FileId` e `SyncId` vêm de Models ou proprietário único reexportado. O Storage persiste `File` integral, sem `SyncItemId` ou `ScopeHint` paralelo.

## 2. Requests e resultados

```rust
pub(super) struct StoreOpenRequest {
    pub identity: StoreIdentity,
    pub location: StoreLocation, // fornecida pela fábrica privada, não pelo Detector público
}
pub(super) struct StoreOpenResult { pub store: EventStoreHandle }
pub(super) struct EnqueueEventRequest { pub file: File }
pub(super) struct EnqueueEventResult { pub file_id: FileId, pub inserted: bool }
pub(super) struct ClaimBatchRequest {
    pub max_events: u32,        // > 0
    pub lease_duration: Duration,
}
pub(super) struct ClaimBatchResult {
    pub lease_id: Option<LeaseId>, // None se não há eventos
    pub files: Vec<File>,
    pub first_observed_at: Option<Timestamp>,
    pub last_observed_at: Option<Timestamp>,
}
pub(super) struct AcknowledgeRequest {
    pub lease_id: LeaseId,
    pub disposition: DeliveryDisposition,
}
pub(super) struct AcknowledgeResult { pub confirmed_count: u32 }
pub(super) struct ReleaseBatchRequest { pub lease_id: LeaseId }
pub(super) struct ReleaseBatchResult { pub released_count: u32 }
pub(super) struct StoreStatusRequest {}
pub(super) struct StoreStatusResult { pub stats: StoreStats }

pub(super) trait EventStore: Send + Sync {
    fn enqueue(&self, request: EnqueueEventRequest) -> StoreResult<EnqueueEventResult>;
    fn claim_batch(&self, request: ClaimBatchRequest) -> StoreResult<ClaimBatchResult>;
    fn acknowledge(&self, request: AcknowledgeRequest) -> StoreResult<AcknowledgeResult>;
    fn release_batch(&self, request: ReleaseBatchRequest) -> StoreResult<ReleaseBatchResult>;
    fn status(&self, request: StoreStatusRequest) -> StoreResult<StoreStatusResult>;
}
pub(super) fn open(request: StoreOpenRequest) -> StoreResult<StoreOpenResult>;
```

`StoreLocation`, `EventStoreHandle`, IDs e relógio de leases serão definidos com ownership concreto na implementação. Os detalhes de armazenamento permanecem encapsulados na interface definida aqui.

## 3. Contrato funcional de cada método

### STO-01 — `open`

**Finalidade:** abrir armazenamento da sincronização, validando identidade e integridade.
**Parâmetros:** `identity.sync_id` obrigatório; localização interna não vazia e segura.
**Retorno:** handle encapsulado para a fila dessa sincronização.
**Limites e erros:** formato inválido/corrupção/identidade divergente falham; não substituir a base de dados existente.

### STO-02 — `enqueue`

**Finalidade:** gravar evento único e durável.
**Parâmetros:** `File` completo obrigatório, com ID, sincronização, evento e localizações, cobertura, escopo e metadados disponíveis.
**Retorno:** ID e `inserted` (false somente para repetição comprovadamente idêntica pelo mesmo ID).
**Limites e erros:** IDs repetidos com conteúdo divergente falham; não coalescer alterações diferentes nem gerar operação de sincronização.

### STO-03 — `claim_batch`

**Finalidade:** reservar atomicamente até `max_events` eventos pendentes para entrega.
**Parâmetros:** limite positivo e prazo de lease válido; nenhum filtro de tecnologia Local/Remote.
**Retorno:** lote tipado em ordem determinística, `lease_id` opcional para fila vazia e limites temporais opcionais.
**Limites e erros:** sem lease para fila vazia; lease válido impede claim concorrente; não exclui eventos por leitura.

### STO-04 — `acknowledge`

**Finalidade:** confirmar que consumidor tratou ou descartou **explicitamente** o lote reservado.
**Parâmetros:** lease válido e disposição `Processed` ou `Discarded` obrigatórios.
**Retorno:** quantidade confirmada, com proteção de idempotência para repetição da mesma confirmação.
**Limites e erros:** lease errado/expirado não apaga registros; não interpretar confirmação como baseline sincronizado.

### STO-05 — `release_batch`

**Finalidade:** devolver lote reservado sem confirmação para consumo futuro.
**Parâmetros:** lease válido.
**Retorno:** quantidade liberada; chamada repetida não deve causar perda.
**Limites e erros:** não altera payload nem converte evento em descartado.

### STO-06 — `status`

**Finalidade:** consultar estado da fila sem reservar eventos.
**Parâmetros:** request vazio; identidade já pertence ao handle.
**Retorno:** contagens pendentes/reservadas/confirmadas.
**Limites e erros:** consulta sem efeitos colaterais de consumo; pode retornar erro de indisponibilidade.

## 4. Erros, estados e ausência

`File.data=None` não prova exclusão; escopo e evento permanecem no objeto completo. `lease_id=None` só é válido quando nenhum evento foi reservado. `ClaimBatchResult.files=[]` é fila vazia, não prova de que nenhuma mudança ocorreu durante a aplicação parada.

**Regra de contrato:** a confirmação é feita por lote sob `LeaseId` e todos os estados são transacionais. Lease expirado deve tornar evento elegível novamente, sem perder sua ordem original. Recuperação física (SQLite, journal, WAL, índice, FKs) é privada.

## 5. Pendências concretas

Definir serialização fiel e versionada do File completo, ownership/traits concretos, política de retenção e lease expirado, mecanismo de tempo para testes, instalação da base quando pasta sincronizada não estiver acessível e proteção contra processos simultâneos. Resolver antes de implementar casos dependentes; manter os detalhes internos encapsulados nas operações documentadas.

## 6. Fidelidade e integridade do File

- `enqueue` persiste o objeto `models::File` integral, incluindo `File.event.previous/current`, `File.data: Option<FileData>`, metadados de `LocalFile`/`RemoteFile` e envelope `ProviderFileData` quando presentes. Não reduzir a IDs ou reconstruir por nome/caminho.
- `File.id` é chave da ocorrência; reentrega deve preservar ID e payload. Duplicata idêntica é idempotente; mesmo ID com conteúdo divergente falha sem substituir.
- `claim_batch` devolve os mesmos `File` em `files: Vec<File>`; Detector monta `FileBatch` com o identificador/lease de entrega. Leitura não confirma.
- Evolução de campos nos modelos pode exigir revisão do codec, mas não da lógica de fila, lease ou confirmação. Storage trata File como valor opaco.
- Dados sensíveis (ACL, xattrs, usuários, labels, detalhes de segurança) exigem política aprovada de retenção, sanitização e limite. Se não puder persistir com fidelidade, recusar o enqueue com falha tipada; nunca truncar silenciosamente após aceite.
- Os registros persistidos representam ocorrências detectadas e seu estado de entrega.

## Proteção do banco de eventos e recuperação

Quando o Store comprovar corrupção de seu armazenamento, deve emitir `StoreFailure` tipado e **mensagem de erro ao usuário**, solicitar a [Configurations](../../configuration/README.md) a política de recuperação automática e executar localmente a restauração de backup validado. Configurations não manipula fila, lease, cursor ou SQL de terceiros.

Se a restauração falhar, **informar novamente o usuário** e reconstruir um Store vazio, preservando a base danificada. Isso **não equivale a eventos processados**: observações perdidas devem ser refeitas pela fonte local/remota; cursor ausente ou incoerente exige reobservação segura (por exemplo, listagem remota completa quando o checkpoint incremental não for utilizável). Não liberar uma fila recém-criada como prova de ausência de alterações.

Não confundir corrupção com `Unavailable`, `Conflict`, `CapacityExceeded`, identidade divergente, `SQLITE_BUSY` ou esquema incompatível: esses erros não devem provocar reconstrução automática. Se a criação do novo Store falhar, permanecer em erro sem confirmar entrega fictícia. O contrato público do Detector permanece inalterado; a recuperação precisa ser sinalizada à coordenação de inicialização.

Backups consistentes, WAL, validação de identidade, preservação dos arquivos danificados e rotação são executados pelo Storage sob política de Configurations. Especificação documental, não código implementado.
