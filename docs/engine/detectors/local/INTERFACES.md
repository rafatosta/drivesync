# Detector Local — INTERFACES

## 1. Escopo e visibilidade

Contrato **interno de `detectors/local/`** consumido somente pelo Detector pai. O módulo Local implementa **observação física do filesystem** e produz **`LocalFile` definido por [Models](../../models/INTERFACES.md)**. Models não é uma dependência operacional: apenas structs/enums e invariantes comuns.

As assinaturas abaixo são **esboços contratuais de Rust**, não código compilado. O Local pode ser testado com sink simulado e dependências isoladas.

## 2. Tipos internos e modelo compartilhado

```rust
// Reuse shared model types; do not redefine them here.
use crate::models::{
    AccountId, SyncId, File, FileData, FileEvent, FileEventKind, FileLocation,
    FileScope, ObservationCoverage, LocalFile, LocalRelativePath, Timestamp,
};

pub(super) type LocalDetectorResult<T> = Result<T, LocalDetectorFailure>;

pub(super) enum LocalDetectorState {
    Stopped, Starting, Running, Degraded, Stopping, Failed,
}
pub(super) enum LocalScanCoverage { Complete, Partial, Unavailable }
pub(super) enum LocalStartMode { Initial, Resume }
pub(super) enum LocalScanCause { Startup, WatchEvent, Recovery, Manual }
pub(super) enum LocalScanScope {
    Full,
    Paths(Vec<LocalRelativePath>),
}
pub(super) enum LocalStopReason { UserRequest, Shutdown, Unavailable }
pub(super) enum LocalDetectorError {
    InvalidScope, InvalidState, RootUnavailable, PermissionDenied,
    PartialObservation, WatcherUnavailable, ResourceLimit, Busy, ModelInvalid,
    PreconditionFailed, Cancelled, NotificationFailed, Internal,
}
pub(super) struct LocalDetectorFailure {
    pub kind: LocalDetectorError,
    pub message: String, // Sanitized diagnostic.
    pub retryable: bool,
    pub progress: Option<LocalScanResult>,
}
```

`AccountId`, `SyncId` e `Timestamp` serão importados de seus **proprietários efetivos**, eventualmente reexportados por Models. Sua localização em `crate::models` é ilustrativa até unificação dos tipos existentes. **Não criar definições novas para fazer o exemplo compilar.**

### Aviso individual

```rust
// The detector builds event context; the collector builds LocalFile.
pub(super) trait LocalNotificationSink: Send + Sync {
    // One event per call; success means durable acceptance by the parent.
    fn notify(&self, file: File) -> LocalDetectorResult<()>;
}
```

**Regras de encapsulamento:** o produtor emite **um File por chamada**. `File.data=Some(FileData::Local(local_file))` preserva integralmente o objeto entregue pelo coletor; `File.data=None` é válido para `Deleted` suspeito, `RecheckRequired` e `ScopeUnavailable` segundo os invariantes de Models. O Local constrói somente contexto do evento (`FileEventKind`, localizações anterior/atual, `FileScope`, horário e cobertura), sem inspecionar campos internos de `LocalFile`. Não fabrica identidade ou ação.

`Created/Modified/Renamed/Moved/MetadataChanged` requerem evidências confiáveis e metadados presentes. Para movimento/renomeação, preservar `previous/current` quando correlacionados com segurança; caso contrário, usar `RecheckRequired`. Watcher remove pode ser exclusão suspeita, nunca comando de apagar. Falhas em escopo são indisponibilidade, não exclusão.

### Porta estável de obtenção de LocalFile

O Detector conhece apenas **a solicitação de coleta e o resultado**, sem acessar a composição interna de `LocalFile`. Proposta de porta conceitual, independente da lista de campos do modelo:

```rust
#[derive(Clone)]
pub(super) struct LocalCancellation { /* Shared, read-only cancellation view. */ }

pub(super) struct LocalMetadataRequest {
    pub relative_path: LocalRelativePath,
    pub cancellation: LocalCancellation,
    // The collector is bound to the same root at construction.
}
pub(super) struct LocalMetadataResult {
    pub file: Option<LocalFile>,
    pub coverage: LocalScanCoverage,
}
pub(super) trait LocalMetadataCollector: Send + Sync {
    fn scope(&self) -> LocalDetectorScope;
    fn collect(&self, request: LocalMetadataRequest)
        -> LocalDetectorResult<LocalMetadataResult>;
}
```

**A interface de coleta é estável mesmo que `LocalFile` ganhe ou perca campos.** `file=None` e cobertura/erros representam ausência de metadados confiáveis, **não exclusão comprovada**. A coleta é síncrona no worker de observação, nunca no callback do watcher ou na thread da GUI. O detector fornece cancelamento cooperativo; não criar métodos adicionais por atributo.

O coletor implementa internamente as chamadas de sistema e **pode delegar a funções auxiliares** a aquisição de grupos mais caros ou específicos (ACLs, xattrs, flags, dados de symlink). O código do Detector Local não conhece nem enumera esses grupos e não constrói `LocalFile` por struct literal. Sua única função é solicitar a coleta e transportar **o objeto devolvido sem projetar/reduzir seus campos**.

**Separação:** a definição `LocalFile` pertence a `models/`; a coleta com I/O pertence ao adaptador local; watcher, varredura e avisos pertencem ao Detector Local. Nenhuma modificação de `LocalFile` deve exigir mudança na lógica do Detector. A porta está descrita conceitualmente, sem implementação nesta etapa.

## 3. Objetos internos de requisição e resultado

```rust
pub(super) struct LocalDetectorScope {
    pub sync_id: SyncId,
    pub account_id: AccountId,
    pub local_root: std::path::PathBuf, // Operational scope, not file metadata.
}
pub(super) struct LocalDetectorSettings {
    pub debounce_ms: Option<u64>,
    pub max_pending_paths: u32,
}
pub(super) struct LocalNotificationHandle {
    /* Arc<dyn LocalNotificationSink>, supplied by the parent */
}
pub(super) struct LocalDetectorCreateRequest {
    pub scope: LocalDetectorScope,
    pub notifications: LocalNotificationHandle,
    pub settings: LocalDetectorSettings,
    pub collector: std::sync::Arc<dyn LocalMetadataCollector>,
}
pub(super) struct LocalDetectorCreateResult {
    pub detector: LocalDetector,
    pub state: LocalDetectorState,
}
pub(super) struct LocalStartRequest {
    pub mode: LocalStartMode,
    pub requested_at: Timestamp,
}
pub(super) struct LocalStartResult {
    pub state: LocalDetectorState,
    pub scheduled_scan: bool,
}
pub(super) struct LocalScanRequest {
    pub scope: LocalScanScope,
    pub cause: LocalScanCause,
    pub request_id: Option<String>,
}
pub(super) struct LocalScanResult {
    pub coverage: LocalScanCoverage,
    pub observed_count: u64,
    pub emitted_count: u64,
}
pub(super) struct LocalStopRequest {
    pub reason: LocalStopReason,
    pub requested_at: Timestamp,
}
pub(super) struct LocalStopResult {
    pub state: LocalDetectorState,
    pub in_flight: u32,
}
pub(super) struct LocalStatusRequest {}
pub(super) enum LocalActivity { Inactive, Watching, Scanning, Recovering, Unavailable }
pub(super) struct LocalStatusResult {
    pub state: LocalDetectorState,
    pub activity: LocalActivity,
    pub observed_in_current_round: u64,
    pub emitted_in_current_round: u64,
    pub last_observation_at: Option<Timestamp>,
    pub last_coverage: Option<LocalScanCoverage>,
    pub last_error: Option<LocalDetectorError>,
}
```

**Decisão:** `LocalScanResult` retorna **contagens e cobertura**, não uma segunda coleção de avisos, para evitar duas rotas concorrentes de entrega. Todos os avisos seguem **exclusivamente o sink**; o pai é responsável pela fila. `LocalFile` não é persistido pelo Local.

## 4. Assinaturas internas

```rust
pub(super) struct LocalDetector { /* Private runtime state. */ }
impl LocalDetector {
    pub(super) fn new(request: LocalDetectorCreateRequest)
        -> LocalDetectorResult<LocalDetectorCreateResult>;
    pub(super) fn start(&self, request: LocalStartRequest)
        -> LocalDetectorResult<LocalStartResult>;
    pub(super) fn scan_once(&self, request: LocalScanRequest)
        -> LocalDetectorResult<LocalScanResult>;
    pub(super) fn stop(&self, request: LocalStopRequest)
        -> LocalDetectorResult<LocalStopResult>;
    pub(super) fn status(&self, request: LocalStatusRequest)
        -> LocalDetectorResult<LocalStatusResult>;
}
```

## 5. Contratos por método

### LOC-01 — `new`

**Finalidade:** configurar observador local isolado para `sync_id`, conta e raiz, com sink de saída.
**Parâmetros:** `scope`, `notifications`, `collector` e `settings` obrigatórios; `debounce_ms=None` ou `Some(0)` desativa espera intencional. `max_pending_paths` deve ser positivo.
**Retorno:** instância `Stopped`, sem watcher ativo.
**Erros/limites:** `InvalidScope` para identidade vazia, raiz relativa/reservada ou limite zero; não consultar Accounts para validar associação. Não fabricar `LocalFile`. A validação física ocorre em `start`/`scan_once`.

### LOC-02 — `start`

**Finalidade:** ativar watcher e programar verificação da sua cobertura, tanto em `Initial` quanto em `Resume`. Não reconstruir o histórico do período parado.
**Parâmetros:** `mode`, `requested_at` obrigatórios.
**Retorno:** estado e indicação de scan programado; não comprova varredura concluída.
**Erros/limites:** falha de instalação retorna `WatcherUnavailable`, `RootUnavailable` ou `PermissionDenied`, com estado `Failed` e recursos parciais liberados. Repetição e concorrência seguem a seção 8; nenhuma falha significa exclusão.

### LOC-03 — `scan_once`

**Finalidade:** inspecionar `Full` ou `Paths` e emitir avisos de observação/reavaliação contendo `LocalFile` quando possível.
**Parâmetros:** caminhos relativos tipados, causa e correlação opcional.
**Retorno:** cobertura, quantidade observada e quantidade de avisos aceitos pelo sink.
**Erros/limites:** `Complete` exige inspeção integral do escopo; falha do sink não incrementa `emitted_count` como entregue e exige replay/recuperação; não atualiza baseline nem executa operações.

### LOC-04 — `stop`

**Finalidade:** interromper watcher, agendamento e leitura em andamento.
**Parâmetros:** motivo e instante.
**Retorno:** estado e trabalho em trânsito.
**Erros/limites:** parar não confirma eventos, não apaga filas e não altera modelo de arquivo.

### LOC-05 — `status`

**Finalidade:** obter diagnóstico interno do observador.
**Parâmetros:** request vazio.
**Retorno:** estado, atividade atual (`Watching`, `Scanning`, `Recovering` etc.), contadores da rodada ativa, última observação, cobertura e erro opcionais.
**Erros/limites:** consulta sem scan, I/O remoto ou acesso a banco.

## 6. Semântica de tipos e ausência

- `LocalScanScope::Paths` usa `LocalRelativePath`; nenhum caminho pode escapar da raiz. Lista vazia resulta em `InvalidScope`, sem I/O nem emissão; nunca vira scan global.
- `LocalFile.local_id=None` significa identidade local desconhecida; não usar inode como ID estável sem comprovação.
- `LocalFile.kind=MetadataValue::Known(LocalEntryKind::Unknown)` ou `NotCollected`/`Unavailable` não autoriza inferir tipo ou tamanho; hash derivado não é atributo nativo obrigatório.
- Nomes Unix inválidos em UTF-8 devem ser preservados segundo a representação definida em Models.
- `LocalScanCoverage::Complete` indica **cobertura da leitura**; não equivale a exclusão confirmada.
- `debounce_ms`, `request_id`, últimos timestamps e erros opcionais não implicam comportamento não documentado quando ausentes.

## 7. Fronteiras e pendências

O Local **não importa `SyncItem` ou `SyncItemId`** nesta etapa. Não depende da implementação de Storage; o sink fake é suficiente para testes locais. O Detector pai persiste o `File` completo sem remontar metadados.

A revisão de Models já seleciona o perfil local e define os invariantes de caminho, raiz e ausência. A implementação dos tipos Rust compartilhados continua posterior; a compilação do envelope completo também depende da revisão de RemoteFile, sem autorizar implementação remota nesta demanda. Reutilizar `AccountId` e `Timestamp` existentes; `SyncId` e `FileId` pertencem a Models. A escolha da biblioteca concreta do watcher e das syscalls do coletor é posterior, preservando os contratos deste documento. Detector pai e Storage recebem `File` integral, sem exigir sua implementação nesta etapa.

**Estado:** contrato de projeto não compilado, sem testes executados.

## Integração com controle de entrega

O Local **não possui `pause()` de entrega**: recebe `start/stop` do Detector pai e continua emitindo eventos pelo sink enquanto o pai estiver com entrega ao Orquestrador pausada. O pai persiste `File` antes de aceitar a emissão; pausa de consumo externo não bloqueia a observação local. `stop` interrompe efetivamente o watcher/leituras e preserva o que já foi aceito pelo sink, mas **não** executa Scanner nem reconciliação para compensar o intervalo sem detecção. Cabe ao Orquestrador decidir quando solicitar verificações após retomada. O resultado de `start` indica aceite, não garantia de cobertura completa.


## 8. Decisões operacionais dos cinco métodos

Estas regras complementam as assinaturas da seção 4. São definição documental para a implementação futura, não interfaces já compiladas.

### Execução, estados e concorrência

- Os métodos são síncronos. `new` configura sem percorrer disco; `start` instala a captura e agenda sua verificação; `scan_once` aguarda a rodada; `stop` aguarda encerramento seguro; `status` lê um retrato em memória. Métodos com I/O são chamados fora da thread da GUI.
- Um worker por instância serializa coleta, varreduras e entrega ao sink. Não manter lock do retrato de status ao fazer I/O, chamar dependências ou esperar o worker. O sink não deve reentrar nos métodos bloqueantes da mesma instância.
- `new` devolve `Stopped`, atividade `Inactive`, contadores zero e últimos campos `None`. Não instala watcher, não chama `collect`/sink e não cria arquivo ou banco. Consulta somente `collector.scope()` em memória para verificar a composição.
- `start` em `Stopped` cria uma nova geração. Ao aceitar a captura, devolve `Starting` e `scheduled_scan=true`; a verificação concluída leva a `Running/Watching`. Cobertura incompleta leva a `Degraded`, sem declarar sucesso completo. Falha estrutural leva a `Failed`.
- `start` em `Starting`, `Running` ou `Degraded` é idempotente: retorna o estado atual e `scheduled_scan=false`, sem segunda instalação. Em `Stopping` retorna `InvalidState`; em `Failed`, requer `stop` antes de nova tentativa. `Initial` e `Resume` identificam a intenção do chamador, não autorizam inferência de histórico.
- `scan_once` é permitido em `Stopped`, `Running` e `Degraded`; em `Stopped` não instala watcher e conserva esse estado após uma rodada bem-sucedida. Em `Starting`, `Stopping` e `Failed` retorna `InvalidState`. Uma segunda rodada concorrente retorna `Busy`; pistas do watcher recebidas durante a rodada permanecem pendentes para outra rodada.
- `stop` cancela instalação, leituras, timers e observação, invalida a geração e aguarda chamadas já iniciadas ao coletor/sink retornarem. Sucesso significa `Stopped/Inactive`, `in_flight=0` e nenhuma chamada futura ao sink da geração antiga. Repetições, inclusive concorrentes, aguardam o mesmo encerramento e retornam sucesso. Recursos devem ser liberados também após `Failed`.
- `stop` não interrompe uma syscall de forma insegura nem promete prazo de término para dependência bloqueada. As dependências devem respeitar cancelamento entre operações. Uma emissão já aceita continua aceita; uma rejeitada não se torna entregue por causa da parada.
- `status` pode ser chamado durante qualquer operação. Na rodada ativa, contadores são reiniciados e atualizados progressivamente; fora dela ficam zero, enquanto `last_observation_at`, `last_coverage` e `last_error` preservam o último diagnóstico. Rodada completa bem-sucedida limpa o último erro. `Recovering` significa reparar cobertura da captura, não reconciliar sincronização.

### Escopos e observação física

- A raiz operacional é um caminho absoluto não reservado; sua existência, tipo, acesso e identidade são revalidados antes de observar. Raiz ou ancestral simbólico não deve ser seguido. Substituição de raiz/ancestral durante trabalho invalida a cobertura, não autoriza continuar em outra árvore.
- `Full` percorre os descendentes elegíveis da raiz, sem emitir um `LocalFile` artificial para a própria raiz. Raiz vazia acessível resulta em zero observações e cobertura completa.
- `Paths` valida todos os caminhos antes de I/O: rejeita absoluto, NUL, segmentos vazios, `.` e `..`; preserva bytes não UTF-8. Diretórios incluem seus descendentes; caminhos duplicados ou já abrangidos por outro são visitados uma vez por rodada. Para a raiz, usar `Full`, não caminho vazio.
- `.drivesync` e seus descendentes são reservados e ignorados, inclusive em `Paths`. Uma solicitação só com entradas reservadas resulta em zero observações, sem coletar essas entradas. Outros filtros antigos de backups/temporários não são importados implicitamente.
- Symlinks internos são observados como entradas, com alvo literal no coletor conforme o perfil selecionado em Models; não são seguidos. FIFO, socket e dispositivo são observados por metadados, sem abrir seu conteúdo. Hardlinks não comprovam identidade lógica estável.
- `Complete` exige percurso integral do escopo e coleta dos grupos obrigatórios aprovados. `Partial` preserva os sucessos e incertezas de itens/subárvores; `Unavailable` significa que o escopo solicitado não pôde ser observado. Campo opcional legitimamente não suportado difere de falha de leitura obrigatória. O detector agrega cobertura retornada pelo coletor sem examinar campos de `LocalFile`.
- Entrada que desaparece durante leitura gera reavaliação sem metadados; subárvore inacessível gera aviso de indisponibilidade daquele escopo, e cobertura agregada parcial quando outras entradas foram observadas. Nenhum desses resultados confirma exclusão.
- `observed_count` conta entradas elegíveis cuja tentativa de coleta terminou na rodada, inclusive resultado inconclusivo; não conta entradas reservadas nem enumeração auxiliar. `emitted_count` conta apenas chamadas ao sink que retornaram sucesso nessa rodada, inclusive avisos de escopo. Contagens não precisam coincidir.
- Problemas por item são representados na cobertura e nos avisos, permitindo continuar os demais. Cancelamento, falha de entrega ou impedimento estrutural interrompem a rodada com erro. `LocalDetectorFailure.progress` contém contadores e cobertura parcial da rodada interrompida; é `None` quando nenhuma rodada começou. Nunca declarar `Complete` em uma rodada interrompida.

### Captura, agrupamento e entrega

- O callback do watcher só registra pistas e sinaliza trabalho. Não faz coleta, percurso, banco, hashing, rede ou entrega bloqueante. Callbacks de gerações encerradas são descartados antes de qualquer efeito.
- A captura deve cobrir subdiretórios novos/movidos e verificar a janela entre enumeração e instalação. Overflow, limite do backend ou cobertura incerta causam `Degraded` e reavaliação do escopo afetado. Reparo da captura não chama Scanner, Inventário ou Analisador.
- `debounce_ms=Some(n)` positivo define janela fixa desde a primeira pista pendente, em relógio monotônico; pistas posteriores não adiam seu vencimento. Não é uma promessa de latência quando há trabalho em curso. Só pistas ainda não convertidas em `File` podem ser agrupadas por caminho; eventos já construídos não são fundidos nem alterados.
- `max_pending_paths` limita caminhos de pistas, não a fila persistente do pai. Ao exceder o limite, substituir pistas por uma necessidade de reavaliação de toda a raiz, marcar cobertura degradada e não alegar histórico completo. Também limitar o trabalho em trânsito: não acumular uma segunda fila ilimitada de `File`.
- A primeira observação de um item em scan é `RecheckRequired`, não `Created` por ausência de histórico. Tipos específicos exigem evidência do watcher e coleta atual; renomear/mover exige correlação confiável fornecida pelo backend. Inode ou semelhança de nome não bastam. Em dúvida, reavaliar os caminhos afetados.
- `notify` recebe um `File` por chamada e retorna somente após aceitar ou rejeitar. `Ok` significa aceite durável no pai; erro, inclusive resultado incerto após gravação, não autoriza contar entrega. Testes locais simulam esse contrato e não comprovam durabilidade.
- Em falha, preservar o `File` integral, inclusive ID e horário, para repetição enquanto a instância permanecer ativa; não recoletar para modificar essa mesma ocorrência. Falha retryable deixa o produtor ativo `Degraded` (em scan isolado, permanece `Stopped` com `last_error`); novo `scan_once` tenta primeiro a entrega pendente antes de observar novamente. Não fazer loop de retry sem limite; pistas posteriores permanecem como necessidade de observação. Falha não retryable leva a `Failed`, exigindo parada e diagnóstico.
- Parada/crash não dá durabilidade ao evento ainda não aceito: a pendência é volátil e deve ser informada como erro/estado degradado antes de parar. Reinício não promete replay desses objetos nem recuperação do histórico. A decisão de pedir nova varredura do intervalo interrompido pertence ao Orquestrador.
- `request_id` é correlação da solicitação no diagnóstico interno, não ID da ocorrência nem chave de deduplicação. `None` não gera correlação artificial. `requested_at` é o instante fornecido pelo chamador; `observed_at` é amostrado na observação e preservado no retry. Debounce nunca usa relógio UTC.

## 9. Portas internas de coleta e notificação

As portas das seções 2 e 3 são os únicos contratos de dependência de negócio nesta etapa. O backend concreto do watcher fica privado; sua normalização será testada por pistas controladas e por alterações reais no filesystem, sem expor tipos da biblioteca ao pai.

### `LocalMetadataCollector::scope`

**Assinatura:** `fn scope(&self) -> LocalDetectorScope`.

**Finalidade:** informar a vinculação operacional do coletor para validar a composição.

**Parâmetros:** nenhum; o escopo é fixado na construção do adaptador.

**Retorno:** cópia do escopo (conta, sincronização e raiz absoluta) usado pelo coletor; deve coincidir com o pedido em `new`. A comparação aqui é de configuração, sem canonicalização ou I/O. A revalidação física posterior continua obrigatória.

**Limites e erros:** consulta em memória, sem efeitos externos ou erro. O coletor não pode anunciar um escopo e observar outro. Divergência detectada pelo detector resulta em `PreconditionFailed` antes de chamar `collect`.

### `LocalMetadataCollector::collect`

**Assinatura:** `fn collect(&self, request: LocalMetadataRequest) -> LocalDetectorResult<LocalMetadataResult>`.

**Finalidade:** consultar uma entrada sem seguir symlinks e construir o `LocalFile` aprovado em Models.

**Parâmetros:** caminho validado relativo à raiz vinculada ao coletor; `cancellation` pertence à geração/rodada ativa, é compartilhado por clone e somente o detector pode acioná-lo. `LocalCancellation` é um token interno com consulta `fn is_cancelled(&self) -> bool`, sem I/O. Antes de uma nova operação de leitura, o coletor consulta esse token; verdadeiro interrompe com `Cancelled`.

**Retorno:** objeto integral e cobertura da coleta. `file=None` significa ausência de dados confiáveis, nunca exclusão. O caminho do objeto deve ser o solicitado; essa correspondência é obrigação do adaptador, testada na fronteira, sem projetar campos no detector.

**Limites e erros:** sem mutação de arquivos, hash de conteúdo, rede, persistência ou criação de identidade lógica. A raiz do coletor deve ser a mesma da instância; composição incorreta é `PreconditionFailed`, não motivo para observar outra raiz. Falhas de atributo opcional são representadas pelos estados de Models; cancelamento e impossibilidade estrutural retornam erro tipado. `new` compara `collector.scope()` ao escopo solicitado antes de qualquer I/O; não aceitar um coletor de outra raiz silenciosamente.

### `LocalNotificationSink::notify`

**Assinatura:** `fn notify(&self, file: File) -> LocalDetectorResult<()>`.

**Finalidade:** transferir uma ocorrência completa ao pai pelo único canal de saída.

**Parâmetros:** evento validado, com `sync_id` da instância e identidade de ocorrência preservada; ownership do argumento não dispensa o produtor de manter cópia pendente até confirmação.

**Retorno:** `Ok(())` somente após aceite durável. O fake de teste registra a chamada e controla aceite/rejeição/resultado incerto.

**Limites e erros:** `NotificationFailed` inclui retryability e mensagem sanitizada; falha não altera o evento nem incrementa entrega. Não chama de volta métodos bloqueantes da instância. Aceite não significa consumo pelo Orquestrador nem arquivo sincronizado.

### `LocalCancellation::is_cancelled`

**Assinatura:** `fn is_cancelled(&self) -> bool`.

**Finalidade:** permitir interrupção cooperativa de coleta e percurso.

**Parâmetros:** nenhum; token da rodada/geração recebido por clone.

**Retorno:** inicialmente `false`; depois de cancelado permanece `true` para todos os clones dessa geração. Nova geração recebe outro token.

**Limites e erros:** consulta em memória, segura entre threads, sem efeitos externos e sem erro. Não cancela gerações novas nem desfaz emissão aceita.

## 10. Dependências documentais e limite desta entrega

Este documento define controle, portas e regras de observação. Não cria Rust, watcher, coletor ou testes executáveis nesta etapa. Os atributos de LocalFile e os invariantes locais estão selecionados em [Models](../../models/INTERFACES.md#2-localfile--seleção-para-observação-local). A implementação futura deve seguir essa seleção; o envelope completo ainda depende da revisão remota. Não implementar versões paralelas para contornar dependências.

Models define a raiz por WholeSync sem dados/localizações e rejeita LocalRelativePath vazio. Complete exige o perfil básico local, com btime/flags opcionais; o coletor devolve cobertura sem expor os campos ao detector. NotApplicable no alvo de não-symlink não degrada cobertura. Avisos locais respeitam a tabela de combinações de File na seção 5.5 de Models. Essa dependência não autoriza implementar RemoteFile, Remoto, Storage, Scanner ou fachada pública do Detector. A eventual necessidade de ajustar um contrato vizinho deve ser registrada antes da implementação, sem mudar seu escopo silenciosamente.
