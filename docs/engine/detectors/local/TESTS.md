# Detector Local — TESTS

## 1. Escopo

Plano TDD **exclusivo do Detector Local real** com filesystem temporário, watcher real quando aplicável, clock/scheduler controlados e **sink individual fake**. O Local importa os tipos de [Models](../../models/INTERFACES.md), mas não precisa de implementação real do Detector pai, Remoto, Storage, Inventário, Analisador ou Google API.

**Não** testar a persistência SQLite no Local: a garantia real de durabilidade pertence ao Storage. Aqui o sink simula aceitar ou rejeitar a gravação, e o Local deve reagir corretamente.

## 2. Fixtures

| Fixture | Conteúdo | Evidência |
| --- | --- | --- |
| `empty_root` | Raiz temporária válida, vazia | Leitura completa sem eventos de exclusão inventados |
| `local_tree` | Arquivos/subpastas com nomes UTF-8 e não UTF-8 Unix | `LocalFile.relative_path` preservado |
| `unknown_identity` | Arquivo sem identidade local estável comprovada | `LocalFile.local_id=None`; sem `SyncItemId` inventado |
| `unknown_metadata` | Permissões ou atributos indisponíveis | Campos `MetadataValue::NotCollected`/`Unavailable`; cobertura compatível |
| `closed_app_changes` | Criar/editar/mover durante stop | Recuperação por nova varredura |
| `partial_tree` | Subárvore inacessível | Aviso de reavaliação sem `LocalFile` e cobertura Partial |
| `removed_candidate` | Arquivo desaparece ou evento de remoção chega | `File.data=None`, sem exclusão confirmada |
| `symlink_external` | Link para fora da raiz | Nenhuma leitura externa |
| `reserved_directory` | `.drivesync/` alterado | Ignorar dados internos |
| `concurrent_edits` | Mudança entre watcher e scan | Não declarar leitura completa sem evidência |
| `rejecting_sink` | Sink retorna falha após observação | Sem falso aceite, replay/revarredura |
| `separate_roots` | Dois escopos isolados | `sync_id`, caminhos e eventos não se cruzam |

## 3. Contratos dos cinco métodos

### LOC-01 — `new`

**Sucesso:** criar instância `Stopped` com raiz, conta, identidade e sink fake.
**Negativos/opcionais:** raiz reservada ou inválida; settings com debounce ausente; conta de sincronização incorreta.
**Retorno/invariantes:** sem `LocalFile` fabricado, sem evento, sem I/O remoto; nenhum tipo novo de identidade.
**Verificação:** `LocalDetectorCreateResult`, ausência de chamadas do sink e watcher.

### LOC-02 — `start`

**Sucesso:** iniciar watcher e programar inspeção inicial/reinício; mudanças anteriores ao início serão reobservadas.
**Negativos/opcionais:** start repetido; falha do watcher; raiz desmontada.
**Retorno/invariantes:** estado consistente e cobertura não inventada.
**Verificação:** scheduler, watcher e avisos individuais produzidos depois da observação.

### LOC-03 — `scan_once`

**Sucesso:** produzir `File` com `File.data=Some(FileData::Local(LocalFile))` para metadados verificados; preservar tipos, caminhos e opcionais de Models.
**Negativos/opcionais:** Full/Paths; Paths vazio ou fora da raiz; leitura parcial; symlink; request_id None/Some; sink rejeita.
**Retorno/invariantes:** contagem observada distinta de avisos **aceitos**; sem lista paralela de eventos; sem baseline.
**Verificação:** capturas do sink com valores de `LocalFile` concretos e cobertura apropriada.

### LOC-04 — `stop`

**Sucesso:** parar observação e tarefas.
**Negativos/opcionais:** stop repetido; interrupção durante varredura.
**Retorno/invariantes:** não emitir sucesso de entrega para sinais pendentes; não limpar fila ou estado externo.
**Verificação:** ausência de novas observações depois de stop efetivo, retorno tipado.

### LOC-05 — `status`

**Sucesso:** consulta de estado, última cobertura e erro.
**Negativos/opcionais:** antes de observar = campos opcionais None; falhas sanitizadas.
**Retorno/invariantes:** leitura não inspeciona disco, não chama sink nem banco.
**Verificação:** status antes/depois das operações.

## 4. Integração com Models — casos obrigatórios

| Caso | Situação | Comportamento obrigatório |
| --- | --- | --- |
| LOC-M-01 | Arquivo regular | Aviso contém `Some(LocalFile)` com `kind=Known(LocalEntryKind::Regular)` |
| LOC-M-02 | Diretório | Aviso contém `Known(LocalEntryKind::Directory)`; sem conteúdo fabricado |
| LOC-M-03 | Symlink | Modelo `Symlink`, sem seguir alvo externo |
| LOC-M-04 | Nome Unix não UTF-8 | Caminho preserva componentes sem conversão com perda |
| LOC-M-05 | Identidade local não comprovada | `local_id=None`; sem gerar `SyncItemId` |
| LOC-M-06 | Hash/tamanho não observados | metadados `NotCollected` ou `Unavailable`, conforme a coleta conforme observação; sem preencher por suposição |
| LOC-M-07 | Evento de remoção | Aviso com `File.data=None` + escopo quando possível |
| LOC-M-08 | Watcher overflow | Recheck com cobertura degradada e sem `LocalFile` obrigatório |
| LOC-M-09 | Sink aceita | Um aviso individual por chamada, contado como entregue |
| LOC-M-10 | Sink rejeita | Falha propagada; não contar como evento persistido |
| LOC-M-11 | Mesmo arquivo em duas alterações | Não consolidar destrutivamente observações diferentes |
| LOC-M-12 | Duas raízes com mesmo caminho relativo | Escopos distintos preservam `sync_id` |
| LOC-M-13 | `.drivesync/` | Não gerar `LocalFile` sincronizável |
| LOC-M-14 | Leitura parcial | Não converter `None` em deleção nem `Complete` |
| LOC-M-15 | Componentes irmãos ausentes | Local roda com Models e sink fake; não exige Remoto/Storage reais |

## 5. Segurança, cobertura e recuperação

Testar raiz removida/desmontada, permissões, overflow do watcher, mudanças durante scan, symlink externo e paralelismo de sessões. Evento isolado não é confirmação de exclusão; após app fechado, nova varredura deve apontar necessidade de reavaliação sem alegar reconstrução do histórico.

Cenário de falha entre observação e aceite durável deve preservar possibilidade de repetição. Não basta comparar structs de Models: exercitar o **observador real** com diretório temporário.

## 6. Critério de aceite

**5/5 métodos internos** e cenários de Models aplicáveis passam independentemente, com resultados PASS/FAIL/BLOCKED e evidências de comandos executados no Flatpak. **Não** exigir testes Google/Inventory/Storage para concluir o Local.

Pendências ainda bloqueadoras de casos particulares: implementação de `LocalRelativePath`, ownership único de IDs/Timestamp, watcher concreto e regras de cobertura que determinem identidade. Nenhum teste foi executado nesta atualização documental.

## 7. Regressão obrigatória — independência do esquema de LocalFile

| Caso | Cenário | Resultado esperado |
| --- | --- | --- |
| LOC-COL-01 | Coletor fake devolve LocalFile, independentemente de seus campos | Detector encaminha o objeto integral ao sink sem inspeção/projeção de atributos |
| LOC-COL-02 | Acrescentar campo a LocalFile e atualizar somente coletor/fixture | Código de watcher, scanner e notificação do Detector permanece **inalterado** |
| LOC-COL-03 | Remover campo de LocalFile e adaptar o coletor | Detector continua compilável/funcional, sem acesso ao campo removido |
| LOC-COL-04 | Coleta auxiliar de ACL/xattr falha | Coletor indica indisponibilidade sem impedir evento de reavaliação; Detector não interpreta o atributo |
| LOC-COL-05 | Coleta parcial por custo/limite | Coletor preserva estados de coleta; Detector repassa o LocalFile sem atribuir valores padrões |
| LOC-COL-06 | Coletor devolve file=None | Detector emite reavaliação segundo cobertura, sem inventar exclusão |

Testes do coletor exercitam syscalls e funções auxiliares em filesystem temporário. **Teste de arquitetura:** alterações no layout de `LocalFile` devem ser demonstradas sem alterações nos arquivos-fonte do Detector (exceto no componente coletor/adaptador e seus testes). Este é um critério de aceite, não somente uma recomendação.

## 8. Testes de encapsulamento de eventos

| ID | Cenário | Resultado |
| --- | --- | --- |
| LOC-F-01 | Arquivo criado | `FileEventKind::Created`, `File.data=Some(FileData::Local(...))`, `current=Some(Local(...))` |
| LOC-F-02 | Edição/metadata | `Modified` ou `MetadataChanged` somente com evidência; metadados atuais íntegros |
| LOC-F-03 | Renomeação/movimentação | `previous/current` corretos quando identidade correlacionada; senão `RecheckRequired` |
| LOC-F-04 | Remoção por watcher | `Deleted` como indício ou `RecheckRequired`, nunca operação de exclusão |
| LOC-F-05 | Escopo inacessível | `ScopeUnavailable`, `coverage=Unavailable`, `data=None` |
| LOC-F-06 | Coletor fake acrescenta campo | Detector continua inalterado e repassa `LocalFile` sem perda |
| LOC-F-07 | Sink fake recusa `File` | Falha propagada; nenhuma entrega considerada durável |
| LOC-F-08 | Retry após falha | Mesma ocorrência pode ser reentregue com ID estável, sem invenção de metadados |
