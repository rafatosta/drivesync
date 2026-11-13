# Models — TESTS: fluxo local selecionado e catálogo remoto

## Estado

O fluxo local tem seleção documental definida em [INTERFACES.md](INTERFACES.md), seções 1/2/5. Os campos remotos continuam catálogo sem seleção e não bloqueiam a revisão dos tipos locais, mas bloqueiam a implementação integral do envelope compartilhado. Os testes locais implementados e sua evidência estão na seção 8; os cenários de envelope e remoto continuam especificação futura. Não inferir aprovação remota a partir da seleção local.

Os testes de `models/` são **puros**: sem rede, filesystem real, SQLite, serviços Google, InventoryManager, Orquestrador ou operação de sincronização.

## 1. Identidades e tipos auxiliares

Casos de FileHash/FieldValue/FileKind e referências remotas permanecem catálogo futuro; não são requisitos de implementação do LocalFile selecionado. Identidades locais, caminhos e Timestamp existente pertencem ao fluxo desta revisão.

| Caso | Atributos/objeto | Sucesso esperado | Rejeição/invariante |
| --- | --- | --- | --- |
| MOD-ID-01 | `RemoteFileRef { account_id, file_id }` | Dois arquivos com mesmo ID bruto em contas diferentes são distintos | ID remoto sozinho não identifica recurso global |
| MOD-ID-02 | `File.id: FileId` | Mesmo evento reentregue mantém seu ID; nova ocorrência recebe novo ID | Retry não gera nova identidade |
| MOD-ID-03 | `AccountId`, `SyncId`, `LocalFileId` | Um único tipo proprietário, reutilizado/reexportado | Nenhum tipo homônimo incompatível |
| MOD-ID-04 | `Timestamp.unix_millis` | Preserva valor em UTC e precisão expressa em ms | Não converter `None` de mtime em relógio atual |
| MOD-ID-05 | `FileHash` | Retém algoritmo + digest hexadecimal válido | Não declarar equivalência entre algoritmos distintos |
| MOD-ID-06 | `LocalRelativePath` | Entrada abaixo da raiz; nomes Unix não UTF-8 preservados byte a byte | Rejeitar vazio, NUL, absoluto, barra final, separador repetido, `.` e `..`; raiz usa WholeSync |
| MOD-ID-07 | `FieldValue<T>` | `Unknown`, `Known([])` e `Known([id])` mantêm significados distintos | Não transformar `Unknown` em `Known([])` |
| MOD-ID-08 | `FileKind` | Cada variante mantém significado, incluindo `Unknown` | Não adivinhar `Regular` pelo nome/extensão |

## 2. LocalFile — cenários do contrato selecionado

| Caso | Campo/grupo | Critério verificável |
| --- | --- | --- |
| MOD-LOC-01 | local_id/relative_path | None não gera ID; ID presente inválido é rejeitado; path permanece byte-exato |
| MOD-LOC-02 | kind | Preservar cada variante; Known(Unknown) não vira Regular nem suporta Complete |
| MOD-LOC-03 | mode | raw_mode e permission_bits coerentes; divergência com tipo concreto é rejeitada |
| MOD-LOC-04 | owner | UID/GID inclusive zero, sem resolução de nomes ou inferência de autorização |
| MOD-LOC-05 | times | mtime/ctime/btime conhecidos ou com estado explícito; nanos inválidos rejeitados; segundos negativos preservados, sem impor ordem entre datas |
| MOD-LOC-06 | storage | size/nlink/inode/device independentes, inclusive zero; device/inode não são LocalFileId |
| MOD-LOC-08 | flags | Par valor/máscara preservado; Known sem máscara Known é inválido; estados não Known devem coincidir; bits fora da máscara não ganham semântica inferida |
| MOD-LOC-13 | symlink.target | Symlink conhecido aceita bytes literais inclusive absoluto/.., sem converter a LocalRelativePath; não-symlink conhecido exige NotApplicable; kind desconhecido não admite NotApplicable |
| MOD-LOC-15 | MetadataValue | Distinguir Known(0)/Known([]), NotCollected, NotApplicable, Unsupported, PermissionDenied e Unavailable; não trocar um pelo outro |
| MOD-LOC-16 | modelo puro | Construção/validação só com valores sintéticos; nenhuma dependência de arquivo, credencial, banco, detector ou provedor |
| MOD-LOC-17 | cobertura | LocalFile parcial coerente passa validate; File Complete com básico incompleto falha. Opcionais btime/flags indisponíveis não impedem Complete do básico |
| MOD-LOC-19 | limite do esquema | Revisão estrutural garante ausência de hash, atime, conteúdo, ACL, xattrs e blobs abertos; não confundir revisão do esquema com teste de coleta física |

Os antigos casos MOD-LOC-07, 09, 10, 11, 12, 14 e 18 cobriam categorias do catálogo abrangente agora excluídas do envelope. Não exigir execução de consultas de montagem, Direct I/O, AtomicWrite, xattrs, ACL ou blobs para concluir o modelo local. Se forem especificadas sob demanda no futuro, seus testes pertencem à consulta correspondente. Nenhum dado remoto é selecionado por essa retirada.

A suíte Models usa dados sintéticos. A suíte do **coletor local** valida as fontes reais e as máscaras do sistema, inclusive readlink e fallback de statx; não exigir xattrs/ACL só para provar o perfil básico. Coverage é calculada pelo coletor e pelo percurso, com invariantes estruturais verificadas por Models.

## 3. RemoteFile — identidade, metadados e vínculo com provedor

| Caso | Atributo/contrato | Teste conceitual |
| --- | --- | --- |
| MOD-REM-01 | `reference` | ID é associado à conta; mesmo ID bruto em contas distintas não colide |
| MOD-REM-02 | `provider` | Serviço coincide com provedor da conta e do envelope |
| MOD-REM-03 | `identity` | Nome, descrição e nome original conhecidos ou individualmente não coletados |
| MOD-REM-04 | `classification` | Tipos regulares, pasta, nativo e atalhos não são confundidos |
| MOD-REM-05 | `location.parent_ids` | `Known([])` é diferente de `NotCollected`; IDs pais não viram caminhos inventados |
| MOD-REM-06 | `location.container_id` | Suporte a drive compartilhado sem presumir que todo provedor usa driveId |
| MOD-REM-07 | `times` | Datas remotas preservadas sem substituição por instante atual |
| MOD-REM-08 | `content` | Tamanho, quota, versão opaca, revisão e hashes não inventados |
| MOD-REM-09 | `state` | Trashed/explicitlyTrashed/Shared etc. não são presumidos falsos por ausência |
| MOD-REM-10 | `access` | Capacidades só são Known se equivalência e evidência forem válidas; canMove não simplifica capacidades distintas de modo falso |
| MOD-REM-11 | `presentation` | Links preservados quando disponibilizados; ausência não produz URL falsa |
| MOD-REM-12 | `provider_data` | `NotCollected` distingue payload não obtido de payload conhecido; envelope mantém ID do provedor |
| MOD-PRO-01 | `ProviderFileData.provider` | Coerente com RemoteFile e provedor da conta |
| MOD-PRO-02 | `format` e `format_version` | Formato conhecido/versionado; versão desconhecida não é decodificada silenciosamente |
| MOD-PRO-03 | `payload` | Preserva metadados escolhidos do recurso original, sem conteúdo do arquivo nem credenciais |
| MOD-PRO-04 | `GoogleFile` parcial | Mantém distinção entre valor recebido, false, zero, lista vazia e campo não solicitado |
| MOD-PRO-05 | `GoogleFile.id` ausente | Não criar RemoteFileRef fictício; sinalizar reavaliação sem dados do arquivo |
| MOD-PRO-06 | `GoogleChange.removed` | Remoção ou perda de acesso não vira deleção confirmada |
| MOD-PRO-07 | renomeação/movimento | Identidade por conta + ID é preservada, embora nome/pais mudem |
| MOD-PRO-08 | atalhos/itens nativos | Preservar target/formatos específicos sem converter a arquivo comum por hipótese |
| MOD-PRO-09 | recursos sensíveis | Não incluir tokens, wrapped keys, usuários e geo em logs/filas por padrão sem revisão |
| MOD-PRO-10 | formato, tamanho e política | BLOCKED até serem formalizadas codificação, limites e retenção do envelope |
| MOD-REM-13 | evolução de esquema | Detector Remoto permanece inalterado quando adaptador ganha/perde atributo |
| MOD-REM-14 | independência | Models não importa GoogleFile, cliente API, detector ou inventário |

**Responsabilidade dos testes:** Models valida dados puros e invariantes; o **adaptador Google** testa mapeamento fiel de `GoogleFile`, campos opcionais, filtros, codificação e sanitização; o Detector Remoto testa polling/cursor/entrega usando adaptador fake; o Storage testa a persistência integral da observação.

**Pré-requisito de implementação:** aprovar matriz por campo **manter / sob demanda / descartar** e regras de segurança. Estes são testes especificados para o futuro, não executados.

## 4. FileData, FileEvent e File

| Caso | Objeto/campo | Verificação |
| --- | --- | --- |
| MOD-DAT-01 | `FileData::Local` | Encapsula exatamente um `LocalFile` sem perda de metadados |
| MOD-DAT-02 | `FileData::Remote` | Encapsula exatamente um `RemoteFile` sem perda de metadados |
| MOD-DAT-03 | `FileData` | Somente variantes `Local` e `Remote`; Orquestrador não interpreta variante |
| MOD-FIL-01 | `File.id` | ID da ocorrência estável entre retries, distinto do ID de arquivo |
| MOD-FIL-02 | `File.sync_id` | Cada ocorrência pertence à sincronização correta |
| MOD-FIL-03 | `File.event.kind` | Oito variantes: Created, Modified, Deleted, Renamed, Moved, MetadataChanged, RecheckRequired, ScopeUnavailable |
| MOD-FIL-04 | `File.observed_at` | Instante de observação, não mtime |
| MOD-FIL-05 | `File.coverage` | Complete, Partial e Unavailable mantêm a semântica de escopo |
| MOD-FIL-06 | `File.scope` | Local(path), Remote(ref) ou WholeSync sem inferir exclusão |
| MOD-FIL-07 | `File.data` | `Some(Local)` e `Some(Remote)` exigem escopo compatível |
| MOD-EVT-01 | `Created` | Tem `current` + `data`; `previous=None` não é prova isolada de criação |
| MOD-EVT-02 | `Modified` | Tem `current` + `data`; não determina sozinho comparação de conteúdo |
| MOD-EVT-03 | `Deleted` | Pode conter `data=None` e `previous=Some` com `current=None`; não autoriza exclusão automática |
| MOD-EVT-04 | `Renamed` | Exige `previous`, `current`, `data` e correlação de identidade comprovada |
| MOD-EVT-05 | `Moved` | Exige origem/destino, `data` e correlação de identidade comprovada |
| MOD-EVT-06 | `MetadataChanged` | Tem metadados atuais; não presume conteúdo alterado |
| MOD-EVT-07 | `RecheckRequired` | Aceita `data=None`, escopo conhecido, informações parciais |
| MOD-EVT-08 | `ScopeUnavailable` | Exige `coverage=Unavailable`, `data=None`, sem `current` inventado |
| MOD-EVT-09 | `FileLocation::Local` | Preserva bytes de caminho anterior e atual |
| MOD-EVT-10 | `FileLocation::Remote` | Qualifica ID por conta; parent e name opcionais distinguem desconhecido de observado |
| MOD-EVT-11 | transição | Não misturar localização local/remota entre `previous` e `current` para Move/Rename |
| MOD-EVT-12 | mudança simultânea | Renomear + mover não gera falsa afirmação de exclusividade; inconclusivo usa RecheckRequired |
| MOD-EVT-13 | `WholeSync` | Permitido apenas com `data=None` |
| MOD-EVT-14 | integridade do evento | Reentrega preserva objeto completo; `FileEvent` não executa operação nem decide conflito |

**Regra dos testes:** verificar a estrutura e consistência dos dados em Models; teste de classificação de eventos por watcher/API pertence aos Detectores; teste de confirmação e retry pertence ao Detector/Storage; tomada de decisão pertence ao Analisador.

## 5. Integração externa e critérios de aceite

| Caso | Proprietário | Contrato a testar em sua suíte |
| --- | --- | --- |
| MOD-EXT-01 | Detector | `FileBatch.files` transporta `Vec<File>` completos mesmo quando há um único elemento |
| MOD-EXT-02 | Detector Storage | A notificação persistida conserva `File.id` e todo seu conteúdo em retry |
| MOD-EXT-03 | Orquestrador/Analisador | Orquestrador não interpreta `FileData`; Analisador pode distinguir variantes |
| MOD-EXT-04 | Inventário | Revisão futura do contrato `SyncItem`, sem modificar Models antecipadamente |

**Esses quatro casos são pontos de integração, não testes unitários do módulo Models.**

A implementação futura exige seguir a seleção local desta revisão e resolver as dependências remotas antes do envelope completo. Executar testes de entrada/saída, opcionais e invariantes; registrar PASS/FAIL/BLOCKED no ambiente Flatpak. O estado dos testes executáveis locais está na seção 8; nenhum caso remoto/envelope é declarado executado.


## 6. Índice por método puro do fluxo local

| Método especificado | Casos | Fonte contratual |
| --- | --- | --- |
| LocalRelativePath::new | MOD-PTH-01 a MOD-PTH-03 | INTERFACES §§ 2.5/2.8 |
| LocalRelativePath::as_bytes | MOD-PTH-04 | INTERFACES § 2.8 |
| LocalTimestamp::new | MOD-TIM-01 a MOD-TIM-02 | INTERFACES § 2.8 |
| LocalFile::validate | MOD-VAL-01 a MOD-VAL-04 | INTERFACES §§ 2.4/2.8 |
| File::validate (combinações locais) | MOD-VAL-05 a MOD-VAL-09 | INTERFACES §§ 5.4–5.6 |

### LocalRelativePath::new e as_bytes

**Contrato:** assinaturas e validação lexical de INTERFACES § 2.8. Fixtures como vetores explícitos de bytes, não produzidos pelo mesmo normalizador do código sob teste.

**Sucesso:** MOD-PTH-01 aceita `dir/file`, `.hidden`, `..backup`, `a..b`, espaços/quebra de linha, barra invertida literal e componente com byte 0xff. Confirmar os bytes esperados sem UTF-8 com perda.

**Negativos/opcionais:** MOD-PTH-02 rejeita vazio, `/`, `/file`, `a/`, `a//b`, `.`, `..`, `a/./b`, `a/../b` e NUL em qualquer componente. MOD-PTH-03 aceita `.drivesync/file` lexicalmente; nenhum teste de existência, symlink ou política é executado pelo modelo.

**Retorno/invariantes e verificação:** InvalidRelativePath nas entradas inválidas; MOD-PTH-04 as_bytes devolve os mesmos bytes por empréstimo imutável sem normalização. Confinamento físico é testado fora de Models. Se houver desserialização futura, repetir os negativos por essa entrada, sem permitir construção inválida.

### LocalTimestamp::new

**Contrato:** segundos i64 e nanos u32, conforme INTERFACES § 2.8.

**Sucesso:** MOD-TIM-01 preserva segundos -1/0/positivos e nanos 0/999_999_999. Epoch zero é valor real, não estado de ausência.

**Negativos/opcionais:** MOD-TIM-02 rejeita 1_000_000_000 e u32::MAX com InvalidTimestamp, sem carry automático para segundos. Ausência usa MetadataValue e não chama o relógio.

**Retorno/invariantes e verificação:** comparação campo a campo dos valores preservados, sem conversão de timezone, truncamento ou resolução de ordem temporal.

### LocalFile::validate

**Contrato:** consistência estrutural de INTERFACES §§ 2.4/2.8.

**Sucesso:** MOD-VAL-01 valida cada tipo de entrada, básicos completos e objetos parciais coerentes; local_id=None preservado. Fixtures distintas de zero para evitar sucesso acidental por defaults.

**Negativos/opcionais:** MOD-VAL-02 rejeita LocalFileId vazio/whitespace, timestamp inválido, modo incoerente, tipo/mode divergentes, máscara ausente e NotApplicable em campo local não autorizado; asserts de erros tipados sem incluir caminho na mensagem.

**Retorno/invariantes:** MOD-VAL-03 cobre btime e flags opcionais, cada estado de falha, par de flags coerente e target aplicável/não aplicável. Não usar model.validate para exigir que todo atributo seja Known.

**Verificação:** MOD-VAL-04 compara o objeto antes/depois de validação válida e inválida; nada é preenchido, apagado ou resolvido. Device/inode iguais em duas fixtures não criam LocalFileId. Datas em ordem inesperada não são rejeitadas só pela ordem.

### File::validate — fluxo local

**Contrato:** INTERFACES §§ 5.4–5.6 e os oito eventos já descritos em MOD-EVT-01 a MOD-EVT-14. Um teste puro verifica os dados apresentados, não a veracidade de um sinal de watcher.

**Sucesso:** MOD-VAL-05 parametriza as oito variantes com fixtures locais explicitamente compatíveis com a tabela da seção 5.5. Created/Modified/MetadataChanged têm current/data; Rename/Move têm origem/destino diferentes; Deleted tem previous sem dados atuais; Recheck e ScopeUnavailable admitem as ausências documentadas.

**Negativos/opcionais:** MOD-VAL-06 remove cada campo obrigatório e troca isoladamente sync_id/FileId por vazio/whitespace, path de scope/current/data, origem local por remota e coverage por Unavailable em evento específico; esperar erro tipado correspondente. MOD-VAL-07 aceita WholeSync apenas em RecheckRequired/ScopeUnavailable sem dados/localizações; rejeita path vazio e WholeSync com Created, Deleted ou dados.

**Retorno/invariantes:** MOD-VAL-08 aceita opcionais de btime/flags sem degradar perfil básico; rejeita Complete quando falta obrigatório ou kind é Unknown; aceita a observação parcial com coverage Partial. O validador não muda Partial para Complete nem infere exclusão de None.

**Verificação:** MOD-VAL-09 clona uma ocorrência, valida as cópias e confirma preservação integral de FileId, observed_at, contexto e LocalFile; uma nova fixture pode ter outro FileId para o mesmo caminho. Teste de geração de IDs e retry real fica no produtor/Storage. Ausência de APIs operacionais é verificada por revisão de dependências, não afirmada por um mock de I/O.

## 7. Critérios de escopo e evolução

- A matriz de decisão cobre todos os grupos e campos do catálogo local; o código futuro só pode incluir os campos selecionados.
- Cada método puro tem sucesso, negativos/opcionais, retorno/invariantes e evidência independente; estados de ausência e bytes Unix estão explícitos.
- A matriz de eventos locais fecha combinações de data, scope, previous/current e coverage sem transformar observação em comando.
- Testes remotos e de FileData::Remote continuam documentais e dependentes da revisão remota; não fabricar um RemoteFile para declarar o envelope completo implementável.
- A implementação atual fica nos modelos locais e testes puros. Não implementar migração de Inventário, envelope remoto ou detector por consequência. Testes físicos e durabilidade não são comprovados por esta suíte.


## 8. Evidências da implementação incremental

Foram implementados 18 testes puros em `engine/src/models/tests.rs`, cobrindo quatro métodos locais (`LocalRelativePath::new/as_bytes`, `LocalTimestamp::new`, `LocalFile::validate`). Nenhum teste usa filesystem, rede, relógio ou credencial para exercer os modelos. Os testes de regressão de AccountId/ProviderId/Timestamp usam JSON em memória.

| Contrato | Evidência executável |
| --- | --- |
| MOD-PTH-01 a 04 | Preservação de bytes não UTF-8, nomes especiais/reservados, empréstimo imutável e rejeição dos caminhos inválidos |
| MOD-TIM-01/02 | Segundos assinados/extremos e frações válidas/inválidas, sem carry |
| MOD-ID-03/04 | Compatibilidade dos tipos reexportados e JSON das identidades existentes |
| MOD-LOC-01/02/03/04/05/06/08/13/15 | Tipos, modos/permissões, owner, datas, zeros, device/inode, flags, alvos e estados distintos |
| MOD-VAL-01 a 04 | Observação completa/parcial; identidade fornecida; timestamps Known inválidos; NotApplicable em cada atributo; flags coerentes/incoerentes; ausência de mutação e mensagens de erro sem payload |
| MOD-LOC-16/19 | Revisão do código confirma pureza e exclusão das categorias descartadas/sob demanda; não são provas de coleta física |

RED foi observado em duas unidades: ausência dos tipos primitivos e ausência de LocalFile/grupos (logs de compilação). GREEN: `cargo test --offline --locked -p drivesync-engine --lib models::` passou com 6 testes na fundação e com 18 após LocalFile/validate.

Ambiente: `org.gnome.Sdk//50` e a extensão rust-stable do manifesto, com `CARGO_HOME=/home/tosta/.cargo` explicitamente exposto ao SDK. Isso resolveu o cache anteriormente invisível, sem instalar SDK ou extensão. Novos modelos não definem serialização; testar desserialização fica pendente de um contrato de formato posterior.

Casos MOD-VAL-05 a 09, MOD-EVT/MOD-FIL/MOD-DAT e remotos permanecem pendentes de implementação do envelope/revisão remota. Cobertura Complete/Partial física, construção do File, IDs/retry de eventos e estabilidade diante de corridas não são demonstrados por LocalFile::validate. A coleta permanece em outro componente.


Validação final executada no mesmo SDK, com cache exposto, em 2026-11-13:

- `cargo fmt --all --check`: PASS.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --offline --locked --workspace`: 536 testes PASS e 1 doctest PASS, 6 testes ignorados (5 de desktop/portal e 1 de Secret Service isolado).
- `cargo doc --offline --locked --workspace --no-deps --document-private-items`: PASS, sem warnings.
- `cargo test --offline --locked --workspace --doc`: 1 doctest PASS.

A execução inclui testes de providers contra servidores locais simulados; não comprova conta Google real, uso instalado do Flatpak, portal ou observação de filesystem pelo detector.
