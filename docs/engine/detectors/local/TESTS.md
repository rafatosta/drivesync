# Detector Local — TESTS

## 1. Escopo

Plano TDD **exclusivo do Detector Local real** com filesystem temporário, watcher real quando aplicável, clock/scheduler controlados e **sink individual fake**. Os testes utilizam tipos de evento previstos no contrato e dependências de observação isoladas.

**Não** testar a persistência SQLite no Local: a garantia real de durabilidade pertence ao Storage. Aqui o sink simula aceitar ou rejeitar a gravação, e o Local deve reagir corretamente.

## 2. Fixtures

| Fixture | Conteúdo | Evidência |
| --- | --- | --- |
| `empty_root` | Raiz temporária válida, vazia | Leitura completa sem eventos de exclusão inventados |
| `local_tree` | Arquivos/subpastas com nomes UTF-8 e não UTF-8 Unix | `LocalFile.relative_path` preservado |
| `unknown_identity` | Arquivo sem identidade local estável comprovada | `LocalFile.local_id=None`; sem `SyncItemId` inventado |
| `unknown_metadata` | Permissões ou atributos indisponíveis | Campos `MetadataValue::NotCollected`/`Unavailable`; cobertura compatível |
| `closed_app_changes` | Criar/editar/mover durante stop | Reobservação somente após pedido explícito de varredura |
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
**Negativos/opcionais:** raiz reservada ou inválida; settings com debounce ausente; identidade vazia; limite de pendências zero; coletor composto com escopo incorreto. Não exigir consulta a Accounts.
**Retorno/invariantes:** sem `LocalFile` fabricado, sem evento, sem I/O remoto; nenhum tipo novo de identidade.
**Verificação:** `LocalDetectorCreateResult`, ausência de chamadas do sink, watcher e `collect`; consulta de escopo em memória permitida.

### LOC-02 — `start`

**Sucesso:** iniciar watcher e programar verificação de cobertura; não prometer reconstrução das mudanças do período parado.
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
| LOC-M-06 | Atributos não coletados ou indisponíveis | Estados explícitos de Models, sem zero fabricado; hash de conteúdo fora do contrato local |
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

Os tipos locais selecionados em Models já possuem implementação e testes puros; os testes do Detector continuam não implementados, e o envelope completo ainda depende de revisão remota. As decisões operacionais estão na seção 8 de INTERFACES. A escolha do watcher concreto não permite relaxar esses oráculos. A execução atual dos testes físicos está descrita na seção 16; os casos de controle/eventos continuam pendentes.

## 7. Regressão obrigatória — independência do esquema de LocalFile

| Caso | Cenário | Resultado esperado |
| --- | --- | --- |
| LOC-COL-01 | Coletor fake devolve LocalFile, independentemente de seus campos | Detector encaminha o objeto integral ao sink sem inspeção/projeção de atributos |
| LOC-COL-02 | Acrescentar campo a LocalFile e atualizar somente coletor/fixture | Código de watcher, scanner e notificação do Detector permanece **inalterado** |
| LOC-COL-03 | Remover campo de LocalFile e adaptar o coletor | Detector continua compilável/funcional, sem acesso ao campo removido |
| LOC-COL-04 | Coleta opcional de btime/flags indisponível | Coletor preserva o estado e a cobertura do perfil básico; Detector não interpreta o atributo |
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

## Controle de entrega e estado — testes adicionais

| ID | Cenário | Resultado |
| --- | --- | --- |
| LOC-S-01 | Pai suspendeu entrega ao Orquestrador | Local continua observando e emitindo pelo sink durável |
| LOC-S-02 | `status()` durante watcher e verificação | Atividade fiel (`Watching` ou `Scanning`), contadores da rodada sem bloquear |
| LOC-S-03 | Falha local com outras instâncias saudáveis | Erro da instância preservado sem interferir nas demais; agregação de Local/Remoto pertence aos testes do pai |
| LOC-S-04 | `stop()` durante observação | Cessa captura após conclusão segura, sem apagar eventos já aceitos |
| LOC-S-05 | `start()` após período parado | Retoma observação; Local não chama Scanner/Analisador nem decide reconciliação |

**Estado:** cenários documentais, ainda não executados.


## 9. Índice de rastreabilidade para a implementação

Além dos casos existentes, cada método deve satisfazer os seguintes grupos. Os IDs das seções anteriores permanecem válidos; os casos abaixo detalham seus oráculos, sem afirmar execução.

| Interface | Casos adicionais | Fonte |
| --- | --- | --- |
| `new` | LOC-D-01 a LOC-D-03 | INTERFACES §§ 3, 5, 8 |
| `start` | LOC-D-04 a LOC-D-06, LOC-W-01 a LOC-W-08 | INTERFACES §§ 5, 8 |
| `scan_once` | LOC-D-07 a LOC-D-10, LOC-P-01 a LOC-P-08 | INTERFACES §§ 5, 8 |
| `stop` | LOC-D-11 a LOC-D-13 | INTERFACES § 8 |
| `status` | LOC-D-14 a LOC-D-16 | INTERFACES §§ 3, 8 |
| `LocalMetadataCollector::scope` | LOC-A-02, LOC-A-08 | INTERFACES § 9 |
| `LocalMetadataCollector::collect` | LOC-A-01 a LOC-A-03, LOC-COL-01 a LOC-COL-06 | INTERFACES §§ 2, 9 |
| `LocalNotificationSink::notify` | LOC-A-04 a LOC-A-06, LOC-F-07 e LOC-F-08 | INTERFACES §§ 8, 9 |
| `LocalCancellation::is_cancelled` | LOC-A-07 | INTERFACES § 9 |

## 10. Casos de controle e concorrência

Para cada linha: preparar o estado descrito em filesystem temporário, exercer o método real e comparar resultado, retrato de status e chamadas capturadas. Usar barreiras/canais para concorrência; não usar sleeps como única prova de ordenação.

| ID | Contrato e cenário | Resultado e ausência de efeitos proibidos |
| --- | --- | --- |
| LOC-D-01 | `new` com configuração válida | `Stopped/Inactive`, zero contadores, últimos campos `None`; zero chamadas a watcher, `collect` e sink; nenhum arquivo criado |
| LOC-D-02 | `new` com raiz relativa/reservada, identidade vazia ou limite zero | `InvalidScope`; `collect`/watcher/sink não chamados. Não validar associação consultando Accounts |
| LOC-D-03 | `new` com debounce ausente/zero/positivo | Configuração aceita nos três casos; efeitos temporais só começam após start/pistas |
| LOC-D-04 | `start` em Stopped; bloquear verificação de cobertura | Retorno `Starting`, scan agendado; `status` acessível; liberar barreira leva a Running ou Degraded conforme cobertura |
| LOC-D-05 | `start` repetido, inclusive concorrente | Uma instalação por geração; chamada posterior retorna estado atual e `scheduled_scan=false`; Stopping/Failed rejeitados conforme contrato |
| LOC-D-06 | `start` falha ao instalar ou encontra raiz indisponível | Erro tipado, Failed e recursos parciais liberados; após stop uma nova geração pode iniciar; zero eventos de exclusão |
| LOC-D-07 | `scan_once` em Stopped, Running e Degraded | Rodada permitida; Stopped continua sem watcher; estado compatível ao concluir; request_id None/Some não muda FileId nem deduplica ocorrências |
| LOC-D-08 | `scan_once` em Starting/Stopping/Failed ou duas rodadas simultâneas | `InvalidState` ou `Busy`, respectivamente; nenhuma segunda coleta; pistas posteriores causam outra rodada |
| LOC-D-09 | `scan_once` com subárvore inacessível e outra acessível | Sucessos preservados, aviso de escopo inacessível e cobertura agregada Partial; nenhum descendente desconhecido declarado ausente |
| LOC-D-10 | `scan_once` interrompido por cancelamento/sink/raiz substituída | Erro com progress da rodada; entregas já aceitas contadas, rejeitadas não; coverage nunca Complete. Falha anterior ao início tem progress=None |
| LOC-D-11 | `stop` durante coleta e chamada ao sink bloqueadas | Cancelamento observável; stop aguarda os retornos; sucesso Stopped/in_flight=0; após retorno nenhuma chamada da geração anterior |
| LOC-D-12 | `stop` repetido, concorrente e após Failed | Encerramento único e idempotente; recursos liberados; sink aceito não é desfeito e fila externa não é tocada |
| LOC-D-13 | Callback antigo após stop/start | Callback não altera estado nem emite; geração nova conserva seu token e observações |
| LOC-D-14 | `status` enquanto worker aguarda I/O/sink | Retorna sem aguardar barreira do worker; nenhum I/O, coleta ou entrega provocado pela consulta |
| LOC-D-15 | `status` durante e depois de rodada com duas entradas | Contadores progressivos durante rodada e zero fora; últimos instante/cobertura preservados; rodada completa bem-sucedida limpa erro |
| LOC-D-16 | `status` após falha, cancelamento e reinício | Atividade/estado refletem condição real; sem caminhos absolutos, nomes pessoais ou mensagens brutas do SO nos diagnósticos |

## 11. Escopos, tipos locais e limites físicos

| ID | Contrato e fixture | Oráculo |
| --- | --- | --- |
| LOC-P-01 | Full em raiz vazia e árvore com arquivos/subpastas | Contagem exata das tentativas elegíveis; raiz não vira LocalFile artificial; nenhum evento de criação baseado apenas em primeiro scan |
| LOC-P-02 | Paths vazio, absoluto, NUL, componentes vazios, `.` e `..` | Rejeição integral antes de I/O; lista parcialmente inválida não observa os demais caminhos |
| LOC-P-03 | Paths com repetição, pai/filho sobrepostos, bytes não UTF-8 e nomes temporários comuns | Uma visita por entrada/rodada, bytes preservados; sem filtro antigo de temporários; diretório inclui descendentes |
| LOC-P-04 | `.drivesync` e descendentes em Full/Paths | Nenhuma coleta; solicitação só reservada retorna zero; pai elegível não é ocultado |
| LOC-P-05 | Symlink externo, FIFO, socket, hardlink e dispositivo quando disponível | Apenas metadados; nenhum acesso ao alvo/conteúdo, bloqueio em FIFO ou LocalFileId derivado de inode; casos sem suporte real são BLOCKED com motivo |
| LOC-P-06 | Troca de raiz/ancestral/diretório entre enumeração e leitura | Falha ou cobertura parcial/indisponível; sentinela externa intocada e não lida; nunca continuar na árvore substituta |
| LOC-P-07 | Item desaparece durante coleta | RecheckRequired sem metadados fabricados; cobertura inconclusiva, sem confirmação de exclusão |
| LOC-P-08 | Permissão real negada, atributo opcional não suportado e grupo obrigatório não coletado | Estados distintos no coletor; Partial quando falta grupo obrigatório; não declarar negação apenas por bits de modo |

## 12. Watcher, tempo e entrega

| ID | Contrato e cenário | Oráculo |
| --- | --- | --- |
| LOC-W-01 | Instalação em árvore existente; criar/mover subpasta durante instalação | Janela incerta é verificada; novos descendentes observados ou cobertura declarada Degraded, nunca cobertura completa inventada |
| LOC-W-02 | Criar, editar, alterar metadados, remover, renomear e mover arquivos reais | Evidência suficiente produz tipo específico; correlação insuficiente produz RecheckRequired. Aceitar sequências equivalentes do SO, não exigir número exato de callbacks |
| LOC-W-03 | Debounce ausente/zero e positivo com relógio controlado | Sem espera intencional nos dois primeiros; janela positiva vence desde primeira pista e não é adiada por rajada contínua; avanço/recuo UTC não muda prazo |
| LOC-W-04 | Limite de caminhos excedido, overflow e falha do backend | Memória de pistas limitada; necessidade de reavaliar raiz, Degraded e diagnóstico; nenhuma exclusão nem promessa de recuperar histórico |
| LOC-W-05 | Alteração durante scan e no instante de conclusão | Ao menos uma observação posterior da mudança; sem dois workers nem perda na troca de rodada |
| LOC-W-06 | Salvamento por arquivo temporário seguido de substituição/rename | Caminho final reobservado; não presumir inode anterior nem ocultar mudança por ocorrer durante atividade interna |
| LOC-W-07 | Alterações durante stop e start posterior | Start restabelece/verifica captura, sem Scanner/Inventário/Analisador. Pedido explícito de scan reobserva estado atual; não reconstitui operações intermediárias |
| LOC-W-08 | Duas instâncias com mesmos caminhos relativos | Conta/raiz/sync_id e cancelamento isolados; falha/stop de uma não interrompe outra |

### Portas e falhas de aceite

| ID | Contrato e cenário | Oráculo |
| --- | --- | --- |
| LOC-A-01 | `collect` devolve objeto com grupos opcionais distintos | Mesmo objeto integral no File; lógica do detector não consulta ou remonta campos; cobertura vem do resultado do coletor |
| LOC-A-02 | Coletor vinculado ao escopo errado na composição | PreconditionFailed antes de observar; teste da composição privada demonstra validação, sem acesso a Accounts |
| LOC-A-03 | Cancelar antes e entre operações de coleta | Cancelled; nenhuma leitura seguinte iniciada; objeto parcial não é apresentado como completo |
| LOC-A-04 | Sink aceita dois eventos, rejeita o terceiro | emitted_count/progress=2; terceira ocorrência mantida integral para retry; nenhum falso aceite |
| LOC-A-05 | Sink grava, mas simula erro no retorno; novo scan solicita retry | Mesmo FileId, observed_at e payload na repetição; somente retorno Ok conta entrega; não exigir exatamente uma chamada nem inventar garantia exactly-once |
| LOC-A-06 | Rejeição retryable/não retryable e stop com pendência | Degraded/Failed conforme contrato; sem retry infinito ou fila ilimitada; após parada não prometer replay da pendência volátil nem apagar aceites anteriores |
| LOC-A-08 | `scope` chamado repetidamente e durante coleta | Mesmo escopo configurado, devolvido sem I/O ou espera pela coleta; divergência provoca PreconditionFailed em new |
| LOC-A-07 | `is_cancelled` antes/depois do cancelamento em clones e nova geração | false inicial, true irreversível em todos os clones antigos; novo token continua false; consulta sem I/O |

## 13. Método de execução e evidências futuras

1. Implementar casos por contrato em RED → GREEN → REFACTOR, começando pelos tipos compartilhados aprovados e portas; seguir com percurso/coleta e ciclo de vida/watcher.
2. Usar filesystem temporário real para regras de acesso; fake somente para dependências e injeção de falhas/tempo. A biblioteca do watcher não precisa produzir todos os tipos de evento: o normalizador deve preservar incerteza.
3. Testes de permissões devem comprovar negação pelo kernel. Se o ambiente ignorar a restrição, registrar BLOCKED/skip justificado; um mock não comprova proteção física.
4. Testes reais do watcher aguardam condições com timeout limitado e sincronização de prontidão, sem assumir ordenação ou cardinalidade de callbacks. Regras de debounce e corrida usam relógio controlado/barreiras, sem depender de execução rápida.
5. Rodar os comandos Rust exigidos pelo AGENTS da raiz exclusivamente no Flatpak do manifesto; registrar comando, resultado e casos bloqueados. Não usar dados pessoais ou credenciais.
6. Revisar alterações de esquema em checkout/fixture temporário para LOC-COL-02/03; não alterar Models de produção apenas para provar independência. Enquanto os tipos não estiverem implementados, registrar os casos executáveis como pendentes, sem alegar PASS.
7. Comprovar ausência de mutações comparando conteúdo e estrutura das sentinelas antes/depois, considerando que leituras podem atualizar atime. Não usar o próprio normalizador para fabricar o resultado esperado.

**Aceite desta etapa documental:** cinco métodos de controle e quatro métodos auxiliares rastreados; decisões de ciclo de vida, escopo e falhas explicitadas; dependências de Models identificadas. **Somente os testes executáveis do coletor/cancelamento foram implementados nesta etapa; os métodos de controle e entrega continuam pendentes.** A durabilidade real do sink, leases e agregação Local/Remoto são validações futuras dos componentes proprietários.


## 14. Alinhamento com a seleção local de Models

| Caso | Fixture | Oráculo |
| --- | --- | --- |
| LOC-MOD-01 | Coletor devolve perfil básico completo e btime/flags opcionais indisponíveis | Complete permitido; File integral, sem o detector inspecionar atributos |
| LOC-MOD-02 | Coletor devolve campo básico não Known | Partial e estado de coleta preservados; não completar com zero |
| LOC-MOD-03 | Entrada não-symlink com alvo NotApplicable; link com alvo literal externo | Ambos preservados conforme Models; não seguir alvo nem degradar apenas por NotApplicable válido |
| LOC-MOD-04 | Aviso sobre raiz inacessível | WholeSync, ScopeUnavailable, Unavailable, data=None e localizações ausentes; nenhum path vazio |
| LOC-MOD-05 | Metadados de ACL/xattr/Direct I/O não fazem parte do esquema selecionado | Coletor básico não exige consultas extras; nenhuma mudança nas portas ou lógica do detector |

Estes casos complementam o perfil selecionado em Models, não implementam seus validadores. Leitura física continua testada no coletor; validação lexical e combinações puras ficam na suíte Models.


## 15. Matriz do coletor físico isolado

| ID | Operação/cenário | Evidência de aceite |
| --- | --- | --- |
| LOC-FS-01 | new/scope, duas raízes | Scope exato sem I/O na consulta; conta/sync/raiz não se cruzam |
| LOC-FS-02 | new com configuração inválida ou raiz ausente/simbólica | Erro tipado; nenhuma criação ou normalização com perda |
| LOC-FS-03 | Arquivo regular e diretório reais | Kind, caminho, modo, owner, tamanho, timestamps, inode/device/nlink verificáveis; local_id=None |
| LOC-FS-04 | Nome/alvo não UTF-8 e symlink externo/quebrado | Bytes preservados; tipo Symlink e alvo literal, sem observar destino |
| LOC-FS-05 | FIFO/socket/hardlink | Só metadados sem bloqueio; nlink observado não gera identidade |
| LOC-FS-06 | Arquivo sem leitura e diretório sem busca | O primeiro ainda permite metadata por O_PATH; o segundo produz PermissionDenied comprovado pelo kernel |
| LOC-FS-07 | Entrada ausente e pasta interna reservada | Ausente=None/Partial; reservada=InvalidScope; sem tratar falha como ausência confirmada |
| LOC-FS-08 | Cancelamento antes/depois de abrir entrada | Cancelled e nenhuma observação publicada; clone cancelado e nova geração independente |
| LOC-FS-09 | Trocar raiz/ancestral absoluto | RootUnavailable; coletor não adota silenciosamente nova árvore |
| LOC-FS-10 | Trocar ancestral relativo por symlink ou mover para fora | PartialObservation ou None/Partial, sem metadados externos ou leitura de conteúdo |
| LOC-FS-11 | Substituir alvo/editar durante coleta | None/Partial, sem fotografia completa inventada |
| LOC-FS-12 | Alvo maior que buffer inicial | Bytes integrais, sem truncamento considerado Known |
| LOC-FS-13 | Chamadas concorrentes ao mesmo coletor | Resultados de requisições independentes, sem estado de evento ou banco |
| LOC-FS-14 | statx com máscaras parciais ou sem suporte | Respeitar bits; básico fallback real, btime/flags com estados explícitos |
| LOC-FS-15 | Erro/limite na leitura literal | Campo de alvo mantém estado de falha e Partial; cancelamento é propagado |

Usar fixtures reais temporárias com RAII e testes de corrida por barreiras/hooks nos pontos de abertura e observação, sem sleeps. Injeção de falha deve afetar somente a fronteira de syscall necessária (por exemplo, statx), não substituir o coletor ou a árvore física. Os casos são planejados; registrar separadamente os resultados efetivamente executados nas unidades abaixo.


## 16. Evidência incremental do coletor

Unidade básica: 16 testes físicos em `engine/src/detectors/local/tests.rs`. Cobrem LOC-FS-01 a 13, incluindo new/scope/collect, cancelamento compartilhado, alvo longo/não UTF-8, diretório/FIFO/socket/hardlink, permissão realmente negada ao usuário não privilegiado e corridas determinísticas antes/depois da observação. LOC-FS-14/15 ficam para o enriquecimento e injeção de falhas nas syscalls.

RED executado antes dos módulos do coletor existirem. GREEN: `cargo test --offline --locked -p drivesync-engine --lib detectors::local::` com 16 PASS no org.gnome.Sdk//50, rust-stable, CARGO_HOME=/home/tosta/.cargo exposto ao SDK. A primeira execução do Clippy encontrou literais de permissão sem base octal nos testes; foram corrigidos para 0o0, sem alteração do contrato.

Nenhum desses testes implementa File, sink, start/stop/status do Detector, scan_once ou watcher. Os testes de scoped identity incorreta no futuro LocalDetector::new continuam pendentes: scope do coletor é apenas a vinculação original, não consulta Accounts.


Unidade de enriquecimento: 25 testes totais do coletor, incluindo LOC-FS-14/15. Validam statx real, remoção de cada bit básico, ausência opcional de btime, erros ENOSYS/EOPNOTSUPP/EPERM/EIO, fallback fstat real, fração Known inválida, falha/truncamento de readlink e cancelamento entre queries. O teste do limite de 64 KiB simula somente o tamanho retornado pela syscall, sem alegar suporte do filesystem a links desse tamanho.

Os testes de permissões foram executados com UID não privilegiado e comprovaram EACCES pelo kernel, sem inferir apenas por bits. FIFO e socket reais foram observados sem abrir conteúdo. Não foi criado device node (exige capacidades não necessárias ao teste); tipos de device permanecem cobertos no modelo puro, sem reivindicar teste físico desses nós.

O coletor já possui acesso público pelo módulo pai e testes de consumidor externo, mas ainda não é chamado automaticamente pela aplicação: a composição com Detector, sincronização real e UI não foi implementada. Os métodos new/scope/collect do coletor não comprovam new/start/scan_once/stop/status da fachada.


Validação final no mesmo ambiente Flatpak/cache, em 2026-11-13:

- `cargo fmt --all --check`: PASS.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --offline --locked --workspace`: 561 testes PASS e 1 doctest PASS; 6 testes ignorados (desktop/portal e Secret Service isolado).
- `cargo doc --offline --locked --workspace --no-deps --document-private-items`: PASS, sem warnings.
- `cargo test --offline --locked --workspace --doc`: 1 doctest PASS.

Não foram exercitados Google real, aplicativo instalado, portal, watcher, varredura recursiva, fila ou entrega de File. A cobertura atual é do coletor e dos componentes já existentes; nenhum método de controle do Detector é declarado implementado por esses resultados.
