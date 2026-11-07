# Detectores — TESTS

## 1. Objetivo e escopo

Plano **Contract-First + TDD** do componente [Detectores](README.md), fundamentado nos [contratos públicos](INTERFACES.md). Abrange os **10 métodos** de `LocalDetector` e `RemoteDetector`, o contrato `DetectorNotificationSink`, campos opcionais, falhas, monitoramento contínuo e recuperação.

A validação é **isolada**: filesystem real sob diretórios temporários, Inventário com SQLite real isolado, `google_api/` simulada por provider fake configurável, Orquestrador representado por sink registrador de notificações. Nenhum OAuth real, conta Google, pasta pessoal ou GUI.

Um fake de `LocalDetector`/`RemoteDetector` não valida o módulo; simular **dependências**, não a própria unidade sob teste. Testes de integração real com Drive e Flatpak instalado são uma etapa posterior e não podem ser declarados aprovados por esta suíte.

## 2. Estratégia TDD e isolamento

1. Criar testes de compilação das assinaturas públicas e enums/requests/results.
2. **RED:** escrever casos mínimos de sucesso, falha e opcionais, com expectativas independentes da implementação.
3. **GREEN:** implementar somente o comportamento necessário ao grupo.
4. **REFACTOR:** reorganizar código mantendo API e regressão.
5. Validar grupos: ciclo de vida; varredura local; mudanças incrementais remotas; notificações; reinício/concorrência/erros.
6. Ao fim, executar regressão integral no ambiente Flatpak definido em `AGENTS.md` da raiz e registrar evidências reais.

### Fixtures

| Fixture | Composição | Propósito |
| --- | --- | --- |
| `local_clean_root` | Raiz temporária vazia + inventário com identidade conhecida | Abertura e varredura inicial |
| `local_tree` | Arquivos, subpastas, nomes Unicode/não UTF-8 e `.drivesync/` | Caminhos, exclusões e metadados |
| `local_offline_changes` | Arquivos modificados enquanto watcher parado | Retomada sem eventos em tempo real |
| `local_fault` | Subpasta sem permissão, raiz inacessível, symlink externo | Partial/Unavailable e segurança |
| `remote_tree` | Provider fake com páginas de arquivos e mudanças simultâneas | Inventário inicial |
| `remote_changes` | Eventos paginados, cursor atual, eventos duplicados e mudanças de acesso | Detecção incremental |
| `remote_expired_cursor` | Provider fake responde ExpiredCursor | Recovery, nunca ausência confirmada |
| `inventory_real` | SQLite temporário de uma sincronização, consultado por API | Persistência/cursor/baseline |
| `sink_fake` | Coletor de notificações com repetição e falhas injetadas | Entrega e perdas |
| `clock_and_scheduler_fake` | Relógio e agendamento determinísticos | Debounce, retries, start/stop |

Reproduzir erros do provider fake com respostas tipadas da Google API; não copiar fixtures da implementação sob teste. Para observações locais, utilizar a forma de `LocalFile` definida por seu futuro contrato público.

## 3. Índice obrigatório dos métodos

| Caso | Método | Condição central |
| --- | --- | --- |
| DET-01 | `LocalDetector::new` | Instanciação local |
| DET-02 | `LocalDetector::start` | Início local |
| DET-03 | `LocalDetector::scan_once` | Varredura local |
| DET-04 | `LocalDetector::stop` | Parada local |
| DET-05 | `LocalDetector::status` | Estado local |
| DET-06 | `RemoteDetector::new` | Instanciação remota |
| DET-07 | `RemoteDetector::start` | Início remoto |
| DET-08 | `RemoteDetector::poll_once` | Leitura remota |
| DET-09 | `RemoteDetector::stop` | Parada remota |
| DET-10 | `RemoteDetector::status` | Estado remoto |

## 4. Casos de teste por método

### DET-01 — `LocalDetector::new`

**Sucesso:** Escopo local válido, raiz temporária e dependências fake/inventory real: retorna Stopped sem watcher iniciado.

**Negativos e opcionais:** `local_root=None`, identidade incompatível com inventário e `.drivesync` como raiz rejeitados; settings sem permissão para incluir diretório reservado.

**Retorno e invariantes:** Não cria banco de outras pastas nem altera filesystem do usuário.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-02 — `LocalDetector::start`

**Sucesso:** Watcher configurado com controle de janela inicial, varredura de retomada programada; retorna estado coerente.

**Negativos e opcionais:** Watcher falha, root some ou start repetido; status Degraded/erro tipado; não reportar varredura inexistente.

**Retorno e invariantes:** Sem falsos eventos de exclusão; eventos concorrentes não desaparecem.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-03 — `LocalDetector::scan_once`

**Sucesso:** Varredura Full descobre criação/edição/renomeação/movimento; Paths reavalia alvo; registrar observação via Inventário real.

**Negativos e opcionais:** Subárvore ilegível, varredura cancelada, arquivo removido durante leitura, symlink externo, nomes não UTF-8; request_id opcional.

**Retorno e invariantes:** Partial nunca marca absent em escopo não coberto; observação não altera baseline, filas ou atividade.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-04 — `LocalDetector::stop`

**Sucesso:** Stop libera watcher e faz estado Stopped, preservando observações previamente confirmadas.

**Negativos e opcionais:** Stop duplo, stop durante varredura, cancelamento; responder com transição válida, sem status falso.

**Retorno e invariantes:** Não apaga dados; sem notificações novas após stop concluído, salvo as já publicadas antes.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-05 — `LocalDetector::status`

**Sucesso:** Retorna estado, últimas observações, cobertura e erro registrados; não dispara scan.

**Negativos e opcionais:** Status antes do primeiro start tem timestamps opcionais ausentes; após erro, último erro sanitizado.

**Retorno e invariantes:** Consulta pura sem I/O observacional nem alteração de inventário.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-06 — `RemoteDetector::new`

**Sucesso:** Sessão fake da conta certa e raiz remota conhecida; constrói sem HTTP e sem inventário novo.

**Negativos e opcionais:** Session de outra conta, remote_root_id ausente e escopo divergente; nenhuma tentativa de autenticação.

**Retorno e invariantes:** Não expõe token; apenas contratos públicos de Google API/Inventário.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-07 — `RemoteDetector::start`

**Sucesso:** Estado de inicialização e trabalho de primeira leitura/retomada programado conforme cursor existente.

**Negativos e opcionais:** Sem acesso, falha em consultar cursor, stop durante início; não fingir Complete.

**Retorno e invariantes:** Sem exigir push; retomada incremental não perde mudanças desde cursor confirmado.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-08 — `RemoteDetector::poll_once`

**Sucesso:** InitialInventory percorre files.list paginado, obtém cursor e reconcilia alterações; Incremental percorre changes.list e persiste cursor/eventos por página.

**Negativos e opcionais:** 403/404/timeout, projeção parcial, more-pages, cursor expirado, removed por perda de acesso, page_budget finito; sem confirmação de ausência indevida.

**Retorno e invariantes:** Checkpoint e eventos atômicos no Inventory; has_more=true implica não Complete; baseline intacto, eventos pendentes.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-09 — `RemoteDetector::stop`

**Sucesso:** Encerra consultas e cancelamento, preserva checkpoint confirmado.

**Negativos e opcionais:** Parar em paginação com página parcialmente processada; falha de parada; estado coerente.

**Retorno e invariantes:** Sem avançar cursor por página não confirmada; não revoga sessão nem exclui conteúdo remoto.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.


### DET-10 — `RemoteDetector::status`

**Sucesso:** Recupera estado, última cobertura e erro, sem comunicação HTTP.

**Negativos e opcionais:** Antes de observação, Option None legítimo; após cursor expirado, estado Degraded.

**Retorno e invariantes:** Sem efeito colateral; nenhum polling por leitura de status.

**Verificação:** exercitar o método público com condições determinísticas, comparar o objeto `DetectorResult<T>`, observar chamadas ao provider fake e inspecionar o Inventário **por sua API pública**; para garantias SQLite, usar banco temporário real. Distinguir observação/notificação de operação física e repetir após reinício quando houver estado persistido.



## 5. Notificações — contrato do Orquestrador

| ID | Experimento | Assertiva |
| --- | --- | --- |
| DET-N-01 | Evento com `affected_item_ids` conhecidos | Notificação inclui sync_id, lado, motivo, momento e cobertura corretos |
| DET-N-02 | Escopo alterado sem identidade estável | Array de IDs vazio **não** faz evento desaparecer |
| DET-N-03 | Vários eventos equivalentes/coalescidos | O sink aceita avisos agregados sem presumir exatamente-uma-vez |
| DET-N-04 | Sink indisponível | Erro registrado/degradação; nenhuma escrita física compensatória |
| DET-N-05 | Reinício após perda de notificação | Scanner e eventos pendentes reconstroem necessidade de reavaliação |
| DET-N-06 | Alteração feita pelo próprio operador de download | Detector observa fato; não determina novo upload sozinho |
| DET-N-07 | Raiz indisponível | Aviso de `ScopeUnavailable`, sem fabricar lista de exclusões |

Notificações são **pistas**, não decisões de conflito. Um `notify` bem-sucedido não confirma que houve sincronização.

## 6. Matriz de opcionalidade e estados

| Dado/estado | Casos obrigatórios |
| --- | --- |
| `DetectorScope.local_root?`/`remote_root_id?` | Somente o campo do lado correspondente é obrigatório; combinações inválidas rejeitadas |
| `DetectorSettings.debounce_ms?`/`polling_interval_ms?` | Ausente aplica política segura, presente aplica configuração validada |
| `request_id?` | Ausente/presente; não criar deduplicação inexistente nem suprimir mudança legítima |
| `RemotePollRequest.page_budget?` | Nenhum limite explícito vs limite finito; `has_more=true` não é `Complete` |
| `RemotePollResult.checkpoint?` | `None` antes de commit, `Some` apenas após cursor confirmado |
| `DetectorNotice.affected_item_ids` vazio | Reavaliação de escopo, não ausência de mudança |
| `DetectorStatus.last_*?` | `None` antes de observação, valor depois de coleta concluída/erro |
| `ScanCoverage` | Complete, Partial, Unavailable sem confundir com `PresenceState` |
| `DetectorState` | Start/Stop/Degraded/Failed e transições inválidas |
| `LocalScanScope` | Full/Paths; caminho fora da raiz proibido; Paths vazio depende de validação explícita |
| `RemoteScanMode` | InitialInventory, Incremental, Recovery; reinício com cursor válido/expirado |

## 7. Cenários transversais de segurança e recuperação

| Caso | Contexto | Resultado exigido |
| --- | --- | --- |
| DET-S-01 | Pasta local removida/desmontada | Unavailable; não registrar `AbsentConfirmed` em massa |
| DET-S-02 | Falha de leitura de uma subpasta | Partial; ausência não confirmada na parte ilegível |
| DET-S-03 | Mudanças com aplicativo fechado | Varredura na retomada identifica diferenças, preservando baseline |
| DET-S-04 | Evento watcher perdido/overflow | Revarredura segura, nunca exclusão por evento isolado |
| DET-S-05 | Arquivo criado/renomeado durante scan | Reconciliação com eventos concorrentes sem declarar falsa completude |
| DET-S-06 | Symlink aponta para fora da raiz | Não percorrer conteúdo externo |
| DET-S-07 | Mudança em `.drivesync/` | Não gerar item sincronizável nem notificação de arquivo do usuário |
| DET-S-08 | Resposta remota parcial e paginação | Não limpar campos omitidos ou declarar Complete antes do fim |
| DET-S-09 | `removed=true` por perda de acesso | Reavaliar recurso; não marcar exclusão confirmada sem prova |
| DET-S-10 | `changes.list` interrompido | Cursor e eventos da página corrente permanecem atômicos |
| DET-S-11 | Cursor remoto expirado | Degraded/Recovery e nova observação segura; sem reset destrutivo |
| DET-S-12 | Push remoto não chegou | Poll incremental identifica alterações |
| DET-S-13 | Notificação duplicada após reconexão | Nenhuma operação física duplicada pelo detector |
| DET-S-14 | Duas sincronizações simultâneas | Observações, cursores e avisos não vazam entre sync_ids |
| DET-S-15 | Google nativo sem checksum | Ausência legítima de metadados; nenhuma assinatura inventada |
| DET-S-16 | Erros locais/remotos em sequência | Estado e erro sanitizado, sem segredo, SQL, token ou conteúdo de arquivo |

## 8. Integração contratual sem invasão de outros módulos

- Confirmar pelo fake `google_api/` quais **operações públicas** foram chamadas para obter páginas, arquivos, changes e cursores; o detector não monta URL HTTP.
- Confirmar pelo Inventário real que somente `record_local`, `record_remote`, `publish_remote_changes` e `get_remote_cursor` (e consultas públicas necessárias) são usados; nenhuma escrita SQL externa.
- Consultar baseline antes/depois para garantir **imutabilidade do estado confirmado**; confirmar que não surge operação em fila nem atividade de transferência por simples observação.
- Verificar que o sink recebe uma notificação **após** fatos necessários terem sido persistidos, quando aplicável. Simular falha entre persistência e notificação e demonstrar recuperação, sem supor garantia de entrega exatamente uma vez.
- Não afirmar teste end-to-end do Orquestrador, Analisador ou operadores por observar apenas seus dublês.

## 9. Erros, critérios de aceite e pendências

Verificar todas as variantes públicas de `DetectorError` com `kind`, `retryable` e mensagem sanitizada; algumas falhas são transitórias, outras exigem nova observação. Nunca converter erro de rede, permissão ou watcher em exclusão confirmada.

**Aceite integral** requer: 10/10 métodos públicos implementados e testados, invariantes/fixtures/casos DET-N e DET-S pertinentes executados, ausência de operações físicas indevidas, funcionamento de retomada e recuperação de páginas, testes dentro do Flatpak e regressão sem falhas relevantes. Cobertura percentual pode ser medida, mas não substitui esses casos.

**Pendências a fechar antes de testes dependentes:** contrato de `LocalFile`, forma dos handles tipados entre módulos, fluxo exato de start/stop e agendamento, política de symlinks e cobertura de varreduras, semântica do sink quando falha, limites de debounce/polling e tratamento de mudanças durante inicialização. Não inventar valores ou resultados e não marcar como `PASS` o teste bloqueado.

## 10. Registro de execução

Para cada teste registrar **ID, cenário/fixture, comando no ambiente Flatpak, resultado PASS/FAIL/BLOCKED, evidência de chamadas/estado, erro e limitação**. Antes de declarar concluído, conferir que não faltam casos de interface nem negativos, rodar regressão e registrar métodos ainda pendentes.

**Estado:** especificação de testes; nenhum teste foi executado por este documento.
