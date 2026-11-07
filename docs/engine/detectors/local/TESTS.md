# Detector Local — TESTS

## 1. Objetivo e isolamento

Plano TDD para **somente o Detector Local**. Fonte de verdade: [README.md](README.md) e [INTERFACES.md](INTERFACES.md). Não é necessário compilar, testar ou implementar o Detector Remoto para concluir esta etapa.

Testar `LocalDetector` real com **filesystem real em diretórios temporários**, Inventário/SQLite real isolado acessado por **API pública**, sink fake de notificação, relógio/scheduler injetados para determinismo. Nunca utilizar arquivos pessoais, Google Drive real, credenciais ou GUI. O watcher deve ser exercitado quando disponível; não substituí-lo inteiramente por mock da unidade sob teste.

## 2. Ciclo TDD

RED: escrever teste de compilação e de comportamento que falhem pela capacidade ausente. GREEN: implementar mínimo. REFACTOR: manter contratos e rodar regressão. Desenvolver incrementalmente em grupos: ciclo de vida, observação, integração com Inventário, avisos e recuperação após reinício. Cada grupo validado deve gerar commit local conforme [AGENTS.md](AGENTS.md).

## 3. Fixtures

| Fixture | Conteúdo | Resultado a avaliar |
| --- | --- | --- |
| `empty_root` | Raiz temporária + inventário do mesmo sync_id | Varredura inicial vazia |
| `local_tree` | Arquivos, subpastas, nomes Unicode e bytes Unix inválidos para UTF-8 | Observações preservam nomes e atributos |
| `closed_app_changes` | Criar/editar/mover arquivos com watcher desligado | Nova varredura detecta diferenças |
| `partial_tree` | Falha controlada em subárvore | Partial/Unavailable, não exclusão |
| `external_symlink` | Link apontando para fora da raiz | Não observar conteúdo externo |
| `reserved_directory` | Metadados em `.drivesync/` | Ignorar como arquivo de usuário |
| `concurrent_edits` | Mutação durante scanner e registro de watcher | Sem lacuna declarada como Complete |
| `notification_sink` | Coleta e falha injetável de avisos | Avisos corretos e recuperação |
| `separate_roots` | Duas raízes e inventários isolados | Nenhum vazamento cruzado |

## 4. Casos por método

### LOC-01 — `LocalDetector::new`

**Contrato:** método e objetos tipados de [INTERFACES.md](INTERFACES.md).

**Sucesso:** Com raiz temporária válida, identidade conhecida, Inventário isolado e sink fake, construir instância Stopped sem observar filesystem.

**Negativos e opcionais:** Raiz interna `.drivesync/`, vínculo inválido ou raiz fora do escopo; validação deve falhar sem ações externas.

**Retorno e invariantes:** Nenhuma observação gravada, watcher não iniciado, sem acesso remoto.

**Verificação objetiva:** comparar resposta pública, chamadas ao sink, observações obtidas pela API do Inventário antes/depois; fechar e reabrir o banco quando houver persistência; validar erros tipados e ausência de alterações físicas proibidas.

### LOC-02 — `LocalDetector::start`

**Contrato:** método e objetos tipados de [INTERFACES.md](INTERFACES.md).

**Sucesso:** Registrar watcher e varredura inicial/retomada; detectar mudanças ocorridas antes da inicialização.

**Negativos e opcionais:** Iniciar duas vezes, perder watcher, raiz removida durante inicialização e falha de agendamento.

**Retorno e invariantes:** Estado coerente Running/Starting/Degraded; não declarar ausência por falha de watcher.

**Verificação objetiva:** comparar resposta pública, chamadas ao sink, observações obtidas pela API do Inventário antes/depois; fechar e reabrir o banco quando houver persistência; validar erros tipados e ausência de alterações físicas proibidas.

### LOC-03 — `LocalDetector::scan_once`

**Contrato:** método e objetos tipados de [INTERFACES.md](INTERFACES.md).

**Sucesso:** Varredura Full e Paths com arquivos e subpastas; observar criação/edição/renomeação/movimento via API do Inventário.

**Negativos e opcionais:** Subárvore ilegível, arquivo alterado durante scan, symlink externo, bytes não UTF-8, cancelamento; request_id None/Some.

**Retorno e invariantes:** Complete só com escopo validado; Partial preserva regiões desconhecidas; baseline e fila não mudam.

**Verificação objetiva:** comparar resposta pública, chamadas ao sink, observações obtidas pela API do Inventário antes/depois; fechar e reabrir o banco quando houver persistência; validar erros tipados e ausência de alterações físicas proibidas.

### LOC-04 — `LocalDetector::stop`

**Contrato:** método e objetos tipados de [INTERFACES.md](INTERFACES.md).

**Sucesso:** Parar watcher e varreduras sem apagar inventário; liberar recursos.

**Negativos e opcionais:** Parada repetida; stop enquanto varredura ainda está em andamento; falha de encerramento.

**Retorno e invariantes:** Não alegar sincronização concluída ou criar atividade de transferência.

**Verificação objetiva:** comparar resposta pública, chamadas ao sink, observações obtidas pela API do Inventário antes/depois; fechar e reabrir o banco quando houver persistência; validar erros tipados e ausência de alterações físicas proibidas.

### LOC-05 — `LocalDetector::status`

**Contrato:** método e objetos tipados de [INTERFACES.md](INTERFACES.md).

**Sucesso:** Ler estado, cobertura e erro sem alterar dados.

**Negativos e opcionais:** Antes de qualquer scan, campos last_* None; após falha, erro sanitizado.

**Retorno e invariantes:** Não dispara varredura nem contato remoto; leitura não modifica estados.

**Verificação objetiva:** comparar resposta pública, chamadas ao sink, observações obtidas pela API do Inventário antes/depois; fechar e reabrir o banco quando houver persistência; validar erros tipados e ausência de alterações físicas proibidas.

## 5. Notificações e recuperação

| Caso | Situação | Verificação |
| --- | --- | --- |
| LOC-N-01 | Mudança de arquivo com item conhecido | Aviso inclui `sync_id`, motivo, ID e cobertura corretos |
| LOC-N-02 | Alteração sem identidade segura | Aviso de reavaliação de escopo, mesmo com lista de IDs vazia |
| LOC-N-03 | Eventos repetidos em rápida sequência | Agregação não perde necessidade de reavaliação |
| LOC-N-04 | Sink falha depois de observação persistida | Diagnosticar falha e permitir revarredura/reemissão |
| LOC-N-05 | App desligado durante alterações | Reinício identifica mudanças sem histórico de watcher |
| LOC-N-06 | Operador local alterou arquivo | Detector registra fato, não inicia upload por conta própria |

## 6. Testes transversais de segurança

| Caso | Experimento | Evidência |
| --- | --- | --- |
| LOC-S-01 | Raiz desmontada/ausente | Unavailable sem exclusão em massa |
| LOC-S-02 | Subpasta inacessível | Partial, ausência não comprovada |
| LOC-S-03 | Evento perdido/overflow do watcher | Recuperação por varredura |
| LOC-S-04 | Arquivo criado/renomeado durante scan | Cobertura não declarada completa incorretamente |
| LOC-S-05 | Symlink externo | Nenhuma leitura fora da raiz |
| LOC-S-06 | `.drivesync/` modificado | Nenhum SyncItem de metadados internos |
| LOC-S-07 | Nome Unix não UTF-8 | Bytes preservados sem conversão com perda |
| LOC-S-08 | Duas sincronizações simultâneas | Notificações e observações isoladas |
| LOC-S-09 | Campo local não observado | Não sobrescrever valor anterior com None/zero |
| LOC-S-10 | Estado local diferente do baseline | Baseline preservado; detector não decide operação |
| LOC-S-11 | Falha na persistência | Não informar fato como durável |
| LOC-S-12 | Repetição da mesma varredura | Não duplicar vínculos ou gerar histórico artificial |

## 7. Tipos, opcionais e erros

Cobrir todas as variantes de `LocalDetectorState`, `LocalScanCoverage`, `LocalScanScope`, `LocalScanCause`, `LocalStopReason`, `LocalNoticeReason` e `LocalDetectorError`. Testar `debounce_ms=None/Some`, `request_id=None/Some`, `notification=None/Some` e `last_observation_at/last_coverage/last_error=None/Some`. `Paths` deve validar escopo e tratar lista vazia conforme decisão documentada, sem escolher comportamento arbitrário.

Todo `LocalDetectorFailure` precisa ter tipo de erro, mensagem sanitizada e sinalização de possibilidade de retry. Falha de permissão, filesystem ou watcher **não** corresponde a ausência confirmada.

## 8. Pendências e aceite

Dependências ainda abertas: contrato definitivo `LocalFile`, implementação do sink do Orquestrador, concreteza dos handles, política de symlinks, valores/política de debounce e protocolo de start/stop. Testes que dependem dessas decisões devem ficar marcados **BLOCKED** até formalização, não aprovados por mocks permissivos.

Concluir o **Detector Local** somente com **5/5 métodos públicos** implementados e testes aplicáveis aprovados, cenário de retomada após interrupção e casos de parcialidade comprovados, integração de Inventário validada, regressão sem falhas relevantes e execução Rust no ambiente Flatpak. Não exigir conclusão do Detector Remoto.

## 9. Relatório

Informar IDs LOC-01…LOC-05, LOC-N e LOC-S executados, PASS/FAIL/BLOCKED, ambiente, comandos, evidência de persistência e varredura, commits locais e limitações. **Documento de testes não representa teste executado.**
