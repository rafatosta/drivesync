# Inventário — TESTS

## 1. Objetivo, fonte de verdade e limites

Este documento especifica **os testes a escrever antes de implementar** o módulo `inventory/`, seguindo **Contract-First + TDD**. A referência normativa de API é [INTERFACES.md](INTERFACES.md); a referência do modelo relacional e dos bancos é [README.md](README.md). Quando documentação e implementação divergirem, **não ajustar o teste para esconder a divergência**: corrigir a implementação ou formalizar a decisão no contrato antes de continuar.

**Escopo:** os **16 métodos públicos** de `InventoryManager`, todos os tipos/atributos utilizados por esses métodos e os invariantes do banco `<raiz>/.drivesync/inventory.sqlite3`. O teste deve verificar **comportamento observável pela API**, persistência real e integridade, sem transformar o layout interno das tabelas em contrato público.

Não executar Google Drive real, OAuth, interface GTK, transferência física, nem alterar arquivos de usuário. Não criar nem modificar o banco global da aplicação, que pertence a outros módulos. Não criar compatibilidade com versões antigas da engine.

### Camadas de teste

| Camada | Meio de execução | Evidências |
| --- | --- | --- |
| Compilação/API | Código Rust consumidor do módulo, imports públicos e construções de requests | Assinaturas, nomes, tipos e visibilidade |
| Unidade | Validadores e transformações internas | Presença, campos opcionais, erros e precondições |
| Integração isolada | **SQLite real** em diretórios temporários individuais | Reabertura, transações, constraints, índices, exclusões e recuperação |
| Concorrência | Dois handles/threads sobre mesmo SQLite temporário | Lock, revisão otimista e reivindicação única de operação |
| Falhas | Erros injetados em pontos de persistência definidos pela implementação | Rollback, idempotência e ausência de falso sucesso |
| Segurança de integração | Fixtures `File` de Models com `FileData::Local/Remote`, sem rede nem pastas pessoais | Isolamento, sanitização e nenhuma modificação física |

**Princípio:** um `mock InventoryManager` que apenas retorne objetos pré-programados **não** verifica persistência, chaves estrangeiras ou atomicidade. Os testes principais devem atravessar a API pública e operar em SQLite verdadeiro.

## 2. Organização TDD e fixtures

### Ciclo de execução

1. **RED:** escrever teste de assinatura e teste de comportamento contra o contrato. A falha inicial deve representar método/funcionalidade ausente e não erro de fixture.
2. **GREEN:** implementar apenas a infraestrutura e a ação necessárias para passar naquele conjunto.
3. **REFACTOR:** simplificar sem alterar comportamentos públicos; executar regressão dos grupos anteriores.
4. Repetir para abertura/escopo, consultas, observações, vínculos, confirmação, fila, eventos e histórico.
5. Executar todas as matrizes de erros, opcionalidade, concorrência, segurança e recuperação antes de declarar pronto.

### Fixtures determinísticas

| Fixture | Conteúdo | Uso |
| --- | --- | --- |
| `fresh_root` | Diretório temporário vazio, sem `.drivesync/` | Primeira criação |
| `initialized_root` | Pasta com banco inicializado sob `SyncIdentity` conhecida | Reabertura e validação |
| `remote_only_file` | `RemoteFile` sintético, `LocalFile=None`, presença remota `Present` | Download inicial |
| `local_only_file` | `LocalFile` sintético, `RemoteFile=None`, presença local `Present` | Upload inicial |
| `linked_file` | Representações local/remota associadas, sem conflitos | Consulta e comparação |
| `linked_directory` | Diretório com identidades de pais coerentes | Hierarquia |
| `workspace_native` | Documento Google nativo sem checksum/tamanho de conteúdo comum | Campos não aplicáveis |
| `partial_remote` | Resposta remota sintética com `fields` incompleto | Preservação de valores anteriores |
| `confirmed_item` | Baseline verificado, revisão conhecida | Confirmação e divergência |
| `pending_queue` | Operações com direções, tipos e estados distintos | Listagem, claim e retries |
| `remote_page` | Cursor, eventos paginados e chaves idempotentes | Transação e reconhecimento |
| `activity_history` | Atividades com `item_id` válido, erros e resultados variados | Consulta histórica |

**Isolamento obrigatório:** criar diretório temporário distinto por teste, conta/sync IDs fictícios e zero dados pessoais. Fechar todos os handles SQLite antes do descarte. Se os testes rodarem sob Flatpak, usar ambiente de build/teste apropriado; não depender do sistema do usuário. Os relógios, geradores de IDs e pontos de falha devem ser determinísticos por injeção interna de dependência, **sem expô-los como API de produção**.

A fonte dos objetos `LocalFile` e `RemoteFile` pertencem a Models e entram em observações encapsuladas em `File`. Enquanto `LocalFile` estiver aberto, uma fixture mínima tipada pode ser usada **apenas para testar o Inventário**, sem inventar atributos definitivos. Testes de campos definitivos deverão ser completados antes da implementação da persistência desses campos.

## Matriz de integração ao novo contrato Models

Os testes abaixo validam **o contrato-alvo**, não um módulo compilado. `LocalFile` e `RemoteFile` vêm de [Models](../models/INTERFACES.md); `File` é o envelope de observação que o Analisador encaminha ao Inventário. O Inventário possui `SyncItem`, `SyncBaseline` e estados de persistência.

| ID | Cenário | Resultado obrigatório |
| --- | --- | --- |
| INV-MDL-01 | `record_local` recebe `File.data=Some(FileData::Local(...))` | Último snapshot local do `SyncItem` atualizado sem avançar baseline |
| INV-MDL-02 | `record_remote` recebe `File.data=Some(FileData::Remote(...))` | Último snapshot remoto genérico, com `RemoteFileRef` qualificado por conta, atualizado sem depender de `GoogleFile` |
| INV-MDL-03 | Método local recebe `FileData::Remote` ou vice-versa | Erro de variante/origem, rollback completo |
| INV-MDL-04 | `File.data=None`, sinal de reavaliação | Não remover último snapshot, não inventar `AbsentConfirmed` ou `SyncItem` |
| INV-MDL-05 | `FileEventKind::Deleted` e cobertura parcial | Nenhuma exclusão ou presença `AbsentConfirmed` automática |
| INV-MDL-06 | Ausência efetivamente comprovada com prova do escopo | Atualizar `PresenceState`, manter tombstone, não avançar baseline |
| INV-MDL-07 | `MetadataValue::NotCollected` / `Unsupported` / `Unavailable` | Preservar valores antigos e estado de coleta, sem preencher `0/false/[]` |
| INV-MDL-08 | `MetadataValue::Known(false)` / `Known(0)` / `Known([])` | Persistir os valores conhecidos sem confundi-los com campos omitidos |
| INV-MDL-09 | Duas contas com mesmo ID remoto bruto | `RemoteFileRef` evita colisão; isolamento por `AccountId` |
| INV-MDL-10 | Mesmo arquivo renomeado/movido com identidade correlacionada | `SyncItemId` estável; atualizar localização, baseline anterior preservado |
| INV-MDL-11 | Retry de mesma ocorrência `File.id` | Não criar item ou atividade duplicados; não confundir `FileId` com `SyncItemId` |
| INV-MDL-12 | Observação remota com `ProviderFileData` filtrado | Persistência apenas dos campos aprovados, codec versionado, nenhum token/conteúdo |
| INV-MDL-13 | Resposta nativa de provedor sem tamanho/hash | Não fabricar metadados |
| INV-MDL-14 | Mesmo `SyncItem` local/remoto, versões divergem do baseline | Inventário preserva estados; **Analisador** decide conflito/operação |
| INV-MDL-15 | Observação com `FileScope::WholeSync` sem identidade | Não criar item por nome/caminho; manter necessidade de reavaliação fora da tabela de itens |
| INV-MDL-16 | Confirmação verificada após operação | Baseline, status, operação e histórico atômicos; não relacionada a ack do Detector |
| INV-MDL-17 | Evolução dos campos `LocalFile`/`RemoteFile` | API `InventoryManager` mantém assinaturas; revisar só mapeamento de persistência autorizado |
| INV-MDL-18 | Detector reentrega evento cujo `SyncItem` já foi atualizado | Inventário trata registro idempotente sem criar fila/cursor remoto adicional |

**Bloqueios de implementação:** mapear campos selecionados de `LocalFile` e `RemoteFile` para persistência mantendo estados de coleta; definir codec de `ProviderFileData`; checkpoint/eventos são de propriedade do Detector, sem API duplicada no Inventário. Não registrar PASS para os testes acima sem executar SQLite real com fixtures de Models.

## Responsabilidades excluídas dos testes do Inventário

Os testes de cursor, paginação remota, publicação de eventos, `FileBatch`, claim/lease e reconhecimento de eventos **pertencem aos testes do Detector Remoto e Detector Storage**, não ao Inventário. Os antigos cenários INV-15 a INV-18 dessa natureza foram retirados. Os novos INV-15 e INV-16 testam respectivamente `record_activity` e `list_activities`. Não criar mocks de APIs removidas ou adaptar interfaces legadas.

## Testes dos contratos internos por submódulo

Estas são **propostas de casos de teste**, ainda não executadas. Cada módulo interno só precisa de SQLite isolado e fakes das demais dependências; os testes de integração cruzada devem passar pela fachada `InventoryManager`, sem expor SQL publicamente.

| Identificador | Submódulo | Verificação |
| --- | --- | --- |
| INV-INT-ITEM-01 | [items](items/INTERFACES.md) | Resolver pelas quatro variantes de `ItemSelector`, preservar `SyncItemId`, unicidade de vínculo remoto por conta |
| INV-INT-ITEM-02 | items | `FieldUpdate::Keep/Set/Clear`, revisão otimista, `AbsentConfirmed` somente com prova, tombstone |
| INV-INT-META-01 | [metadata](metadata/INTERFACES.md) | `LocalFile` e `RemoteFile` nos campos aprovados: round-trip SQLite e preservação dos bytes locais |
| INV-INT-META-02 | metadata | `MetadataValue::NotCollected` não sobrescreve `Known`; `Known(0/false/[])` é persistido corretamente |
| INV-INT-META-03 | metadata | `RemoteFileRef` qualificado, pais múltiplos e `ProviderFileData` filtrado conforme decisão de codec |
| INV-INT-BASE-01 | [baseline](baseline/INTERFACES.md) | Observações não avançam baseline; `confirm_sync` verificado cria/substitui baseline |
| INV-INT-BASE-02 | baseline | Rollback completo se status, conclusão de operação ou atividade falhar na mesma transação |
| INV-INT-OPS-01 | [operations](operations/INTERFACES.md) | Claim concorrente, retry/backoff, estado após reinício, precondições remotas e deduplicação |
| INV-INT-OPS-02 | operations | `Completed` somente via `confirm_sync`; operação pendente não implica transferência realizada |
| INV-INT-ACT-01 | [activity](activity/INTERFACES.md) | `item_id` obrigatório, `operation_id` opcional, histórico imutável e tombstone preservado |
| INV-INT-ACT-02 | activity | Replay de confirmação não duplica atividade, com chave de idempotência a definir |
| INV-INT-STO-01 | [storage](storage/INTERFACES.md) | Identidade por sincronização, erro em banco corrompido, FK/UNIQUE e isolamento de conexões |
| INV-INT-STO-02 | storage | Transação multi-repositório, rollback e recuperação após reabertura, sem `EventStore` |

**Não inventar evidência de testes:** a existência destes casos não implica compilação, execução ou aprovação. Casos dependentes de DDL, codec, política de idempotência e revisão de metadados ficam pendentes até que seus contratos sejam aprovados.

## 3. Cobertura obrigatória: todos os métodos

| Caso | Método público | Objeto de entrada | Objeto de saída |
| --- | --- | --- | --- |
| INV-01 | `open` | `OpenInventoryRequest` | `OpenInventoryResult` |
| INV-02 | `get_sync_info` | `GetSyncInfoRequest` | `GetSyncInfoResult` |
| INV-03 | `mark_initial_download_complete` | `MarkInitialDownloadCompleteRequest` | `MarkInitialDownloadCompleteResult` |
| INV-04 | `get_item` | `GetItemRequest` | `GetItemResult` |
| INV-05 | `list_items` | `ListItemsRequest` | `ListItemsResult` |
| INV-06 | `record_local` | `RecordLocalObservation` | `RecordObservationResult` |
| INV-07 | `record_remote` | `RecordRemoteObservation` | `RecordObservationResult` |
| INV-08 | `link_item` | `LinkItemRequest` | `LinkItemResult` |
| INV-09 | `update_item_status` | `UpdateItemStatusRequest` | `UpdateItemStatusResult` |
| INV-10 | `confirm_sync` | `ConfirmSyncRequest` | `ConfirmSyncResult` |
| INV-11 | `enqueue_operation` | `EnqueueOperationRequest` | `EnqueueOperationResult` |
| INV-12 | `list_operations` | `ListOperationsRequest` | `ListOperationsResult` |
| INV-13 | `claim_operation` | `ClaimOperationRequest` | `ClaimOperationResult` |
| INV-14 | `update_operation` | `UpdateOperationRequest` | `UpdateOperationResult` |
| INV-15 | `record_activity` | `RecordActivityRequest` | `RecordActivityResult` |
| INV-16 | `list_activities` | `ListActivitiesRequest` | `ListActivitiesResult` |

Todo método acima exige: (a) caso de sucesso; (b) entrada inválida ou precondição conflitante; (c) retorno/erro correto; (d) persistência e reabertura quando altera dados; (e) ausência de efeitos fora de sua responsabilidade; (f) teste de `Option`/filtros/transições quando aplicável.

## 4. Casos por método

### INV-01 — `open`

**Contrato:** `open(OpenInventoryRequest) -> InventoryResult<OpenInventoryResult>`.

**Sucesso:** Criar inventário em raiz vazia; reabrir e confirmar `SyncIdentity`; validar `created` e `SyncInfo`.

**Negativos e opcionais:** Inventário ausente com criação proibida; diretório não vazio sem inventário; identidade divergente; esquema desconhecido/corrompido; permissão negada. Nunca sobrescrever banco.

**Persistência e invariantes:** Abrir novamente mantém `sync_info`, IDs e geração; nenhum acesso ao banco global.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-02 — `get_sync_info`

**Contrato:** `get_sync_info(GetSyncInfoRequest) -> InventoryResult<GetSyncInfoResult>`.

**Sucesso:** Reabrir e recuperar escopo, `generation`, flag inicial; request vazio.

**Negativos e opcionais:** Banco indisponível ou ilegível: erro tipado; nenhuma conexão remota.

**Persistência e invariantes:** Repetir consulta não muda estado ou timestamps.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-03 — `mark_initial_download_complete`

**Contrato:** `mark_initial_download_complete(MarkInitialDownloadCompleteRequest) -> InventoryResult<MarkInitialDownloadCompleteResult>`.

**Sucesso:** Marcar conclusão inicial somente após pré-condições e geração esperada.

**Negativos e opcionais:** Geração diferente; `expected_complete` não corresponde ao persistido; tentativa de confirmar antes de materialização verificada.

**Persistência e invariantes:** Transição persiste após reabertura; nenhuma alteração de baseline por essa flag.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-04 — `get_item`

**Contrato:** `get_item(GetItemRequest) -> InventoryResult<GetItemResult>`.

**Sucesso:** Buscar mesmo item por `Id`, `RemoteRef`, `LocalId`, `LocalRelativePath`; reconstruir `LocalFile?`, `RemoteFile?`, baseline e presenças.

**Negativos e opcionais:** Seletor sem correspondência retorna `item=None`; seletor malformado erro; ambiguidades locais não retornam item arbitrário.

**Persistência e invariantes:** Renomeação/movimentação preservam `item_id`; lookup não modifica revisão.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-05 — `list_items`

**Contrato:** `list_items(ListItemsRequest) -> InventoryResult<ListItemsResult>`.

**Sucesso:** Filtro de status, presenças, pai local e remoto; paginação de árvore com mais itens que limite.

**Negativos e opcionais:** `limit=0`, acima do máximo, cursor incompatível ou inválido; filtro sem matches retorna página vazia.

**Persistência e invariantes:** Múltiplas páginas cobrem todos os itens sem duplicar; cursor e ordenação estáveis sob critério definido.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-06 — `record_local`

**Contrato:** `record_local(RecordLocalObservation) -> InventoryResult<RecordObservationResult>`.

**Sucesso:** Criar item local; observar edição, renomeação e alteração de diretório; atualizar somente campos observados.

**Negativos e opcionais:** `Present` sem `file`; `AbsentConfirmed` sem prova suficiente; atualização sem versão válida; caminho/identidade ambíguos.

**Persistência e invariantes:** Baseline inalterado; repeated observation idempotente; ausência de raiz acessível não vira exclusão.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-07 — `record_remote`

**Contrato:** `record_remote(RecordRemoteObservation) -> InventoryResult<RecordObservationResult>`.

**Sucesso:** Criar item remoto; observar versão, nome, pais, MIME e hashes disponíveis; respeitar projeções parciais.

**Negativos e opcionais:** `Present` sem identidade do arquivo; `AbsentConfirmed` baseado em erro de rede/paginação incompleta; revisão obsoleta; ID duplicado.

**Persistência e invariantes:** Campos omitidos do Google não são limpos; baseline fica intacto; reabrir preserva identidade.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-08 — `link_item`

**Contrato:** `link_item(LinkItemRequest) -> InventoryResult<LinkItemResult>`.

**Sucesso:** Vincular lado local a remoto, preservar `SyncItemId`, aplicar `Keep`/`Set`/`Clear` para os dois vínculos.

**Negativos e opcionais:** FK inexistente, referência já vinculada em outro item, revisão divergente, solicitação de relacionamento inconsistente.

**Persistência e invariantes:** Desvincular não exclui arquivos físicos; efeito atômico em FKs, revisão e unicidade.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-09 — `update_item_status`

**Contrato:** `update_item_status(UpdateItemStatusRequest) -> InventoryResult<UpdateItemStatusResult>`.

**Sucesso:** Registrar `Pending*`, `Synced`, `Conflict`, `Error` e razão via `Keep`/`Set`/`Clear`.

**Negativos e opcionais:** Revisão obsoleta; estado inválido; razão incompatível conforme regra definida; item inexistente.

**Persistência e invariantes:** Não altera baseline, operações ou metadados de observação; status e revisão persistem.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-10 — `confirm_sync`

**Contrato:** `confirm_sync(ConfirmSyncRequest) -> InventoryResult<ConfirmSyncResult>`.

**Sucesso:** Confirmar observação verificável; atualizar baseline, estado, operação concluída e atividade na mesma transação.

**Negativos e opcionais:** Falha de validação, operação errada, ID de item diferente, atividade exigida ausente, revisão obsoleta, falha injetada entre escritas.

**Persistência e invariantes:** Rollback integral; nenhuma operação `Completed` sem baseline válido; mesma confirmação não duplica atividade.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-11 — `enqueue_operation`

**Contrato:** `enqueue_operation(EnqueueOperationRequest) -> InventoryResult<EnqueueOperationResult>`.

**Sucesso:** Enfileirar create/update/rename/move/trash/delete/restore; precondição de versão e deduplicação; offline.

**Negativos e opcionais:** Item inexistente; revisão obsoleta; ação não permitida; chave de deduplicação repetida/incompatível.

**Persistência e invariantes:** Fila durável entre reinícios; nenhum efeito no filesystem ou rede; nenhuma operação duplicada por retry.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-12 — `list_operations`

**Contrato:** `list_operations(ListOperationsRequest) -> InventoryResult<ListOperationsResult>`.

**Sucesso:** Listar por `item_id`, direção, estados, elegibilidade e paginação; vetor de estados vazio = tudo.

**Negativos e opcionais:** Cursor ruim, intervalo/paginação inválida, filtro sem matches.

**Persistência e invariantes:** Ordem estável; leitura não muda estados ou retira trabalho da fila.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-13 — `claim_operation`

**Contrato:** `claim_operation(ClaimOperationRequest) -> InventoryResult<ClaimOperationResult>`.

**Sucesso:** Duas conexões/threads competem por mesma operação: apenas uma reivindica; estado transita para Running.

**Negativos e opcionais:** `expected_state` obsoleto; operação bloqueada/concluída; ID inexistente.

**Persistência e invariantes:** Reivindicação persistente; evitar execução concorrente duplicada mesmo em SQLite real.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-14 — `update_operation`

**Contrato:** `update_operation(UpdateOperationRequest) -> InventoryResult<UpdateOperationResult>`.

**Sucesso:** Transições Retry/Block/Fail/Requeue com campos obrigatórios da variante; tentativas e erros coerentes.

**Negativos e opcionais:** Transição inválida, timestamp inválido, estado inesperado, operação inexistente.

**Persistência e invariantes:** Não oferece conclusão por atalho; operação concluída somente via `confirm_sync`.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-15 — `record_activity`

**Contrato:** `record_activity(RecordActivityRequest) -> InventoryResult<RecordActivityResult>`.

**Sucesso:** Gravar falha ou conflito de arquivo com `item_id` obrigatório; contexto histórico opcional; operação vinculada quando houver.

**Negativos e opcionais:** Item inexistente, operação de outro item, atividade global sem item, falta de campos de tipo/origem/resultado/data.

**Persistência e invariantes:** FK obrigatória; atividade permanece ligada a tombstone; não conclui operação por si.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.

### INV-16 — `list_activities`

**Contrato:** `list_activities(ListActivitiesRequest) -> InventoryResult<ListActivitiesResult>`.

**Sucesso:** Listar por item/operação/tipo/resultado e intervalo; valores históricos imutáveis após renomear.

**Negativos e opcionais:** Intervalo invertido, cursor inválido, filtro sem matches.

**Persistência e invariantes:** Ordenação temporal consistente; resultados persistem após reinício; não aparece atividade de outra pasta.

**Asserções obrigatórias:** comparar todos os atributos presentes na resposta, confirmar semântica de `None` e campos não aplicáveis, validar erros tipados e repetir os casos após fechar/reabrir `InventoryManager` quando houver escrita. Para cada teste de falha verificar que não ocorreu atualização parcial, excluindo-se apenas efeitos de teste previamente confirmados.


## 5. Matriz de opcionalidade, tipos e reconstrução

### Retornos de domínio

| Tipo | Cenários de teste necessários |
| --- | --- |
| `SyncItem` | `id`, `kind`, estados e revisão sempre definidos; `local=None` com remoto presente; `remote=None` com local presente; ambos observados; nenhum observado após tombstone; `baseline=None` inicial; motivos/timestamps opcionais |
| `SyncBaseline` / `BaselineSide` | Sem baseline antes de confirmação; presença por lado; nome local preservado em bytes Unix; nome remoto em texto; pai, versão, checksum, tamanho e data nulos quando indisponíveis |
| `SyncInfo` / `SyncIdentity` | Identidade estável, raiz correta, geração positiva; mesmo ID de conta mas raiz diferente deve ser rejeitado; `initial_download_complete` persiste |
| `SyncOperation` | Status e direction/operation kind coerentes; `expected_remote_version=None`; `next_attempt_at`, erro, inicio e conclusão dependendo do estado |
| `SyncActivity` | `item_id` **obrigatório**; `operation_id=None` para conflito; descrição, erro, nome remoto e caminho opcionais; dados históricos não se alteram com nomes atuais |
| `Page<T>` | Lista vazia, página parcial, `next_cursor=None` final, paginação em múltiplas páginas, cursor inválido e filtro incompatível |
| `InventoryFailure` | `kind`, `retryable` e `message` sempre presentes; mensagens sem SQL, conteúdo de arquivos, tokens ou segredos |

### Entradas: ausente, fornecido, limpar explicitamente

| Contrato | Caso `None`/vazio | Caso preenchido ou negativo |
| --- | --- | --- |
| `Pagination.after` | Começar da primeira página | Continuar com cursor válido; rejeitar cursor de filtro diferente |
| `ItemFilter`, `OperationFilter`, `ActivityFilter` | `Vec::new()` não restringe; `Option::None` não filtra | Filtrar pelos valores, combinando cláusulas; intervalos devem ser válidos |
| `GetItemResult.item` | Sem vínculo registrado | Retornar único item selecionado, sem inferir estado físico |
| `RecordLocalObservation.expected_revision` / remoto | `None` somente ao criar | Versão exigida ao atualizar; stale revision falha |
| `LocalObservation.file` / `RemoteObservation.file` | `None` com `Unknown` não apaga registro antigo | `Present` exige representação válida; ausência confirmada exige prova |
| `ObservationProof.complete_scope` | `false` não confirma ausência por falta de cobertura | `true` ainda depende de evidência válida, escopo e identidade |
| `ItemLinksChange` e `ItemStatusChange` | `Keep` conserva; `Clear` limpa somente associação/campo permitido | `Set` grava novo valor; FK e transação íntegros |
| `ConfirmSyncRequest.completed_operation` | `None` para confirmação sem fila | `Some` deve referenciar operação do mesmo item |
| `ConfirmSyncRequest.activity` | `None` apenas quando não é obrigatório registrar resultado | `Some` inclui `item_id` correto; ausência relevante rejeitada |
| `NewSyncOperation.expected_remote_version` | Não fingir versão conhecida | Valor conhecido protege contra versão remota inesperada |
| `NewSyncOperation.deduplication_key` | Sem chave explícita: sem deduplicação arbitrária | Mesma chave+ação retorna resultado idempotente; colisão divergente é rejeitada |
| `SyncActivity.operation_id` | `None` em evento por arquivo fora da fila | `Some` deve apontar a operação do mesmo item |
| `ActivityFilter.from` / `until` | Intervalo aberto | Validar limites e ordem temporal |
| `PresenceState::Unknown` | Falta de comprovação; **não** é ausência | `AbsentConfirmed` necessita evidência e nunca resulta só de falha de rede |

### Invariantes de tipos públicos

- Teste de compilação acessa **todos os tipos declarados nas seções 2–10 de `INTERFACES.md`**, incluindo enums, IDs, estruturas `*Request`, `*Result`, filtros e tipos parametrizados.
- Teste de compilação usa os **16 métodos com seus objetos nomeados**, sem converter para argumentos posicionais.
- Testar `FieldUpdate<T>` nos três casos, identidade `ItemSelector` em suas quatro variantes (`RemoteRef` com conta), `ObservationTarget` em suas quatro variantes, os estados de `SyncStatus`, `OperationState`, `ActivityType` e `InventoryError`.
- Todos os campos opcionais devem ter teste de **ausência legítima**, **presença válida** e **não alteração involuntária**; campos obrigatórios devem ter cobertura para valor válido e, quando representável, valor inválido.
- `RemoteFile` com campos de mídia inexistentes (documento nativo), `LocalFile` sem inode confiável e nomes locais não UTF-8 **não** devem ser convertidos para valores artificiais.
- Implementar construtores/`Default`/builders quando necessários para evolução de requests sem quebrar chamadores por literais de struct. A estabilidade da assinatura não prova, por si só, compatibilidade do construtor.

## 6. Integridade relacional e segurança do SQLite

| Caso | Verificação |
| --- | --- |
| INV-DB-01 — arquivo único | `<raiz>/.drivesync/inventory.sqlite3` contém metadados, fila operacional e histórico; **não** cria `history.sqlite3` |
| INV-DB-02 — escopo | `sync_info` impede abrir uma pasta com `account_id`, `sync_id` ou `remote_root_id` incompatível |
| INV-DB-03 — foreign keys | FKs ativadas em **toda conexão**; não persistir referência inexistente |
| INV-DB-04 — exclusividade remota | Mesma identidade Google não pertence a dois `SyncItem` na mesma sincronização |
| INV-DB-05 — exclusividade local | Mesma `local_file_id` não pertence a dois itens; identidade não depende só de inode/caminho |
| INV-DB-06 — pais remotos | Relação pai múltipla armazenada como linhas relacionais, não array JSON nem BLOB de struct |
| INV-DB-07 — baseline | Um baseline por `SyncItem`; não existe antes de confirmação |
| INV-DB-08 — atividade | `activity.item_id NOT NULL` e FK válida; não apagar histórico por cascata após excluir vínculo |
| INV-DB-09 — operações | Fila com ID e item corretos; retorno de reinício não duplica operação já concluída |
| INV-DB-10 — índices | Consulta de item remoto, status, fila elegível, eventos pendentes e atividades utiliza índices coerentes para volume relevante |
| INV-DB-11 — dados sem JSON opaco | Objetos, listas, mapas e filas não são salvos inteiros em JSON/BLOB; bytes Unix de **um nome individual** podem ser BLOB |
| INV-DB-12 — caminho reservado | `.drivesync/` não é emitido como conteúdo sincronizável e não é enviado ao Drive |
| INV-DB-13 — correção de erro | Violação UNIQUE, FK ou CHECK traduzida em `InventoryFailure`, sem SQL exposto |
| INV-DB-14 — múltiplas raízes | Dois bancos de sincronizações distintas não compartilham identidades locais, remotas ou estados; objeto de uma raiz não pode ser escrito em outra |
| INV-DB-15 — acesso privado | Nenhum método público expõe conexão, transação ou SQL; outros módulos acessam via requests/responses |
| INV-DB-16 — versionamento | `PRAGMA user_version` compatível; esquema inesperado/corrompido falha **sem recriação automática** |

A inspeção do esquema é permitida **nos testes internos do módulo**, para confirmar FKs, índices e a ausência de campos opacos. Isso não altera a API pública e não autoriza consumidores a usarem SQL.

## 7. Transações, rollback, concorrência e idempotência

| ID | Experimento | Evidência exigida |
| --- | --- | --- |
| INV-TX-01 | Falha entre persistir observação e atualizar vínculo | Rollback integral, sem `local_files`/`remote_files` órfãos que causem ambiguidade |
| INV-TX-02 | Falha entre confirmar baseline e finalizar operação | Nenhuma conclusão parcial após reabertura |
| INV-TX-03 | Falha entre completar operação e gravar `activity` | Operação não aparece concluída sem atividade relevante |
| INV-TX-05 | Dois writers atualizam mesma revisão | Um sucesso e outro `VersionMismatch`; nenhuma escrita perdida |
| INV-TX-06 | Dois workers reivindicam mesma operação | Exatamente um `ClaimOperationResult` bem-sucedido |
| INV-TX-07 | Mesma observação reaplicada | Sem duplicar `SyncItem`; política explícita para revisão/no-op |
| INV-TX-09 | Mesmo enqueue com chave de deduplicação | Uma operação lógica ou resultado idempotente, não duas execuções |
| INV-TX-10 | Falha `SQLITE_BUSY`, `FULL`, I/O ou permissão | Erro tipado, nenhum falso sucesso, retry apenas se seguro |
| INV-TX-11 | Repetir uma confirmação após resposta perdida | Não produzir duas atividades nem avançar baseline indevidamente |
| INV-TX-12 | Crash/reinício com operação `Running` | Reavaliar resultado remoto antes de repetir; fila durável |
| INV-TX-14 | Renomear/mover arquivo após atividade | Histórico preserva identidade e fotografia histórica |
| INV-TX-15 | Remoção do item lógico com histórico existente | Tombstone mantém FK válida; não aplicar CASCADE |
| INV-TX-16 | Fechar inventário e reabrir após commit | Metadados, baseline, fila de operações e histórico consistentes |

**Não afirmar que testes isolados simulam crash de processo se só fecham conexões.** Para garantia mais forte, executar subprocesso descartável que finalize abruptamente entre checkpoints, depois reabrir e verificar invariantes. Injeção de falha no SQLite é uma evidência complementar.

## 8. Estados, situações reais e cenários de sincronização

| Caso | Estado inicial | Estímulo | Resultado a exigir do Inventário |
| --- | --- | --- | --- |
| INV-SC-01 | Remoto conhecido, local não materializado | Nova observação remota | `remote=Some`, `local=None`, baseline ausente |
| INV-SC-02 | Local conhecido, remoto ainda não associado | Alteração local durante offline | Observação + operação upload persistidas; sem contato com Drive |
| INV-SC-03 | Baseline confirmado | Local muda com app fechado | Após nova observação, local difere do baseline; Inventário **não decide** operação |
| INV-SC-04 | Baseline confirmado | Remoto muda enquanto app estava fechado | Novo estado remoto difere do baseline; nenhuma sobrescrita automática |
| INV-SC-05 | Baseline confirmado | Ambos mudam | Preservar fatos distintos para Analisador resolver conflito |
| INV-SC-06 | Arquivo não mudou | Nova varredura sem diferença | Não produzir atividades artificiais |
| INV-SC-07 | Pasta local inacessível | Varredura falha | Presença `Unknown` ou observação anterior preservada, nunca exclusão confirmada |
| INV-SC-08 | Drive inacessível | Erro remoto | Não limpar metadados nem concluir exclusão |
| INV-SC-09 | Página de changes incompleta | Provedor fornece mais páginas | Cursor/eventos não equivalem a inventário completo |
| INV-SC-10 | Arquivo renomeado | Mesmos IDs, nome novo | Mesmo `SyncItemId`, estado observado atualizado, baseline antigo preservado |
| INV-SC-11 | Arquivo movido | Mesmo ID remoto, pai novo | Preservar identidade e relacionamentos; sem presumir reupload |
| INV-SC-12 | Remoto removido com prova válida | `AbsentConfirmed` | Registrar ausência comprovada e preservar identificação/tombstone |
| INV-SC-13 | Documento Google nativo | Metadados sem checksum de binário | `None` legítimo; sem hash fabricado |
| INV-SC-14 | Nome Unix não UTF-8 | Observação local | Preservar bytes individuais sem conversão com perda |
| INV-SC-15 | Operação concluída | Confirmação válida | Baseline, status, fila e atividade coerentes num commit |
| INV-SC-16 | Conflito detectado por arquivo | Atividade de conflito | `activity.item_id` válido; nenhuma política escolhida pelo Inventário |

Esses cenários verificam a **representação e durabilidade** dos fatos. Não devem acionar upload/download real nem definir qual versão vence um conflito.

## 9. Erros públicos: cobertura obrigatória

| Família | Exemplos de gatilho | Resultado esperado |
| --- | --- | --- |
| `InvalidArgument` | Cursor/ID inválido, intervalo invertido, limites zero | Falha antes de gravar |
| `InvalidIdentity` | Inventário aberto com conta, `sync_id` ou raiz remota diferentes | Bloqueio sem alterar banco |
| `NotFound` | Escrita exige item/operação/evento inexistente | Erro, sem criar por acidente |
| `AlreadyExists` / `DuplicateAssociation` | ID remoto ou vínculo local duplicado | Unicidade preservada |
| `VersionMismatch` | Revisão do item ou geração esperada incorreta | Releitura necessária; sem escrita perdida |
| `InvalidTransition` | Claim de Completed, finalizar sem confirmar | Estado anterior intacto |
| `UnconfirmedAbsence` / `MissingObservation` | Deleção deduzida só por erro ou `Present` sem arquivo | Rejeição ou preservação de estado |
| `MissingActivity` | Conclusão de ação relevante sem atividade | Rollback |
| `InvalidCursor` | Cursor inválido/de outro filtro | Erro não destrutivo |
| `CorruptDatabase` / `UnsupportedSchema` | Banco não legível, versão incompatível | Sem apagar/recriar dados |
| `PermissionDenied` / `StorageUnavailable` / `StorageFull` | Acesso negado, pasta ausente, disco cheio | Erro tipado e sem falso sucesso |
| `TransactionFailed` | Falha de transação | Estado anterior preservado |
| `SerializationFailed` | Dado de entrada malformado não representável | Sem persistência parcial |

Em todos os erros, verificar **`kind`, mensagem sanitizada e `retryable`**. `retryable=true` não implica autorização para repetir operação destrutiva sem reavaliar precondições. Não expor SQL, credenciais nem dados privados em mensagens.

## 10. Lacunas a resolver antes de implementar casos dependentes

A especificação de testes é deliberadamente estrita sobre comportamento seguro, mas **não deve inventar decisões ausentes no contrato**. Registrar e fechar os seguintes pontos **no INTERFACES.md primeiro**, antes de escrever testes com números ou invariantes arbitrários:

| Tema | Por que falta definição | Teste que depende dela |
| --- | --- | --- |
| `LocalFile` | Tipos/campos e como reconstruí-los a partir do banco ainda em aberto | Round-trip completo de metadados locais |
| `RemoteFile` | Quais campos não retornados podem ser preservados/reconstruídos | Round-trip de projeções completas e parciais |
| `ObservationProof.complete_scope` | Comprovação de ausência requer evidência sobre geração/escopo | Testes positivos de exclusão confirmada |
| Geração da flag inicial | `MarkInitialDownloadCompleteRequest` possui `expected_complete` mas não valor-alvo explícito | Teste de transição idempotente e reversão |
| Paginação | Limite máximo, cursor opaco e ordenação sob mudanças concorrentes não fixados | Teste de fronteira numérica e snapshots |
| Deduplicação | Domínio e prazo de unicidade de `deduplication_key` | Colisão, reenvio e repetição de eventos |
| Finalização sem operação | Quando `ConfirmSyncRequest.activity` é exigido sem fila | Testes de registro histórico condicional |
| Operações em lote por item | Regras quando duas ações dependentes coexistem | Testes de ordering, collapse e replay |
| Retenção histórica | Tombstone e política de limpeza | Testes de preservação de FK em longo prazo |

**Critério:** esses casos permanecem **pendentes**, não serão considerados aprovados nem ignorados. A especificação já define a segurança mínima: sem exclusões presumidas, sem duplicações, sem falso sucesso e sem perda histórica.

## 11. Metas de cobertura e aceite

| Critério | Meta |
| --- | --- |
| Métodos públicos descritos por casos positivos/negativos | **20/20** |
| Objetos `Request` e `Result` | 100% dos campos com caso de presença/ausência/erro quando aplicável |
| Tipos/enums públicos | 100% das variantes/IDs usados no contrato |
| Tabelas e relacionamentos definidos | 100% das invariantes/FKs/UNIQUE relevantes |
| Escritas transacionais críticas | 100% com rollback e reabertura verificados |
| Classes de erro públicas | 100% com caso de mapeamento e sanitização |
| Cobertura de branches de implementação | Alvo inicial **>=90%**, sem substituir casos de risco por métrica |
| Proibição de rede/GUI/credenciais reais | 100% dos testes isolados |
| Regressão | Nenhuma regressão nos grupos já validados |

A porcentagem acima é **meta futura**, não resultado de execução. Não declarar `PASS` sem rodar testes e registrar comandos, ambiente, versão SQLite e Rust, número de casos e falhas restantes.

### Checklist de saída

- [ ] Os 16 métodos compilam com os tipos documentados.
- [ ] SQLite é criado somente no inventário de uma pasta, sem banco de histórico separado.
- [ ] Testes funcionais usam banco real isolado; falhas não deixam transações parciais.
- [ ] Os campos selecionados de `LocalFile` e `RemoteFile` suportam round-trip dos campos cujo mapeamento está definido.
- [ ] A ausência de dados opcionais não vira exclusão automática.
- [ ] Baseline só avança mediante confirmação válida; dados observados não o substituem.
- [ ] Operações e histórico persistem após reinício; checkpoints e eventos do Detector são testados no módulo Detector.
- [ ] Duas conexões concorrentes não quebram exclusividade/controle de revisão.
- [ ] Cada atividade tem FK `item_id` válida; tombstone preserva histórico.
- [ ] Nenhuma operação física é executada pelo Inventário.
- [ ] Erros são tipados e sanitizados, sem dados sensíveis.
- [ ] Pendências contratuais foram formalmente resolvidas ou marcadas como bloqueadoras.

## 12. Execução, documentação dos resultados e limites

Registrar no PR/commit de implementação:

- Ambiente Rust e SQLite em execução (Flatpak/SDK conforme `AGENTS.md`), versão e comando usado para testes.
- ID dos cenários executados; resultado, duração, falha e evidência de rollback/reabertura.
- Cobertura apurada, grupos ainda faltantes e limitações dos fakes.
- Diferença entre **teste isolado de componente** e **teste funcional real com conta Google**, que ocorrerá em etapa posterior.
- Cada teste que não puder ser automatizado deve ter justificativa técnica e procedimento manual explicitado.

**Estado deste documento:** plano de testes, não implementação nem resultado de execução. A execução começa somente quando os respectivos testes e código forem criados e efetivamente rodados.
