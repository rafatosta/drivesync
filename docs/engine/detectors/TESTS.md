# Detector — TESTS

## 1. Objetivo e escopo

Contrato TDD do **módulo público `Detector`**, **não** dos detectores internos. São **sete operações públicas** descritas em [INTERFACES.md](INTERFACES.md), além da porta de eventos `DetectorEventSink.publish`. [README.md](README.md) define responsabilidades e limites.

Testar o **coordenador real** com **duas fontes internas simuladas e independentes**, sink fake do Orquestrador e Inventário real isolado quando precisar verificar persistência. Não executar filesystem watcher nem cliente Google real nos testes do coordenador; essas responsabilidades pertencem às suítes dos filhos. O Orquestrador simulado não deve receber/importar seus tipos.

## 2. Estratégia TDD e fixtures

RED → GREEN → REFACTOR por grupo funcional: composição; ciclo de vida; solicitação de observação; tradução e emissão de eventos; status/erros; recuperação e concorrência. Escrever casos antes da implementação, executar regressão incremental, registrar commits locais validados.

| Fixture | Conteúdo | Verificação |
| --- | --- | --- |
| `healthy_sources` | Duas fontes internas fake com eventos diferentes | Coordenação sem distinção pública de origem |
| `local_degraded` | Uma fonte funcional, outra indisponível | Estado agregado e eventos preservados |
| `remote_degraded` | Ordem inversa de falha | Mesmo contrato externo, sem privilégios de uma origem |
| `duplicate_events` | Eventos repetidos/reordenados | Agregação não perde reavaliação |
| `unknown_identity` | Evento interno sem SyncItemId | Aviso público válido de reavaliação de escopo |
| `inventory_rollback` | Falha ao persistir fatos relacionados | Nenhum evento marca `persisted=true` sem commit |
| `failed_sink` | Sink retorna erro depois de fatos persistidos | Diagnóstico e recuperação sem perda do fato |
| `closed_app` | Pausa/stop, alterações simuladas e resume | Necessidade de recuperação repassada às fontes |

## 3. Índice dos sete métodos

| Caso | Método | Evidência principal |
| --- | --- | --- |
| DET-01 | `Detector::new` | Composição e encapsulamento |
| DET-02 | `Detector::start` | Início e estado parcial |
| DET-03 | `Detector::pause` | Suspensão |
| DET-04 | `Detector::resume` | Recuperação após pausa |
| DET-05 | `Detector::stop` | Parada segura |
| DET-06 | `Detector::request_scan` | Pedido de reavaliação |
| DET-07 | `Detector::status` | Estado agregado |

## 4. Casos descritivos por método

### DET-01 — `Detector::new`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Criar instância para `sync_id` válido, dependências fictícias e duas fontes internas simuladas; expor só `Detector`.

**Negativos e opcionais:** Identidade inválida, Inventário indisponível, sink inválido; construção nunca obriga Orquestrador a criar Local/Remoto.

**Retorno e invariantes:** Nenhuma detecção iniciada; tipos internos não aparecem em superfície pública.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-02 — `Detector::start`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Solicitar início e verificar que o coordenador inicia fontes disponíveis e informa agendamento de recuperação.

**Negativos e opcionais:** Uma fonte inicia, outra falha; start repetido, dependência indisponível, estado degradado.

**Retorno e invariantes:** Status agregado conserva falha; não gera exclusões nem confirma leitura inexistente.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-03 — `Detector::pause`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Pausar detecção e verificar encaminhamento interno do controle.

**Negativos e opcionais:** Pausa já ativa, fonte indisponível ou encerramento concorrente.

**Retorno e invariantes:** Dados persistidos não são apagados; pausa não paralisa operações de sincronização fora do Detector.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-04 — `Detector::resume`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Retomar depois de intervalo offline e verificar que ambas as fontes recebem pedido de recuperação segundo suas estratégias.

**Negativos e opcionais:** Falha de retomada em uma fonte, eventos perdidos, estado anterior inválido.

**Retorno e invariantes:** Retomada não presume que nenhuma alteração ocorreu durante pausa; status identifica degradação.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-05 — `Detector::stop`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Encerrar fontes e liberar recursos; consultar estado Stopped.

**Negativos e opcionais:** Parada repetida, fontes interrompidas, observações em andamento.

**Retorno e invariantes:** Não modifica baseline, fila, cursor válido ou histórico do Inventário.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-06 — `Detector::request_scan`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Solicitar observação `All` e `Items` pelo mesmo contrato, sem selecionar mecanismo de watcher/polling.

**Negativos e opcionais:** Lista `Items` vazia, item de outra sincronização, request_id opcional, fonte indisponível.

**Retorno e invariantes:** Retorno de aceitação não é verificação concluída; nenhum upload/download é iniciado.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


### DET-07 — `Detector::status`

**Contrato:** método, request e result públicos definidos em `INTERFACES.md`.

**Sucesso:** Obter estado agregado do módulo após eventos/leituras com fontes saudáveis.

**Negativos e opcionais:** Status antes do start, uma fonte degradada, última observação/erro ausentes.

**Retorno e invariantes:** Nenhum campo de status público exige conhecimento de Local/Remoto, e leitura não dispara observação.

**Verificação:** conferir apenas o contrato público, chamadas às fontes fake e estado do Inventário pela API; usar estados simulados de origem diferentes sem mudar o objeto externo. Inspecionar erros sanitizados, ausência de efeitos proibidos e transições.


## 5. Notificações unificadas

| Caso | Estímulo interno | Esperado na API pública |
| --- | --- | --- |
| DET-N-01 | Mudança local com item identificado | `DetectorEvent` válido, sem tipo de filesystem exposto |
| DET-N-02 | Mudança remota com item identificado | **Mesmo tipo** de `DetectorEvent`, sem GoogleChange/JSON/cursor |
| DET-N-03 | Evento sem identidade confiável | `affected_items=[]` significa reavaliar escopo |
| DET-N-04 | Campo `persisted=true` | Inventário confirma previamente os fatos necessários |
| DET-N-05 | Falha de persistência | Aviso diagnóstico com `persisted=false`, sem alegar observação confirmada |
| DET-N-06 | Sink falha | Registrar erro e permitir recuperação; não executar operações físicas |
| DET-N-07 | Eventos duplicados, agregados ou perdidos | Não presumir exatamente-uma-vez; fontes recebem recuperação na retomada |
| DET-N-08 | Evento de fonte degradada | Cobertura `Partial`/`Unavailable` e status compatível |
| DET-N-09 | Tipo interno novo em uma fonte | A API pública continua a mesma e exige apenas tradução sem perda de diagnóstico essencial |

## 6. Visibilidade e independência — testes de compilação

- Um consumidor externo consegue importar `Detector`, `DetectorEvent`, tipos públicos de request/result e `DetectorEventSink`.
- Um consumidor externo **não** consegue importar, construir ou chamar `LocalDetector`, `RemoteDetector`, `InternalObservationSource`, `LocalNotificationSink`, watchers, cursors ou tipos concretos dos filhos pela fachada pública.
- `DetectorCreateRequest` não exige que o consumidor instancie os filhos.
- O teste do coordenador funciona sem implementar as duas fontes reais, usando fakes internos.
- O teste do Local pode ser executado sem implementar o Remoto, e vice-versa.
- O contrato público não discrimina execução local e remota em assinaturas diferentes.

## 7. Matriz de estados, opcionais e recuperação

Testar todas as variantes de `DetectorState`, `DetectorCoverage`, `DetectorEventKind`, `DetectorScanScope`, `DetectorScanReason`, `DetectorError`. Cobrir `correlation_id=None/Some`, `request_id=None/Some`, `last_observation_at=None/Some`, `last_error=None/Some`; lista vazia de itens (reavaliação ampla em evento, mas requisição `Items([])` inválida). `pending_checks` e `degraded_sources` são agregados tipados, sem detalhes internos.

Garantir que `request_scan` apenas agenda detecção, não realiza sincronização física. Simular fontes com estado independente, parada parcial, interrupção após commit e antes do aviso, e retomada após perda de eventos efêmeros.

## 8. Pausa, recuperação e entrega diferida — testes obrigatórios

| Caso | Situação | Resultado exigido |
| --- | --- | --- |
| DET-P-01 | `pause` com fontes ativas | Nenhuma notificação de processamento normal é entregue enquanto pausado; `status` informa `Paused` |
| DET-P-02 | Local entrega vários sinais do mesmo item durante pausa | Necessidade de reavaliação permanece e pode ser agregada, sem exigir reenvio literal dos eventos brutos |
| DET-P-03 | Remoto continua polling durante pausa | Eventos/checkpoint confirmados no Inventário não são descartados; entrega pode ficar adiada |
| DET-P-04 | Remoto suspende polling durante pausa | `resume` solicita consulta incremental desde cursor confirmado; não presume ausência de mudança |
| DET-P-05 | Local perde watcher durante pausa | `resume` solicita varredura apropriada, sem inferir exclusões |
| DET-P-06 | `resume` com pendências de ambos os filhos | Eventos de reavaliação chegam pela **mesma API pública**, sem tipos locais/remotos |
| DET-P-07 | Falha de recuperação em uma fonte | Estado Degraded e cobertura Partial/Unavailable, preservando fatos válidos da outra |
| DET-P-08 | Stop e novo Start após alterações | Recuperação solicitada mesmo sem eventos em memória |
| DET-P-09 | Crash após observação persistida mas antes de aviso | Recuperação/revarredura emite necessidade de reavaliação, sem depender do buffer volátil |
| DET-P-10 | Buffer de sinais cheio ou eventos duplicados | Agregação ou reavaliação por escopo sem perda silenciosa |
| DET-P-11 | Pause enquanto o Analisador já processa transferência | O Detector não cancela operação externa; apenas controla sua própria entrega |
| DET-P-12 | Recuperação apenas agendada | `recovery_scheduled=true` não é interpretado como conclusão da recuperação |

**Independência das fontes:** simular Local orientado a eventos e Remoto com polling de tempo determinístico. Não impor o mesmo comportamento de pausa às duas fontes. O que o teste da fachada exige é **nenhuma perda permanente da necessidade de reavaliar**, respeitando durabilidade e escopo, não preservação de todos os eventos brutos.

**Separação de filas:** comprovar que notificações pendentes de detecção não são inseridas como `sync_operations` antes de análise/autorização. Uma fila ou buffer interno não substitui a persistência de fatos no Inventário nem justifica banco adicional.

## 9. Erros, bloqueios e critérios de aceite

Cada erro tipado deve incluir `kind`, `retryable` e mensagem sanitizada. Falha em uma fonte não deve mascarar observações válidas da outra; indisponibilidade não é ausência confirmada.

**Aceite integral:** sete métodos e emissão de eventos implementados, testes DET-01…DET-07 e DET-N aplicáveis efetivamente executados, visibilidade de tipos comprovada, falha parcial e recuperação demonstradas, regressão sem falhas, comandos Rust somente no Flatpak. **Não exigir implementação real de Local e Remoto para validar a fachada isoladamente.** Integração completa exige outra etapa.

**Pendências técnicas:** ownership/traits reais, criação interna das fontes, agendamento/async, entrega e persistência dos fatos. Decidir antes de implementar casos dependentes, sem ampliar API pública unilateralmente.

## 10. Registro de testes

Relatar IDs executados, PASS/FAIL/BLOCKED, comandos, ambiente, commits, limitações e diferenças entre simulação de fontes e integração real. **Este arquivo não prova execução dos testes.**
