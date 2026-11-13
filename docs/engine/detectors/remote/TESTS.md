# Detector Remoto — TESTS

## 1. Isolamento e TDD

Testar **apenas o Detector Remoto real** com um adapter remoto fake da interface definida pelo módulo (testes HTTP/Google ficam em `remote/google/`) e um sink individual **fake durável**, com falhas configuráveis. Usar checkpoint backend fake ou temporário apropriado. Executar os cenários em isolamento, sem serviço remoto real. TDD: RED, GREEN, REFACTOR, regressão por unidade coesa.

## 2. Fixtures

| Fixture | Cenário |
| --- | --- |
| `remote_pages` | Árvore paginada com arquivos e diretórios |
| `changes_pages` | Mudanças incrementais em múltiplas páginas |
| `concurrent_changes` | Alteração entre cursor inicial e leitura da árvore |
| `removed_access` | `removed=true` por perda de acesso |
| `expired_checkpoint` | Cursor expirado e leitura de recuperação |
| `sink_failure` | Aceitação durável negada após parte da página |
| `checkpoint_failure` | Falha ao confirmar cursor depois de enfileirar eventos |
| `replayed_page` | Repetição após interrupção |
| `offline_interval` | Execução posterior a período sem polling |

## 3. Casos individuais

### REM-01 — `new`

**Sucesso:** construir sem requisição HTTP; retorno Stopped com portas injetadas.
**Negativos e opcionais:** ID de raiz ausente, sessão de outra conta, polling_interval omitido/presente.
**Retorno e invariantes:** utiliza somente as portas fornecidas pela interface.
**Verificação:** spies nas dependências.

### REM-02 — `start`

**Sucesso:** agendar leitura inicial ou incremental conforme modo/checkpoint.
**Negativos e opcionais:** start repetido, checkpoint inválido, fonte indisponível.
**Retorno e invariantes:** estado Starting/Degraded coerente; não afirma que terminou varredura.
**Verificação:** scheduler fake e checkpoint fake.

### REM-03 — `poll_once`

**Sucesso:** consultar páginas pelo API remota fake, emitir **um evento por `emit`**, confirmar cursor só depois de sinais aceitos.
**Negativos e opcionais:** page_budget finito, erro em página, token de página ausente, sinal de remoção por perda de acesso, campos projetados incompletos, sink falhando, cursor expirado.
**Retorno e invariantes:** sem avanço de checkpoint após falha de emissão; leitura parcial não prova exclusão.
**Verificação:** ordem de chamadas emit/commit, IDs, contagem, has_more, checkpoints e replays.

### REM-04 — `stop`

**Sucesso:** parar scheduler sem apagar checkpoint.
**Negativos e opcionais:** stop durante paginação, repetido, timeout.
**Retorno e invariantes:** evento já aceito pelo sink não é descartado; sem chamadas HTTP após stop efetivo.
**Verificação:** scheduler/controlador fake.

### REM-05 — `status`

**Sucesso:** estado, último horário e erro conhecidos.
**Negativos e opcionais:** antes do primeiro poll = opcionais vazios; após falha = erro sanitizado.
**Retorno e invariantes:** consulta não faz HTTP nem altera cursor.
**Verificação:** spies de dependências e igualdade de status.

## 4. Cenários transversais

| ID | Situação | Evidência |
| --- | --- | --- |
| REM-S-01 | Sem conexão por vários intervalos | Recuperação incremental sem depender de notificações push |
| REM-S-02 | Sink indisponível | Evento não marcado durável e checkpoint não avança |
| REM-S-03 | Falha ao confirmar checkpoint após enqueue | Replay seguro, sem perda de alteração |
| REM-S-04 | Cursor expirado | Recovery sem exclusão inferida |
| REM-S-05 | Removed por perda de acesso | Apenas necessidade de reavaliação |
| REM-S-06 | Duas contas distintas | Estado, sessão e checkpoint isolados |
| REM-S-07 | 403/404/timeout | Erro remoto tipado, não ausência |
| REM-S-08 | Arquivo Google nativo, hashes ausentes | Não inventar propriedades |
| REM-S-09 | Page budget insuficiente | Partial/has_more, sem completar falsamente |
| REM-S-10 | Alterações concorrentes na observação inicial | Cursor anterior e leitura conciliados |
| REM-S-11 | Stop/restart | Retomada a partir do último checkpoint seguro |
| REM-S-12 | Emissão de sinais | Cada chamada ao sink contém exatamente um evento |

## 5. Critério de conclusão

**5/5 contratos** e todos os cenários aplicáveis executados no Flatpak com dependências simuladas, sem exigir módulos concretos irmãos. Reportar PASS/FAIL/BLOCKED, comandos, commits e limitações; não afirmar integração real ao Google por sucesso com fake. Nenhum teste foi executado nesta etapa documental.

## 6. Cenários de Models e provedor

| ID | Cenário | Expectativa |
| --- | --- | --- |
| REM-F-01 | GoogleFile com ID e campos conhecidos | `File.data=Some(FileData::Remote(RemoteFile))`, metadados e envelope do provedor preservados |
| REM-F-02 | Resposta parcial/sem ID | Recheck com `data=None` quando sem referência segura; não fabricar RemoteFile |
| REM-F-03 | Criar/editar | `Created`/`Modified` somente com evidência, contexto e dados atuais |
| REM-F-04 | Renomear/mover | `Renamed`/`Moved` só com `previous/current` correlacionados |
| REM-F-05 | Removed/perda de acesso | Não executar nem concluir exclusão; `RecheckRequired` ou `ScopeUnavailable` |
| REM-F-06 | Replay/checkpoint | Mesmo File integral persiste; checkpoint não avança após falha de sink |
| REM-F-07 | Evolução RemoteFile | Alterar adaptador e seus testes sem tocar polling, cursor ou emissão do Detector |
| REM-F-08 | Provedor diferente | Mesmo contrato `File`/`RemoteFile`, sem dependência da fachada ao GoogleFile |

## Controle de entrega e estado — testes adicionais

| ID | Cenário | Resultado |
| --- | --- | --- |
| REM-S-01 | Pai suspendeu entrega ao Orquestrador | Remoto continua polling e persistindo eventos pelo sink |
| REM-S-02 | `status()` durante polling/espera | Atividade correta (`Polling`/`Waiting`), contadores atuais e sem HTTP na consulta |
| REM-S-03 | Falha remota com Local saudável | Status e último erro remotos disponíveis ao pai; não afetar estado do Local |
| REM-S-04 | `stop()` no meio de paginação | Não avança checkpoint sem persistência completa; não perde eventos aceitos |
| REM-S-05 | `start()` depois de período sem observação | Não executa reconciliação/Scanner por iniciativa própria; decisão é do Orquestrador |

**Estado:** cenários documentais, ainda não executados.

## Evidência da implementação

Os testes em `engine/src/detectors/remote/` exercitam o produtor e scheduler reais com portas falsas e SQLite temporário: paginação da árvore/Changes, cursor anterior à árvore, emissão individual, falhas de sink/checkpoint e replay idempotente, expiração/Recovery, campos parciais e filtragem, remoção/perda de acesso conservadoras, pertencimento por ancestralidade, horários e Retry-After, cancelamento de worker e poll explícito, retomada, leases/ack, corrupção e isolamento por conta/raiz. Models testa validação pura e associação do envelope; Orchestrator rejeita anexar uma origem de outra conta antes de criar armazenamento.

O fixture gráfico do aplicativo usa duas contas sintéticas e falha de autorização real de Accounts, sem credenciais, para verificar a apresentação de erros/horários, troca de abas e pause/play independente. A integração com a conta de testes autorizada foi executada separadamente e está registrada em [IMPLEMENTATION.md](IMPLEMENTATION.md).


A seleção por provedor é testada em `dispatch_tests.rs`, com Accounts real e metadados temporários sintéticos: seleção Google sem autenticação na construção; provedor não registrado sem fallback/sem fila; conta inexistente sem fila. O runtime também é exercitado com uma implementação de `RemoteObserver` independente de Google e verificação de isolamento por ProviderId. O namespace Google anterior é calculado e comparado com o caminho real, e payload de outro provedor é recusado.
