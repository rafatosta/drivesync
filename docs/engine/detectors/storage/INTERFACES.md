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

O Store identifica e registra seu banco no mecanismo técnico comum de [Configuration](../../configuration/INTERFACES.md), fornecendo **localização autorizada e identidade de armazenamento**, sem expor as tabelas, eventos, leases ou cursores. Configuration é responsável pela criação de snapshots consistentes SQLite e pelo procedimento físico de restauração. O Storage não implementa o algoritmo físico de backup/cópia/restauração em paralelo.

Quando corrupção for **comprovada**, o Storage emite `StoreFailure` tipado, marca indisponibilidade e solicita recuperação pela identidade registrada. Antes de qualquer troca, garante fechamento e exclusividade de suas conexões e atividades de escrita. O mecanismo preserva banco e WAL/journal antigos, verifica backup adequado e restaura com segurança; o Storage então reabre e **valida identidade, esquema e invariantes** antes de voltar a aceitar `enqueue`, `claim_batch` ou `acknowledge`.

Se não houver recuperação válida, informar erro e **não** reconstruir banco como resultado implícito de `restore_database`. Uma criação vazia requer autorização explícita da coordenação e inicialização do schema pelo Storage; não equivale a eventos processados ou confirmação de ausência de mudanças. É responsabilidade da coordenação decidir reobservação após perda de histórico.

Falhas transitórias, indisponibilidade, `Conflict`, `CapacityExceeded`, identidade divergente, `SQLITE_BUSY`, disco cheio e incompatibilidade de esquema não autorizam restauração automática. Não chamar diretamente ferramentas de cópia do banco em uso, nem depender de arquivo `.sqlite3` isolado sem tratamento WAL. O resultado técnico e as falhas devem permanecer testáveis com SQLite real isolado.

**Estado:** documentação proposta; API interna de registro e exclusividade será integrada antes da implementação.

## 7. Implementação local compilada

A solicitação de fila persistente autoriza a implementação em `engine/src/detectors/storage/` e a composição pública `PersistentDetector`. O Storage continua privado. A API compilada usa QueueError/QueueFailure/QueueStats, BatchId, FileBatch, EventDisposition e TakeEventsRequest; o contrato conceitual anterior orienta as operações, não impõe uma trait pública ou modelos remotos não revisados.

Cada banco é identificado por AccountId, SyncId e bytes da raiz local. A fábrica gera seu subdiretório por SHA-256 na base privada fornecida; a localização não fica na pasta sincronizada e não pode ficar dentro da raiz monitorada. Diretórios novos usam 0700 e o banco 0600; symlinks, hardlinks do banco, donos incorretos e diretório-base compartilhado são recusados. Conexões são protegidas por mutex; SQLite coordena processos e transações concorrentes. Há apenas um lease ativo por fila, para impedir ultrapassar um lote reservado na ordem de inserção.

O codec local v1 serializa todos os sete campos de File, o FileEvent com motivo e o LocalFile completo, sem projetar seus atributos. Preserva bytes não UTF-8, u64, timestamps assinados e estados MetadataValue. O payload e seu SHA-256 são conferidos na leitura, repetição e confirmação. Cada registro tem limite de 1 MiB; o lote tem até 64 eventos/8 MiB. IDs iguais com dados diferentes são Conflict, inclusive após confirmação. Nenhum RemoteFile fictício foi criado; o codec remoto exige revisão futura.

SQLite usa journal DELETE e synchronous EXTRA, transações IMMEDIATE, foreign keys e quick_check na abertura. Identidade e schema são validados antes de alterar um banco existente; uma falha não recria o arquivo. Reserva e confirmação são transacionais. Leases usam tokens aleatórios e prazo UTC de 1 ms a 1 hora; a ordem usa sequência SQLite, não relógio da observação. Expiração permite reentrega dos mesmos File/IDs. Um salto para trás no relógio pode atrasar a expiração; não elimina os dados.

A política Enabled/Paused também é persistida. Pause impede claims novos de forma transacional; lotes já reservados continuam confirmáveis/liberáveis. Uma fila nova começa pausada; reabertura restaura a política salva. Stats expõe pendentes, reservados, confirmados, total e erro; a apresentação lê cache, enquanto o worker atualiza SQLite fora da thread gráfica. Confirmed é consumo explícito do evento, não sincronização concluída.

Retenção nesta versão: registros completos, inclusive confirmados, são mantidos para idempotência e diagnóstico; 100000 registros e 131072 páginas SQLite limitam a base. Ao atingir capacidade, o enqueue falha sem truncar ou apagar pendências; não há limpeza automática. Os últimos 100 eventos são recuperados para o histórico. A infraestrutura técnica de backup de Configuration ainda não está implementada: não se simula registro nem restauração; corrupção permanece indisponível até recuperação externa e reabertura validada. Isso substitui, nesta entrega, a dependência documental de registrar previamente o banco numa API de Configuration inexistente. Não copiar banco ativo ou reconstruir fila vazia como recuperação.

## 8. Payload e checkpoint remotos

O codec versão 1 admite agora `Remote(RemoteFile)` além de Local, mantendo a codificação dos registros locais existentes. Filas remotas têm identidade de raiz com namespace próprio; não reinterpretam caminhos locais nem misturam contas/origens. Enqueue e leitura validam a conta do payload remoto e o SyncId da fila.

O backend remoto usa uma extensão privada `remote_progress` no mesmo armazenamento privado. O especialista acessa apenas portas de carregar/salvar/emitir. Página preparada contém os File completos, com IDs e horários estáveis; após falha entre enqueue e checkpoint, o replay é idempotente pela identidade/payload. Compare-and-swap impede sobrescrever progresso concorrente. Checkpoint ilegível falha sem descarte ou reconstrução silenciosa. A confirmação de progresso remoto não depende de ack externo nem da política de entrega.


A abertura remota recebe explicitamente o provedor selecionado pela fachada. O prefixo é `remote:<provider>:<root>`, com validação do identificador do provedor para evitar ambiguidades. Para Google, a identidade e o caminho resultantes permanecem iguais aos anteriores. Enqueue/leitura rejeitam payload remoto de outro provedor, mesmo com conta e SyncId iguais.
