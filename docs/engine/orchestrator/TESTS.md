# Orchestrator — TESTS (contratos internos)

## Escopo

Plano **não executado** de integração por contratos dos módulos. Usar implementações concretas do Orchestrator quando existirem, dependências fakes com rastros de chamadas e SQLite real temporário para fronteiras duráveis. Separar cenários verificáveis dos **BLOCKED** por ausência de API nos outros módulos.

## Matriz de integração

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| ORC-01 | Abrir contexto de `SyncId` e conta corretos | Portas apontam para escopo correto; conta/sync divergentes rejeitados |
| ORC-02 | Iniciar sincronização | `Detector.start(delivery=Paused)` **antes** de `Scanner.scan`; duas origens iniciadas pelo Detector |
| ORC-03 | Scanner local com arquivos existentes | Scanner registra observações via Inventory; Orchestrator não duplica as gravações |
| ORC-04 | Evento durante Scanner | Detector persiste mesmo com entrega pausada; evento fica disponível após `resume` |
| ORC-05 | Scanner Partial / falha estrutural | Não marcar cobertura completa nem inferir exclusões; status/erro preservado |
| ORC-06 | Remoto não concluiu observação inicial | Não declarar sincronização inteira coberta só por Scanner local |
| ORC-07 | `take_events` entrega lote misto | Orquestrador registra cada `File` integral em `record_local` ou `record_remote` |
| ORC-08 | Evento sem identidade inequívoca | Não escolher `SyncItem` por nome; reobservar ou encaminhar indeterminação segura |
| ORC-09 | `Deleted`, `data=None` ou escopo incompleto | Não produzir `AbsentConfirmed` automaticamente |
| ORC-10 | Todos eventos persistidos e item afetado retornado | Somente então `acknowledge(Processed)`; análise recebe `SyncItem`, não `FileBatch` |
| ORC-11 | Falha do Inventory no meio do lote | Sem ack global; `release_events`/lease preserva possibilidade de reentrega |
| ORC-12 | Crash entre `record_*` e `acknowledge` | Redelivery exige deduplicação persistente por `File.id`; **BLOCKED até contrato no Inventory** |
| ORC-12A | Crash após ack do Detector, antes de `analyze` | Inventory precisa manter trabalho pendente recuperável; **BLOCKED até contrato de marcação durável** |
| ORC-13 | Mesmo item aparece várias vezes em lote | Preservar eventos; planejar reanálise sem perder mudanças; não coalescer observações persistidas |
| ORC-14 | Analyzer devolve `Completed` | Não repetir Operator nem confirmar baseline no Orchestrator |
| ORC-15 | Analyzer devolve `Pending` | Não bloquear outros itens, não acionar Operator; persistência da pendência **BLOCKED** |
| ORC-16 | Resolução externa válida | Chamar novamente Analyzer com `Resolution`, não interpretar a opção; recuperação durável **BLOCKED** |
| ORC-17 | Resolução com revision/fingerprint obsoletos | Não executar decisão antiga; Analyzer valida e devolve pendência ou erro |
| ORC-18 | `Analyzer::Failed` após efeito remoto incerto | Não retry cego; preservar falha e solicitar reobservação |
| ORC-19 | `Detector.pause` | Observação continua, claims suspensos; Orchestrator sem segunda fila |
| ORC-20 | `Detector.stop` e posterior início | Observação interrompida; exigir verificação da lacuna sem culpar Detector por política externa |
| ORC-21 | Uma origem do Detector falha | Status agregado degradado com causa da origem; outra mantém atividade |
| ORC-22 | Restoration/Rebuilt do Inventory ou Detector Store | Não converter perda de histórico em ausência; bloqueio de mutações perigosas até cobertura |
| ORC-23 | Configuration não disponível | Defaults dos módulos mantêm execução segura |
| ORC-24 | Consulta de status durante scan/analyze lento | Não esperar fim do trabalho; preservar escopo, atividade e erros |
| ORC-25 | Duas sincronizações/contas distintas | Sem mistura de AccountId/SyncId/File.id, fila, inventory ou resolução |
| ORC-26 | Chamada `Detector.request_scan` | Observação extra não é tratada como reconstrução por Scanner |
| ORC-27 | Parada durante lote leased | Não confirmar eventos implicitamente; preservar possibilidade de release/redelivery |
| ORC-28 | Fila Inventory `Running` após crash | Não reexecutar comando por conta própria; reavaliação de plano **BLOCKED** até contrato |

## Testes de fronteiras

Verificar com rastros que **não há chamadas diretas** do Orchestrator a Operator, Providers, Storage interno, Detector Local/Remote, algoritmos Reconciliation/Conflicts ou SQL. O único caminho de execução física é Orchestrator → Analyzer → Operator → Provider/filesystem. Scanner usa a própria API de Inventory para gravar observações locais.

Não aceitar um mock do Detector que sempre retorna `Ok` como comprovação de fila durável: para ORC-04, ORC-11 e ORC-12 é necessário testar integração com armazenamento real isolado (quando implementado).

## Pendências contratuais de aceitação

| Contrato ainda faltante | Impacto |
| --- | --- |
| `Inventory` — marcar/listar análise pendente de maneira durável ao ingerir observações | Recuperação de itens após ack do Detector e crash antes do Analyzer |
| `Inventory` — idempotência por `File.id`/identidade de ocorrência | Reentrega de lote não produz efeitos duplicados de observação |
| `Inventory` — persistência/consulta de `PendingAction` e `Resolution` | Recuperar conflito após reinício |
| `Inventory/Scanner` — cobertura, geração e diagnóstico persistente | Decidir segurança da observação inicial |
| `Detector Remoto` — evidência de cobertura remota inicial | Não confundir polling com snapshot completo |
| Proprietário de conexões — cadastro e escopos | Preparação de contextos por conta/sync |
| `Inventory/Analyzer` — plano de comando durável completo/recovery | Tratar operações em resultado incerto ou Running |

Registrar `BLOCKED` para esses casos até contratos efetivos, sem declarar arquitetura fechada. Nunca alterar API proprietária de outro módulo para fazer teste passar sem decisão explícita.

**Estado:** plano TDD, nenhum teste Rust executado.
