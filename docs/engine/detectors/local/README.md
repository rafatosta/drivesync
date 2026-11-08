# Detector Local — observação baseada em Models

## Objetivo e autonomia

O **Detector Local** é o produtor de observações do filesystem. Ele solicita a um coletor especializado a construção de **`LocalFile` do domínio compartilhado** quando há atributos observáveis de um arquivo/diretório, e emite **um aviso individual por chamada** ao sink interno fornecido pelo módulo pai [Detector](../README.md).

Sua implementação, seu watcher, suas varreduras e seus testes são **independentes** do Detector Remoto, de Storage, do Inventário e dos operadores. O Detector pai é o único módulo público de detecção para o Orquestrador. O Local depende do **contrato de dados** de [Models](../../models/INTERFACES.md), não de um serviço `ModelsManager` (inexistente).

## Responsabilidade de cada componente

| Componente | Faz | Não faz |
| --- | --- | --- |
| `models/` | Define `LocalFile`, `LocalEntryKind`, `LocalRelativePath` e identidades compartilhadas | Não lê disco, não observa mudanças |
| `detectors/local/` | Coordena observação, solicita coleta por porta estável, emite sinais tipados e avalia cobertura | Não modifica baseline, não conhece SQL nem implementações dos demais filhos |
| `detectors/` | Recebe File integral de Local/Remoto e o encaminha ao Storage sem projeção | Não executa observação física local |
| `detectors/storage/` | Persiste fila de notificações, leases e confirmações | Não observa filesystem |
| `inventory/` | Persiste estado e associa observações a `SyncItem` segundo seu contrato | Não é dependência de gravação do Detector Local |
| `analyzer/` | Analisa divergências e conflitos | Não é chamado diretamente pelo Local |

**`SyncItem` não faz parte desta revisão.** Sua eventual transferência para Models ocorrerá somente ao revisar o planejamento do Inventário. O Local **não cria nem exige `SyncItem`**, nem inventa `SyncItemId`.

## Coleta isolada da lógica do Detector

O Detector Local **não monta `LocalFile` atributo por atributo**. Sua responsabilidade é detectar a necessidade de observação, selecionar o escopo, chamar um **coletor de metadados locais** por interface estável e encaminhar o `LocalFile` devolvido de forma opaca.

O coletor é um componente especializado de aquisição/adaptação de metadados do filesystem, separado do watcher, da varredura e do pipeline de notificações. Rotinas auxiliares podem obter grupos adicionais (`statx`, atributos estendidos, ACLs, links, flags etc.) sem introduzir dependências desses campos no Detector.

**Critério obrigatório:** incluir, remover ou reorganizar campos de `models::LocalFile` **não exige qualquer alteração na lógica do Detector Local**. É permitido ajustar o coletor, os testes do coletor e consumidores interessados no campo. O Detector permanece desacoplado do esquema de metadados.

Isso não altera a distinção entre `LocalFile` observado, aviso sem arquivo e cobertura parcial, nem transfere I/O para `models/`. A coleta completa por categoria deve seguir o contrato e a futura revisão de necessidade dos metadados.

## Fluxo

```mermaid
flowchart LR
    FS[Filesystem] --> LOC[Detector Local]
    MOD[Models: LocalFile] -.-> LOC
    LOC -->|"File individual: FileData::Local opcional"| DET[Detector pai]
    DET -->|"enqueue"| STO[Storage do Detector]
    DET -->|"FileBatch: Vec<File>"| ORQ[Orquestrador]
    ORQ --> AN[Analisador]
    AN --> INV[Inventário]
```

1. Validar `sync_id` e raiz local fornecida pela composição do Detector.
2. Iniciar watcher e varredura, preservando evidência de observação durante a instalação do watcher.
3. Solicitar ao coletor o `LocalFile` com **somente atributos realmente observados**; o Detector não monta seus campos.
4. Para cada ocorrência, emitir um `File` contendo `FileData::Local(LocalFile)` **quando disponível** e um escopo relativo para reavaliação quando não houver arquivo conhecido.
5. O sink do Detector pai confirma sucesso apenas após persistência durável em Storage; se falhar, o Local não declara observação entregue e mantém necessidade de replay/revarredura.
6. Após reinício, reobservar o escopo local: o watcher não reconstrói o histórico de alterações feitas enquanto a aplicação esteve parada.

**O arquivo não é obrigatório em todo aviso.** Remoção, overflow, raiz indisponível e leitura incompleta podem produzir um aviso de reavaliação **sem `LocalFile`**. Isso não equivale a um `LocalFile` ausente comprovadamente.

## Cobertura e semântica

- **Complete:** escopo solicitado foi integralmente inspecionado com evidência suficiente; não significa que o Local confirmou exclusão de um `SyncItem`.
- **Partial:** apenas parte foi observada; não limpar atributos nem inferir remoção no restante.
- **Unavailable:** raiz/escopo inacessível; nunca equivale a exclusão.
- **Unknown:** `MetadataValue::NotCollected`/`Unavailable`, tipo de entrada desconhecido ou identidade local opcional não devem ser substituídos por palpites.

O Local observa eventos de criação, mudança, renomeação e movimento, mas essas pistas podem ser ambíguas. Não converter automaticamente um evento de remoção em exclusão confirmada. Caminho e inode isolados não comprovam identidade estável.

## Segurança do filesystem

- `.drivesync/` e seus descendentes não são conteúdo de usuário sincronizável.
- Nomes Unix não UTF-8 devem ser preservados sem conversão com perda, conforme `LocalRelativePath` de Models.
- O `PathBuf` da **raiz operacional** pertence à configuração interna do detector; o `LocalFile.relative_path` é **relativo à raiz** e não carrega caminho absoluto externo.
- Symlinks não são seguidos para fora da raiz; tipo `Symlink` não implica permissão de sincronização.
- Arquivo ainda em escrita não deve ser interpretado como conteúdo final pronto para upload.
- Overflow, erros de permissão, desmontagem e mudanças concorrentes exigem cobertura parcial/indisponível e nova observação.
- Não varrer pastas fora da raiz, nem ler conteúdo para calcular hash indiscriminadamente.
- Dois escopos de sincronização não misturam identidades nem avisos.

## Dependências permitidas

**Permitida:** [Models](../../models/INTERFACES.md), filesystem e biblioteca watcher internos, clock/scheduler e sink de aviso tipado injetados. Tipos de identidade devem ser **reutilizados** de seus proprietários, sem novas definições incompatíveis.

**Proibida:** `GoogleFile`, Google API, Detector Remoto concreto, Storage concreto, Inventário/SQLite, Analisador, Orquestrador e Upload/Download. A porta de aviso pertence ao próprio contrato interno Local, e pode ser implementada por fake em testes.

## Invariantes

1. Toda observação de metadados disponível usa `models::LocalFile`, e não uma struct paralela própria de arquivo.
2. Cada `LocalNotificationSink.notify` recebe **um** `File`; não há dois canais públicos para alterações locais.
3. `File.data=None` significa que o aviso não carrega metadados seguros; **não** significa exclusão.
4. Sem acesso ou modificação de baseline, fila de operações, histórico, banco ou serviço remoto.
5. A falha de entrega não deve ser tratada como sucesso; eventos podem ser reemitidos.
6. `stop` encerra observação; `start` recupera mudanças por nova inspeção conforme cobertura.
7. Contratos próprios e testes reais do Local não exigem implementações reais dos irmãos.

[INTERFACES.md](INTERFACES.md) · [TESTS.md](TESTS.md) · [AGENTS.md](AGENTS.md) · [Models](../../models/README.md).

**Estado:** arquitetura-alvo documentada. Não representa código implementado nem testes executados.

## Eventos e encapsulamento conforme Models

Cada emissão contém **um único `models::File`**, não `LocalNotice` ou struct paralela. O contexto é `FileEvent {kind, previous, current}`, `FileScope::Local`, `observed_at`, `coverage`, `sync_id` e `File.id`. O payload é `Some(FileData::Local(LocalFile))` quando há observação segura; `None` pode representar remoção suspeita ou reavaliação.

O Detector Local só solicita `LocalFile` ao coletor especializado e o incorpora integralmente, sem acessar suas propriedades. Classificação `Created/Modified/Deleted/Renamed/Moved/MetadataChanged` requer evidência; renomeação/movimento correlacionados carregam localizações anterior/atual. Falhas e ambiguidade geram `RecheckRequired` ou `ScopeUnavailable`, **nunca exclusão confirmada**.


## Visão visual detalhada do serviço Local

Os diagramas reproduzem [Local — INTERFACES](INTERFACES.md) e o envelope de [Models](../../models/INTERFACES.md). O **watcher** e a **varredura** são mecanismos internos mencionados no contrato, não novas APIs. O coletor é a porta explicitamente declarada `LocalMetadataCollector`.

### Componentes e limites

```mermaid
flowchart TD
    FS["Filesystem dentro da raiz sincronizada"]
    W["Watcher: sinais de mudança"]
    SC["Varredura: inicial, manual ou recuperação"]
    D["LocalDetector: new, start, scan_once, stop, status"]
    C["LocalMetadataCollector.collect(LocalMetadataRequest)"]
    LF["LocalMetadataResult: Option LocalFile + coverage"]
    EV["File: FileEvent + FileScope + FileData::Local opcional"]
    SK["LocalNotificationSink.notify(File)"]
    P["Detector pai: enqueue durável"]
    FS --> W
    FS --> SC
    W --> D
    SC --> D
    D -->|"solicita metadados"| C
    C -->|"leitura local"| FS
    C --> LF
    LF -->|"valor opaco"| D
    D -->|"observação ou reavaliação"| EV
    EV --> SK
    SK --> P
```

O Detector Local **não** monta atributos de `LocalFile`: apenas solicita `collect`, preserva o resultado integral e acrescenta contexto de observação ao `File`. Coleta especializada (por exemplo, ACLs, xattrs e `statx`) pode ser feita internamente pelo coletor sem alterar watcher, scanner ou sink.

### Sequência de observação, emissão e falha

```mermaid
sequenceDiagram
    participant F as Filesystem
    participant L as LocalDetector
    participant C as LocalMetadataCollector
    participant S as LocalNotificationSink
    participant D as Detector pai / Storage
    F-->>L: sinal de watcher ou varredura
    L->>C: collect(LocalMetadataRequest)
    C-->>L: LocalMetadataResult { file, coverage }
    alt metadados observáveis
        L->>S: notify(File com FileData::Local)
    else ausência de leitura confiável
        L->>S: notify(File de reavaliação, data=None)
    end
    S->>D: persistir File integralmente
    alt persistência confirmada
        D-->>S: aceite durável
        S-->>L: Ok
    else falha ou backpressure
        D-->>S: erro
        S-->>L: erro; requer replay/revarredura
    end
```

### Encapsulamento do aviso local

```text
LocalNotificationSink.notify(File)
└── File                          models/
    ├── id / sync_id / observed_at / coverage / scope
    ├── event: FileEvent
    │   ├── kind: FileEventKind
    │   ├── previous: Option<FileLocation>
    │   └── current: Option<FileLocation>
    └── data: Option<FileData>
        ├── Some(Local(LocalFile)) [quando observado]
        └── None                   [quando sem dados confiáveis]
```

`RecheckRequired` e `ScopeUnavailable` não comprovam exclusão. Identidade local, relações anterior/atual e cobertura precisam de evidência, conforme Models. Estes desenhos **não** descrevem código compilado.
