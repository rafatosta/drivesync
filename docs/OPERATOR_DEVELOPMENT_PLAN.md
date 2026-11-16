# Operador: análise e plano de desenvolvimento

Data: 16/11/2026. Plano proposto, sem implementação de executor ou liberação de operações.

## 1. Referências inspecionadas

- Base atual: `refactor/engine-rebuild`, commit `1d592fa8f84d5913532c8e9e2268f29d1ce7de00`.
- Código anterior: `archive/sync-engine-before-rebuild`, commit `97bd484f47dc836fc1e9c79e4e7da6bfccd54696`.
- A outra branch, `archive/inventory-before-connector-publication-20261114`, não é a referência deste reaproveitamento do motor antigo.
- Analisador: `engine/src/analyzer/{mod,planning,persistence}.rs`, documentação em `docs/engine/analyzer/`, integração em `engine/src/orchestrator/analysis.rs`.
- Contratos: `engine/src/operations/{interfaces,models}.rs`, `engine/tests/operator_contract.rs` e `docs/engine/operations/`.
- Transportes atuais: `engine/src/providers/google/{downloads,uploads,mutations}.rs`; persistência em `engine/src/inventory/storage/{analysis,operations}.rs`.
- Fontes arquivadas citadas abaixo são caminhos relativos à branch arquivada, consultados por `git show`, sem trocar o checkout para executá-las.

As conclusões são de inspeção de código e testes existentes. As suítes antigas não foram reexecutadas nesta análise; não se declara nova validação de rede real, queda de energia ou aplicativo instalado. `docs/engine/analyzer/LIVE_VALIDATION.md` registra aceitação anterior do Analisador em conta real, com 23 verificações, mas explicitamente sem execução do Operador.

## 2. O que o Analisador entrega

O Analisador está suficientemente estruturado para desenvolver a execução isolada. Ainda falta o protocolo que autoriza consumir seus planos.

- Analisa um `SyncItem` por chamada; Reconciler e Conflicts são privados, independentes e puros. Conflito identificado não significa conflito resolvido.
- Persiste recibo, evidências, requisitos, decisões e `AnalysisPlan` por transação do Inventory, com retry idempotente e revalidação do snapshot.
- Materializa `OperationCommand` quando há dados: criação de diretório, upload, download, atualização, rename, move, trash e restore. Não representa exclusão permanente.
- Resolve pais por identidade associada ou raiz configurada. Pai ainda sem réplica gera dependência, não ID inventado. Homônimos e hashes iguais não associam itens.
- Guarda itens e época de observação do namespace. Mudanças relevantes tornam o plano obsoleto. O índice de contexto é reutilizado por época.
- Uma segunda ação depende da anterior: é necessário reobservar e reanalisar depois de mudar versão, nome ou pai.
- Trash continua condicionado à autorização e à verificação dos sobreviventes/descendentes. Restore exige solicitação explícita.
- O Orchestrator já faz análise inicial após Scanner completo e ingestão contínua. Não transfere conteúdo.

**Barreira efetiva:** operações vinculadas a decisões do Analisador nascem `Blocked`. `ensure_executable` recusa claim/transições genéricas para essas operações, inclusive quando já existe comando no plano. O comentário dessa função ainda menciona ausência de comando persistido; o comportamento real é bloquear qualquer operação vinculada à análise. Atualizar essa explicação ao implementar a integração, sem simplesmente retirar a proteção.

Nem `NoChanges`, nem `PendingOperations`, nem `Verified` isoladamente autorizam marcar o item como sincronizado. Associação e baseline precisam de evidência própria após o efeito.

## 3. O que o motor arquivado realmente garante

### Download

`download/stream.rs` copia em blocos limitados, valida tamanho e MD5 remoto e calcula SHA-256 local. `download/filesystem.rs` usa descritores, `NOFOLLOW`, criação exclusiva, validação da raiz/pais e publicação sem substituir destino ocupado. `download/execution.rs` sincroniza temporário e diretório, relê o staging e persiste fases antes/depois da instalação.

`download/recovery_tests.rs`, especialmente `i25_incomplete_stream_and_verified_staging_never_confirm_a_base` e `i26_actual_effect_without_confirmation_replays_only_the_verified_identity`, cobre fluxo incompleto, cancelamento, falha de persistência e recuperação de efeito sem confirmação. O destino anterior é preservado; conteúdo parcial não confirma baseline.

**Limite:** recuperação segura não equivale a retomada por byte. O caminho de transferência examinado chama `source.media` e copia desde o início; o teste de recuperação espera nova leitura da mídia. Reaproveitar a segurança e a recuperação, sem anunciar download parcial retomável.

**Atualização local:** `download/execution/updates.rs` move a versão anterior para backup, valida o que realmente foi movido e instala a nova versão sem sobrescrever terceiros. Isso preserva versões e oferece recuperação, mas são duas renomeações: há uma janela em que o nome final está ausente. Não transportar esse algoritmo como se fosse substituição atômica contínua. Publicação de arquivo novo e atualização de arquivo existente são problemas diferentes.

### Upload

`upload/execution.rs`, `upload/local.rs`, `upload/models.rs` e `state/uploads.rs` implementam criação com intenção durável, ID remoto gerado antes do efeito, snapshot privado, identidade da origem, hashes MD5/SHA-256, sessão persistida e consulta do deslocamento confirmado pelo servidor. A confirmação verifica o recurso por ID e metadados/conteúdo esperados; resposta perdida não autoriza criar outro ID.

Evidências particularmente úteis em `upload/tests.rs`:

- `interrupted_chunks_lost_confirmation_and_expired_sessions_reuse_one_id`;
- `lost_folder_response_recovers_without_duplicate_folder_or_children`;
- `cancellation_retains_the_session_and_confirmed_remote_removal_is_not_recreated`;
- `corrupt_staging_blocks_replay_without_sending_another_chunk`;
- `e04_offline_reconciliation_is_bounded_and_preserves_snapshot_and_bytes`;
- `n11_fifth_attempt_timeout_reconciles_without_a_sixth_effect`;
- cenários de escritor concorrente, origem substituída, snapshot adulterado, revogação e corrupção do estado.

**Limite decisivo:** esse upload é explicitamente *create-only*. Não prova atualização segura de conteúdo remoto existente. Os testes usam falhas simuladas; sua existência não comprova interrupção real da conexão ou queda de energia em produção.

## 4. Matriz de reaproveitamento

| Fonte arquivada | Aproveitar | Adaptação necessária |
| --- | --- | --- |
| `download/filesystem.rs` | Acesso por descritores, proteção de raiz/pais, arquivos regulares, exclusividade, publicação sem sobrescrita | Modelos atuais, nomes Unix em bytes, erros tipados, proteção de `.drivesync`, concorrência e escopo atual |
| `download/stream.rs` | Cópia limitada, hash incremental, detecção de truncamento e arquivo alterado | Streams assíncronos do Provider, opções de execução, algoritmo de checksum explícito |
| `upload/local.rs` | Snapshot estável e verificação antes/durante envio | Staging privado atual, cancelamento, ownership e recuperação sem adotar arquivo substituído |
| `upload/execution.rs` | Máquina de estados de envio e reconciliação por ID | Extrair execução de uma chamada; retirar scan, planejamento, seleção de itens e confirmação de baseline |
| `state/uploads.rs`, `state/downloads.rs` | Invariantes de intenção durável, fases e recuperação | Novo contrato pelo Inventory; não copiar SQL/esquema antigo nem criar migração |
| `download/execution/updates.rs` | Preservação da versão anterior e testes de recuperação | Redesenhar publicação/identidade e explicitar limites de atomicidade |
| Suítes de upload/download | Cenários de falhas e asserções independentes sobre bytes/IDs | Portar fixtures para fachada atual e HTTP simulado dos Providers; não copiar apenas resultados esperados |
| `upload/transport.rs`, `upload/provider.rs` | Regras de validação de sessão e resposta perdida como referência | Não incorporar transporte HTTP antigo; usar Providers e Accounts atuais |
| Planner, coordenador, inventário e baseline antigos | Somente referência histórica de invariantes | Não reincorporar: Analisador, Scanner, Detector, Inventory e Orchestrator atuais já possuem essas responsabilidades |

Não fazer cherry-pick amplo do motor arquivado. Extrair unidades pequenas com testes equivalentes, eliminando dependências do motor antigo.

## 5. Lacunas a fechar antes de prometer execução segura

1. **Identidade da execução e recuperação.** As requisições atuais não têm chave de idempotência, ID reservado nem referência de retomada. Após reinício, uma nova chamada idêntica não pode ser presumida a mesma execução — nem duas criações intencionais podem ser confundidas. Propor identificador técnico estável, sem `SyncItem` no Operador, ligado pelo coordenador à operação durável do Inventory. Persistir intenção, origem, destino, snapshot, ID remoto reservado, sessão, fase e verificação. SQL fica no Inventory; Operador continua sem fila ou serviço persistente próprio. Definir a porta técnica de checkpoint antes de codificar a recuperação.
2. **Origem estável.** Upload/download de criação não carregam precondições explícitas da origem. Fixar a evidência que vincula os bytes executados ao plano: identidade, versão/revisão e tamanho/hash quando disponíveis. Revalidar antes do efeito e da publicação. Não transferir silenciosamente uma revisão que apareceu depois do planejamento.
3. **Precondições remotas.** O Provider atual tem create/update resumable, consulta de sessão, download em range e leitura de revisão. Porém, na inspeção não há parâmetro/header condicional de escrita `If-Match`; ler versão e depois escrever deixa uma corrida. Antes de alterar Providers, apresentar essa limitação e a extensão mínima necessária. Verificar suporte real da API quando a implementação chegar; se não houver garantia aplicável, bloquear a mutação que depende dela. Não afirmar que consulta antes/depois impede perda de atualização.
4. **Sessão e revogação.** Reutilizar Accounts/Engine para obter sessão validada. O helper atual de observação cancela o future HTTP por `select!`; não transplantar essa semântica para mutações como prova de `NotApplied`. Depois do despacho, resposta perdida ou cancelamento pode significar `Unknown`.
5. **Publicação e identidade locais.** Distinguir ID do item lógico e identidade física do arquivo. Substituir por temporário pode trocar inode; registrar identidade realmente publicada e renovar a associação com evidência. Definir estratégia para atualização concorrente, preservação e recuperação, sem prometer compare-and-swap de pathname que o filesystem não oferece.
6. **Lixeira/restauração local.** Há contratos, mas não evidência aqui de implementação atual segura e completa. Definir backend, escopo, identidade recuperável, destino ocupado e comportamento entre filesystems. Sem capacidade, recusar. `delete` sempre recusa localmente, sem autenticação ou I/O.
7. **Documentação contraditória.** `docs/engine/operations/TESTS.md` ainda descreve `OPR-L-11/delete_local` como remoção efetiva. Corrigir esse critério antes dos testes funcionais para exigir recusa; a política da raiz e do contrato compilado prevalece.

Essas são decisões propostas para implementação, não alterações de contrato já aplicadas. A política atual dispensa retrocompatibilidade; isso não autoriza apagar bancos ou arquivos existentes. Formatos incompatíveis devem ser preservados e recusados.

## 6. Distribuição de responsabilidades proposta

```text
Detector / Scanner → Inventory → Analyzer → plano durável bloqueado
                                            ↓
                      coordenador revalida e autoriza um passo
                                            ↓
                        Operator → filesystem / Providers
                            ↓ resultado e evidências
                      Inventory → reobservação → Analyzer
```

O coordenador de execução, integrado ao Orchestrator em etapa posterior, cuida de elegibilidade, autorização, claim, exclusão mútua e dependências. O Analyzer continua decidindo; o Operator executa uma ação delimitada e reporta efeitos reais. O Inventory concentra SQL, checkpoints e transações. A GUI apenas acompanha e oferece escolhas explicitamente suportadas.

Resultado verificado não confirma baseline dentro do Operator. A integração registra o efeito, vincula a réplica realmente criada, reobserva os dois lados e confirma baseline somente com evidências coerentes. Se a observação divergir, preservar o resultado da execução e reanalisar sem fabricar sincronização.

## 7. Sequência de desenvolvimento

Cada entrega deve ser pequena, compilável, documentada e validada antes do commit. Não habilitar a execução automática durante as primeiras etapas.

| Etapa | Entrega e módulos | Critério de conclusão |
| --- | --- | --- |
| 1 — Fechar contratos de segurança | Operations, Inventory e documentação: ID de execução, checkpoints, evidência da origem, efeitos incertos, identidade após publicação; corrigir `delete_local` | Contratos compilam; testes demonstram que chamadas distintas não são confundidas e retry recupera a execução certa; lacunas dos Providers registradas |
| 2 — Primitivas locais | Biblioteca privada em Operations, extraída seletivamente do arquivo | Testes reais de symlink, raiz/pai substituídos, nomes em bytes, colisão, espaço insuficiente, concorrência e fsync; `delete` sem efeitos |
| 3 — Composição e sessão | Factory concreta e especialista Google, com Accounts/Engine | Provedor vem da conta persistida; isolamento de contas e raízes; testes de revogação; nenhuma credencial em request, log ou checkpoint público |
| 4 — Download de criação | Stream do Provider → staging verificado → publicação exclusiva | Interrupção não publica parcial; resposta incompleta/hash divergente falham; recuperação após publicação sem confirmação não duplica nem adota homônimo |
| 5 — Upload de criação | Snapshot privado, ID remoto reservado, sessão e offset confirmados, checkpoint durável | Repetir falhas arquivadas: interrupção, offline, sessão expirada, resposta final perdida, origem alterada e reinício; exatamente um recurso por execução |
| 6 — Criações e metadados | Arquivo/pasta, rename e move locais/remotos; criar diretórios não faz scan recursivo | Colisão, ciclo, conta errada, capacidade ausente e resposta perdida cobertos; preservar identidade quando aplicável; mutação remota dependente de condição fica bloqueada sem suporte |
| 7 — Atualização de conteúdo | Local com preservação/publicação definida; remota no mesmo file ID com proteção concorrente efetiva | Origem/alvo alterados durante operação não provocam sobrescrita silenciosa; recuperar falhas em cada fronteira; garantir versões preservadas conforme estratégia escolhida |
| 8 — Lixeira e restauração | Backend local seguro e metadados remotos via Providers | Sem delete definitivo; autorização é externa ao Operator; restauração não ocupa destino de terceiros; descendentes/capacidade revalidados |
| 9 — Integração controlada | Inventory e Orchestrator: elegibilidade transacional, claim e resultado durável | Plano obsoleto não executa; `Blocked` não é liberado por retry genérico; efeito incerto entra em verificação; crash entre efeito e gravação recupera sem repetição cega |
| 10 — Aceitação real | Fixtures isoladas, conta/pasta explicitamente autorizadas, acompanhamento na GUI | Upload/download e recuperação observados no serviço real; evidência registrada; depois validar o Flatpak instalado e liberar automação conforme critérios aprovados |

Etapas 4 e 5 são o primeiro marco útil: transferência de criação segura nos dois sentidos, sem atualização automática de arquivos existentes e sem liberar a fila inteira. Download por range pode ser uma entrega posterior: só juntar blocos se todos pertencerem à mesma revisão verificável. Recomeçar o download com segurança é aceitável no primeiro marco; upload retomável deve preservar a garantia já existente no código arquivado.

## 8. Validação obrigatória da implementação

- Portar os cenários relevantes, não depender apenas de mocks da fachada. Usar filesystem temporário real e HTTP local com respostas truncadas, atrasadas, perdidas e aplicadas antes da falha.
- Injetar falha antes/depois de persistir intenção, iniciar sessão, enviar bloco, verificar staging, publicar destino e gravar confirmação; reabrir Inventory e reconstruir executor. Acrescentar testes com processo encerrado para distinguir recuperação em disco de exceção simulada.
- Exercitar dois executores sobre a mesma execução/destino, cancelamento em cada fase, reporter ausente/lento e perda do consumidor de progresso.
- Distinguir bytes enviados nesta chamada do offset acumulado confirmado; retomada não deve inventar bytes transferidos ou concluir pela telemetria.
- Não transformar checksum calculado apenas sobre os próprios bytes recebidos em prova independente de integridade remota. Usar tamanho/hash/revisão de referência qualificados; ausência conserva `NotAvailable`/bloqueio aplicável.
- Executar Rust exclusivamente no SDK GNOME 50 e extensão Rust definidos em `flatpak/com.rtosta.DriveSync.Devel.yml`: fmt, Clippy com warnings negados, testes relevantes/workspace, rustdoc e doctests. Registrar comandos e resultados efetivos por entrega.
- Separar teste sintético, rede real, reinício de processo e queda de energia. Só declarar cada garantia com sua evidência; SDK e HTTP simulado não comprovam o Flatpak instalado.

## Recomendação

Reaproveitar primeiro primitivas locais, snapshot, máquina de recuperação do upload de criação e cenários de testes. Manter o transporte atual dos Providers e as fronteiras da reconstrução. Não reutilizar o coordenador antigo, associação por nome de pastas, SQL legado ou confirmação de baseline dentro da transferência. O maior trabalho novo é conectar recuperação durável e execução autorizada aos planos atuais, além de proteger atualização de conteúdo existente.
