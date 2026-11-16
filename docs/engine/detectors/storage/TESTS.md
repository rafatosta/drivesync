# Detector Storage — TESTS

## 1. Objetivo e método

Plano **Contract-First + TDD** para o **Storage real**; usar SQLite real temporário e acesso somente pela API `EventStore`. Não usar um mock do próprio armazenamento como prova de persistência. Executar os testes de persistência de forma independente, com dados sintéticos e sem serviços externos. Fakes de relógio e injeção de falha são permitidos.

RED → GREEN → REFACTOR por unidades coesas; validar o conjunto após cada etapa. `INTERFACES.md` define as seis operações do contrato; `README.md` define a propriedade e invariantes.

## 2. Fixtures

| Fixture | Conteúdo |
| --- | --- |
| `fresh_store` | Raiz temporária e banco ausente |
| `reopened_store` | Banco real com eventos persistidos, handle fechado |
| `two_stores` | Duas identidades de sincronização independentes |
| `pending_events` | Vários eventos individuais, diferentes IDs/horários/escopos |
| `active_lease` | Lote reservado, antes da confirmação |
| `expired_lease` | Relógio controlado após expiração |
| `fault_injection` | Falha entre escrita de evento/lease e commit |
| `incompatible_store` | Identidade ou versão do esquema incompatível |

## 3. Casos descritivos por operação

### STO-01 — `open`

**Sucesso:** abrir banco novo para identidade válida, fechar e reabrir preservando conteúdo.
**Negativos e opcionais:** identidade diferente, corrupção, falta de permissão, schema incompatível e raiz indisponível.
**Retorno e invariantes:** handle válido e isolado; banco existente nunca é recriado para esconder erro.
**Verificação:** reabertura SQLite e inspeção do comportamento pela API pública interna.

### STO-02 — `enqueue`

**Sucesso:** persistir evento e lê-lo após reabertura, inclusive `File.data=None` e `FileScope` explícito.
**Negativos e opcionais:** repetição com mesmo ID+conteúdo é idempotente; mesmo ID com conteúdo diferente falha; capacidade esgotada e escrita interrompida.
**Retorno e invariantes:** confirmação só depois do commit; não coalescer eventos distintos automaticamente.
**Verificação:** dados, ordem, quantidade e rollback em SQLite real.

### STO-03 — `claim_batch`

**Sucesso:** reservar N eventos em ordem, gerar lease e intervalo observado; lote vazio retorna `lease_id=None`.
**Negativos e opcionais:** N=0, duração inválida, duas threads tentando mesma fila e expiração controlada.
**Retorno e invariantes:** reservas concorrentes não incluem evento sob lease válido duas vezes.
**Verificação:** duas conexões/handles, transações e relógio determinístico.

### STO-04 — `acknowledge`

**Sucesso:** confirmar lote como `Processed` ou `Discarded`; reabrir e não reentregar registros confirmados.
**Negativos e opcionais:** lease desconhecido/expirado, identidade errada, repetição de confirmação divergente.
**Retorno e invariantes:** confirmação transacional e idempotência somente da mesma decisão comprovada.
**Verificação:** rollback, contagens e reabertura após confirmação.

### STO-05 — `release_batch`

**Sucesso:** liberar lease válido, permitindo novo claim dos mesmos eventos.
**Negativos e opcionais:** lease expirado/errado, release repetido, crash entre release e commit.
**Retorno e invariantes:** nenhum evento perdido ou confirmado por liberação.
**Verificação:** reentrega após reabertura e comparação de IDs.

### STO-06 — `status`

**Sucesso:** contagens de pending/leased/confirmed refletem enqueue/claim/ack/release.
**Negativos e opcionais:** banco indisponível; zero eventos.
**Retorno e invariantes:** consulta não reserva nem descarta.
**Verificação:** duas consultas consecutivas e comparação de estado.

## 4. Matriz transversal

| ID | Cenário | Comportamento obrigatório |
| --- | --- | --- |
| STO-S-01 | Stop do Detector depois de enqueue | Eventos continuam pendentes na reabertura |
| STO-S-02 | Falha antes do commit de enqueue | Evento não é considerado persistido |
| STO-S-03 | Falha depois do commit e antes de notificar | Evento permanece recuperável |
| STO-S-04 | Consumidor morre após claim | Lease expira; eventos são reentregues |
| STO-S-05 | Duas instâncias usam mesmo banco | Claim atômico sem perda/duplicidade sob lease |
| STO-S-06 | Dois sync_ids | Isolamento absoluto |
| STO-S-07 | Banco inválido de outro sync | Erro, nunca substituição por banco vazio |
| STO-S-08 | Lotes grandes | Limite respeitado sem leitura ilimitada |
| STO-S-09 | Eventos sem item conhecido | Evento válido para reavaliação do escopo |
| STO-S-10 | Discard explícito | Registro só muda após ack com disposição Discarded |
| STO-S-11 | Leitura de status | Sem efeito colateral ou consumo |
| STO-S-12 | Disco cheio/indisponível | Erro e backpressure, sem sucesso fictício |

## 5. Critérios de aceite

**6/6 operações** implementadas; testes de rollback, durabilidade, leases, concorrência e isolamento aprovados; execução Rust/Cargo dentro do Flatpak. Testes do Storage **não dependem** dos demais submódulos. Relatório com IDs PASS/FAIL/BLOCKED, comandos e evidências. Sem testes executados nesta etapa documental.

## 6. Testes de fidelidade Models

| ID | Caso | Evidência |
| --- | --- | --- |
| STO-F-01 | Enqueue de File Local com grupos de metadados | Round-trip SQLite preserva metadados reais e `MetadataValue` |
| STO-F-02 | Enqueue de File Remoto com provider_data | Round-trip preserva identidade de conta/provedor, grupos e payload especializado autorizado |
| STO-F-03 | File `Deleted` com `data=None` | `event.previous`, scope e ausência de dados preservados, sem fabricar exclusão |
| STO-F-04 | Renamed/Moved | `event.previous/current` preservados após release/reopen/retry |
| STO-F-05 | Mesmo `File.id` + mesmo payload | Enqueue idempotente, sem duplicar ocorrência |
| STO-F-06 | Mesmo `File.id` + payload diferente | Falha de integridade, não overwrite silencioso |
| STO-F-07 | Evolução de campos | Codec versionado migra/recusa explicitamente; lógica de leases e fila independe do esquema |
| STO-F-08 | Payload impossível de armazenar com fidelidade | Falha/backpressure e nenhuma confirmação de enqueue; não truncar |

**Não realizar testes de sync reais** neste módulo. SQLite real isolado, dentro do Flatpak, com `File` e fixtures de Models.

## Casos adicionais — recuperação do armazenamento

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| STO-R01 | Banco de eventos corrompido e backup íntegro | Mensagem ao usuário; Configurations coordena restauração; Store reabre validado |
| STO-R02 | Falha de restauração | Segunda mensagem; Store vazio reconstruído sem destruir banco anterior |
| STO-R03 | Reconstrução de fila com cursor remoto perdido | Detector refaz observação de cobertura suficiente; não inventa ausência de mudanças |
| STO-R04 | Lease/evento recuperado de backup anterior | Não confirmar como processado por causa da recuperação; redelivery seguro |
| STO-R05 | Falha temporária, schema incompatível, identidade divergente | Erro específico sem reconstrução automática |
| STO-R06 | Não é possível criar novo Store | Erro persistente, sem liberar consumo ou afirmar entrega |

Exercitar SQLite isolado e falhas injetadas, incluindo reabertura e efeito sobre checkpoints. **Especificação TDD, sem testes executados.**

## Alinhamento com recuperação SQLite comum

- O Storage registra a própria identidade na infraestrutura de recuperação de Configuration, sem repassar eventos à camada de cópia.
- Backup do banco ativo é snapshot consistente; restauração exige exclusividade e preservação do banco/WAL anterior.
- Depois de restaurar, o Storage valida identidade, schema, integridade e estado da fila.
- Falha de restauração não cria automaticamente fila vazia; reconstrução separada requer decisão da coordenação.
- Reabrir banco restaurado não comprova que eventos do intervalo posterior ao backup foram processados.

**Testes documentais; ainda não executados.**


## Implementação compilada — evidência atual

Sete testes privados usam SQLite real temporário: reabertura e igualdade integral de File com nome/alvo não UTF-8, inode u64::MAX e timestamps assinados; duplicação e conflito; ack repetido; lease ativo após reopen, expiração com relógio controlado, release e IDs estrangeiros; duas conexões concorrentes sem claims duplicados; payload corrompido/schema futuro; identidade, caminhos privados e rollback por erro SQL; codec inválido/futuro; corrupção durante reserva recusando confirmação e mantendo indisponibilidade.

A API pública tem testes de captura durante pause, reentrega após reopen com IDs/payload iguais, ack persistido e retomada sem perda. Um subprocesso encerra sem executar destrutores após commits; a reabertura recupera os eventos. Há também um teste de SQLITE_BUSY: depois de liberar a transação externa, o mesmo evento chega à fila sem outra mudança na pasta. O helper de subprocesso retorna sem trabalho quando não recebe o ambiente de fixture; sua execução efetiva é comandada pelo teste pai.

O Orchestrator diagnóstico testa pause persistido entre instâncias, histórico recuperado e consumo após resume. A UI exercita o botão real, pendentes e isolamento entre contas. Não se declara a matriz inteira concluída: backup/restauração, falta real de energia, disco realmente cheio, todos os ataques concorrentes de mesmo UID e todos os editores não foram testados. Nenhum teste confirma sincronização com Google ou Inventory.

Validação atual usa a fachada Detector e SQLite real isolado. Inclui recuperação após encerramento abrupto, lotes mistos, pausa, identidade por conta/raízes e os quatro cenários de localização: banco em `.drivesync/detector.sqlite3` com permissões privadas e Inventário intacto; rejeição de outra identidade; recusa de symlink e raiz ausente; formato antigo rejeitado sem migração ou alteração do arquivo. O teste de captura nativa verifica que gravações dos bancos internos não produzem eventos de usuário.
