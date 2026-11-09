# Scanner Local — INTERFACES

## 1. Contrato e visibilidade

A interface pública do Scanner contém **construção, varredura local e consulta de estado**. É invocada pelo Orquestrador; todos os acessos ao Inventário utilizam sua API pública. A varredura produz observações locais, nunca comandos de sincronização.

As assinaturas Rust são **contratos propostos para implementação orientada a testes**, não tipos ou métodos já implementados.

```rust
pub type ScannerResult<T> = Result<T, ScannerError>;

impl Scanner {
    pub fn new(config: ScannerConfig) -> ScannerResult<Self>;

    pub async fn scan(
        &self,
        request: ScanRequest,
    ) -> ScannerResult<ScanReport>;

    pub fn status(&self) -> ScannerStatus;
}
```

## 2. Modelos importados

Usar, sem redefinir:

- [Models](../models/INTERFACES.md): `SyncId`, `File`, `FileData::Local(LocalFile)`, `FileEvent`, `FileScope`, `LocalRelativePath`, `Timestamp` e identidades do filesystem quando disponíveis.
- [Inventário](../inventory/INTERFACES.md): `SyncItem`, `SyncItemId`, `PresenceState`, `ObservationProof`, `ObservationSource::LocalScan`, `RecordLocalObservation`, `LocalObservation`, `ObservationTarget`, `InventoryManager`, `InventoryResult` e operações de consulta.
- O Scanner é proprietário somente de suas requisições, relatórios, diagnósticos e tipos de erro.

`File.id` identifica a **observação**; `SyncItemId` identifica o item lógico persistido. `SyncItem.revision` é a revisão otimista do Inventário e não um timestamp, inode ou cursor de varredura.

## 3. Configuração e entrada

```rust
pub struct ScannerConfig {
    pub inventory: InventoryAccess,
    pub root: LocalRoot,
}

pub struct LocalRoot {
    pub sync_id: SyncId,
    pub path: PathBuf,
}

pub struct ScanRequest {
    pub sync_id: SyncId,
}
```

**`inventory`:** dependência já configurada de acesso público ao `InventoryManager`. `InventoryAccess` é um identificador conceitual para o wrapper/trait de injeção a definir no Rust; não significa acesso SQL.

**`root`:** raiz local autorizada e identidade de sincronização. Seu caminho precisa ser validado e protegido de escapes por caminhos relativos, symlinks e corridas de filesystem. É responsabilidade de `new` preparar essa configuração, sem realizar varredura.

**`ScanRequest.sync_id`:** deve corresponder à sincronização associada ao Inventário e à raiz configurada. Uma requisição não percorre múltiplas raízes, múltiplas contas ou o serviço remoto.

A primeira versão não oferece varredura remota nem `ScanScope::Remote`: o Detector Remoto é responsável pelas mudanças no serviço externo.

## 4. Estado durante a execução

```rust
pub enum ScannerState {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

pub struct ScannerStatus {
    pub state: ScannerState,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub files_observed: u64,
    pub directories_observed: u64,
    pub items_recorded: u64,
    pub issues_recorded: u64,
    pub current_directory: Option<LocalRelativePath>,
    pub last_report_status: Option<ScanStatus>,
    pub last_error: Option<ScannerErrorKind>,
}
```

**Semântica:** `status()` devolve uma fotografia interna rápida, sem consulta ao Inventário ou filesystem e sem aguardar `scan()`. A leitura deve ser coerente, com contadores correspondentes à mesma execução. `Idle`: ainda não iniciado; contadores zero e instantes/resultados opcionais ausentes. `Running`: contadores crescem conforme observações aceitas e problemas registrados; `current_directory` identifica opcionalmente a pasta relativa visitada, nunca um caminho absoluto privado. `Completed`: `scan()` retornou `Ok(ScanReport)`, **mesmo quando `ScanReport.status == Partial`**; `last_report_status` distingue cobertura. `Failed`: falha estrutural e `last_error` tipado; `Cancelled`: cancelamento efetivo caso disponível internamente, sem declarar relatório concluído.

Uma nova execução reinicia contadores, instantes de execução e último erro; o estado da execução anterior continua consultável até a próxima. Não inferir porcentagem de progresso sem total de arquivos conhecido. `status()` não entrega lista de `ScanIssue` nem substitui persistência de diagnósticos no Inventário.

## 5. Relatório

```rust
pub struct ScanReport {
    pub sync_id: SyncId,
    pub status: ScanStatus,
    pub files_observed: u64,
    pub directories_observed: u64,
    pub items_recorded: u64,
    pub issues: Vec<ScanIssue>,
}

pub enum ScanStatus {
    Completed,
    Partial,
}

pub struct ScanIssue {
    pub location: ScanIssueLocation,
    pub kind: ScanIssueKind,
    pub impact: ScanIssueImpact,
    pub message: String,
}

pub enum ScanIssueLocation {
    Item(SyncItemId),
    RelativePath(LocalRelativePath),
    ScanRoot,
}

pub enum ScanIssueKind {
    AccessDenied,
    ReadFailed,
    MetadataUnavailable,
    EntryDisappeared,
    EntryChanged,
    UnsupportedEntryType,
    SymlinkProblem,
    ResourceExhausted,
    InventoryRecordFailed,
}

pub enum ScanIssueImpact {
    ItemIncomplete,
    SubtreeUnobserved,
    ObservationNotPersisted,
}
```

### Semântica

| Campo | Significado |
| --- | --- |
| `sync_id` | Identidade da raiz efetivamente examinada |
| `status` | `Completed` se todo escopo exigido foi percorrido e os resultados registráveis foram persistidos; `Partial` se houve lacunas de cobertura ou persistência que permitem terminar o levantamento, mas não afirmá-lo completo |
| `files_observed` | Arquivos regulares cuja entrada foi efetivamente encontrada |
| `directories_observed` | Diretórios efetivamente encontrados, conforme regra de contagem a fixar nos testes, incluindo raiz |
| `items_recorded` | Registros de observação local efetivamente aceitos pelo Inventário |
| `issues` | Diagnósticos encontrados sem confundi-los com operações de sincronização |

`Completed` **pode conter `issues` não vazios**, desde que a cobertura e a persistência do levantamento sejam suficientes. Por exemplo, um arquivo conhecido presente mas com atributo opcional indisponível não impede conclusão. Um diretório não percorrido deve resultar em `Partial` e não pode fundamentar exclusões de seus descendentes. O Scanner pode devolver `Partial` depois de percorrer todo o escopo acessível: não significa que interrompeu prematuramente.

`ScanIssueLocation::RelativePath` é usada quando ainda não existe `SyncItemId` confiável. `message` deve ser sanitizada e não revelar segredos nem conteúdo privado. O relatório é resumo, **não substitui persistência** das condições dos itens.

## 6. Falhas estruturais

```rust
pub struct ScannerError {
    pub kind: ScannerErrorKind,
    pub message: String,
}

pub enum ScannerErrorKind {
    InvalidConfiguration,
    AlreadyRunning,
    WrongSyncScope,
    RootNotFound,
    RootInaccessible,
    StorageUnavailable,
    InventoryUnavailable,
    InventoryUnusable,
    Cancelled,
    Internal,
}
```

O `ScannerResult::Err` se destina a situações que **impossibilitam iniciar, prosseguir ou entregar um resultado de varredura minimamente confiável**. Problemas em arquivos/subpastas isolados devem produzir `ScanIssue` e permitir continuar.

- **`InvalidConfiguration`/`WrongSyncScope`:** raiz/inventário inconsistentes ou caminho fora do escopo.
- **`RootNotFound`/`RootInaccessible`/`StorageUnavailable`:** não é possível observar a raiz autorizada; não declarar recursos ausentes.
- **`InventoryUnavailable`/`InventoryUnusable`:** falha estrutural de acesso ou persistência (por exemplo, banco corrompido, impossibilidade geral de gravação); não tentar recriar/resetar o banco automaticamente.
- **`Cancelled`:** interrupção solicitada da execução; não equivale a conclusão.
- **`Internal`:** falha interna irrecuperável que impede relatório coerente.

Falhas transitórias e localizadas de `record_local`, inclusive revisão divergente, podem virar `ScanIssueKind::InventoryRecordFailed` e `Partial`; nunca declarar registro concluído quando a escrita falhou. Se a persistência geral estiver indisponível, retornar `ScannerError`.

## 7. Contrato funcional de cada método

### `Scanner::new`

**Assinatura:** `pub fn new(config: ScannerConfig) -> ScannerResult<Self>`.

**Parâmetros:** `config.root` identifica o `SyncId` e a raiz local. `config.inventory` representa uma interface de Inventário já vinculada à mesma sincronização.

**Resultado:** instância configurada, sem varredura, alteração de arquivo, detector ou sincronização remota.

**Erros:** configuração malformada, desacordo de sincronização ou dependência indisponível quando verificada durante a construção.

### `Scanner::scan`

**Assinatura:** `pub async fn scan(&self, request: ScanRequest) -> ScannerResult<ScanReport>`.

**Parâmetros:** um `SyncId` correspondente à raiz e ao Inventário da instância.

**Comportamento:**
1. Validar escopo e disponibilidade da raiz e do Inventário.
2. Percorrer recursivamente os diretórios autorizados, ignorando a pasta de estado própria `.drivesync/` e outras exclusões explicitamente configuradas em contrato futuro.
3. Para cada entrada encontrada, coletar atributos reais disponíveis e construir uma observação `File` com `FileData::Local`, identificando `ObservationSource::LocalScan`.
4. Correlacionar com `SyncItem` existente **somente quando a identidade for confiável**; criar novo vínculo de item quando comprovadamente novo; em ambiguidade, registrar diagnóstico e não substituir vínculo.
5. Usar `InventoryManager::record_local` e demais consultas públicas, aplicando revisão esperada e `ObservationProof`; não executar SQL.
6. Registrar lacunas por arquivo ou subárvore, preservar observações anteriores onde não houver nova prova e continuar a varredura.
7. Após percorrer um escopo completamente observado, identificar entradas anteriores não encontradas **naquele escopo** e registrar `AbsentConfirmed` apenas com prova suficiente. Nunca usar falta de leitura de subpasta ou arquivo como prova de exclusão.
8. Devolver relatório mesmo que haja questões individuais; não reconciliar estados, consumir filas ou executar comandos.

**Resultado:** `ScanReport` com `Completed` ou `Partial`. O Scanner não devolve a lista de operações de upload/download, nem entrega `SyncItem` diretamente ao Analisador.

**Concorrência:** existe no máximo um `scan()` em andamento por instância. Uma nova chamada durante `Running` retorna `AlreadyRunning` sem perturbar a execução em andamento. O estado `Running` é definido no início efetivo, e o estado terminal é publicado antes do retorno.

**Erros:** somente as falhas estruturais tipadas anteriormente. Quando um item apresentar problema de leitura ou metadados, o resultado correto é diagnóstico e continuação, não `Err` geral.

### `Scanner::status`

**Assinatura:** `pub fn status(&self) -> ScannerStatus`.

**Finalidade:** expor a fotografia da execução atual ou última execução, com estado, contagens, instante de início/fim e último erro estrutural. É uma **consulta síncrona, rápida e sem I/O**, apropriada a acompanhamento por um consumidor externo.

**Retorno:** um `ScannerStatus` coerente. Não inicia ou cancela trabalhos, não consulta arquivos, não acessa o banco nem notifica o dispatcher. A exposição da informação à GUI é responsabilidade de outro componente.

## 8. Persistência de observações e diagnósticos

O Inventário é o proprietário de `SyncItem`, presenças, revisões e baseline. O Scanner registra **exclusivamente observações locais**. Os campos `remote`, `remote_presence`, `baseline`, `last_synced_at` e dados de operações confirmadas não são modificados pelo Scanner.

Quando uma leitura falha, distinguir:
- **Presença comprovada, informação incompleta:** preservar `Present` com atributos disponíveis e registrar diagnóstico.
- **Presença/identidade incerta:** manter `Unknown` ou o último snapshot válido como informação histórica, sem afirmar `AbsentConfirmed`.
- **Subárvore sem cobertura:** não inferir ausência de seus descendentes.

**Lacuna atual de integração:** a API documentada do Inventário possui `record_local`, `ObservationProof` e `PresenceState`, mas ainda **não define método/campo específico para persistir `ScanIssue` por item nem um marcador de geração/escopo da varredura**. A implementação precisa acordar essa representação no contrato do Inventário antes de afirmar que o Analisador consegue recuperar todos os diagnósticos após reinício. Não armazenar as questões apenas em memória e declarar recuperação durável.

O Detector Local mantém sua fila própria e não escreve no Inventário através do Scanner. O Orquestrador inicia/aguarda o Scanner e, após conclusão, volta a consumir os eventos persistidos pelos Detectores. Não cabe ao Scanner ordenar ou reconciliar essas ocorrências.

## 9. Concorrência e segurança

- O Scanner é uma observação, não uma fotografia transacional do filesystem: arquivos podem surgir, mudar ou desaparecer enquanto ele percorre a árvore.
- Não interromper o levantamento por mudanças normais. Registrar o estado efetivamente observado ou sua incerteza; o Analisador/Reconciliador trata estados e eventos posteriores.
- Nunca transformar uma observação antiga que chega por último em prova de presença atual definitiva. A revisão otimista do Inventário deve impedir sobrescritas indevidas; os detalhes de integração temporal serão validados quando os contratos forem reunidos.
- Respeitar raiz e proteção de symlinks sem seguir caminhos fora do escopo; não atravessar montagens/pastas não autorizadas sem política explícita.
- Não ler conteúdo integral ou calcular hash obrigatório na varredura por padrão: metadados e presença são suficientes para o levantamento inicial. Ausência de hash não prova igualdade de conteúdo.
- Não executar remoções, atualizações físicas, uploads, downloads, reparaços ou decisões automáticas de conflito.

**Estado:** contrato documental. A persistência de diagnósticos por item e a semântica de geração/cobertura exigem alinhamento posterior com o Inventário.
