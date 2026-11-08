# Detector Remoto — TESTS

## 1. Isolamento e TDD

Testar **apenas o Detector Remoto real** com um adapter remoto fake da interface definida pelo módulo (testes HTTP/Google ficam em `remote/google/`) e um sink individual **fake durável**, com falhas configuráveis. Usar checkpoint backend fake ou temporário apropriado. **Não** exigir Detector Local, `EventStore`, SQLite do Detector, Orquestrador ou serviço remoto real. TDD: RED, GREEN, REFACTOR, regressão por unidade coesa.

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
**Retorno e invariantes:** não cria ou acessa banco do Detector e não conhece Orquestrador.
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
| REM-S-10 | Alterações concorrentes no inventário inicial | Cursor anterior e leitura conciliados |
| REM-S-11 | Stop/restart | Retomada a partir do último checkpoint seguro |
| REM-S-12 | Emissão de sinais | Cada chamada ao sink contém exatamente um evento |

## 5. Critério de conclusão

**5/5 contratos** e todos os cenários aplicáveis executados no Flatpak com dependências simuladas, sem exigir módulos concretos irmãos. Reportar PASS/FAIL/BLOCKED, comandos, commits e limitações; não afirmar integração real ao Google por sucesso com fake. Nenhum teste foi executado nesta etapa documental.
