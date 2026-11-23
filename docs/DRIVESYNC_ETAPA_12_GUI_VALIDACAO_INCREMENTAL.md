# DriveSync — Etapa 12: Componentes da GUI e validação incremental por evento

**Status:** especificação para implementação e homologação  
**Base:** `main`, documentação `ORCHESTRATOR.md`, `OPERATIONS.md`, `FUNCTIONAL_CORRECTION_STAGE_8.md`, `FUNCTIONAL_CORRECTION_STAGE_9.md` e `FUNCTIONAL_CORRECTION_STAGE_11.md` (consultadas em 23/11/2026).  
**Plataforma:** Rust, GTK4, libadwaita, GNOME e Flatpak.

## 1. Contexto e objetivo

A Engine já implementa observação, análise, execução, persistência e recuperação de várias operações. As etapas anteriores registram validações automatizadas e reais, mas a experiência da aplicação instalada ainda apresenta lacunas: uma edição remota de DOCX não se refletiu localmente no teste observado pelo usuário; uma divergência proposital não resultou na apresentação esperada da resolução de conflito; a projeção de arquivos Google Workspace ainda usa `.desktop`, inadequado para a experiência desejada no GNOME.

Esta etapa **não reconstrói a arquitetura**. Seu objetivo é construir primeiro os componentes reutilizáveis da interface e, em seguida, **validar cada evento funcional da Engine junto com sua integração visual**, em uma única atividade por evento. A etapa só avança quando o evento atual possui evidência de execução correta, feedback correto e correções concluídas ou bloqueios formalmente identificados.

**Decisões de escopo:**

- O seletor de contas já existe na sidebar GNOME: **não duplicar** na página inicial.
- Não criar histórico de atividades, estatísticas ou uma fila visual completa nesta etapa.
- A GUI não decide sincronização, não consulta o Inventory diretamente e não executa operações internas.
- Os componentes têm **estado, API e sinais próprios**, como componentes controláveis de uma UI declarativa. São implementados em GTK/libadwaita, não em React.
- Os componentes **são construídos antes** das validações por evento, com dados de teste; seus contratos e estados poderão evoluir conforme a integração real demonstrar necessidades.
- O feedback deve derivar de fatos publicados pela Engine. Não inventar porcentagens, etapas, sucesso ou conflitos.

## 2. Arquitetura e limites de responsabilidade

```text
                         Sidebar GNOME existente
                         (seleção da conta ativa)
                                   │
                                   ▼
                         DashboardController
                          │             ▲
              ViewModels  │             │  intenções de UI
                          ▼             │
       ┌───────────────────────────────────────────┐
       │ SyncStatus        SyncActivity            │
       │ SyncAttention     ConflictResolver        │
       │ SyncIssueDialog                           │
       └───────────────────────────────────────────┘
                          ▲             │
                 modelos  │             │ comandos tipados
                          │             ▼
                     EngineAdapter
                          ▲             │
                   status │             │ chamadas públicas
                          │             ▼
                     API pública Engine
                          │
             Detectors → Context → Analyzer
                          │
                    Operations / Inventory
```

### Responsabilidades

| Elemento | Responsabilidade | Proibido |
|---|---|---|
| Engine | Detectar, analisar, executar, persistir, recuperar, auditar e publicar fatos | Conhecer widgets GTK |
| EngineAdapter | Assinar API pública, mapear modelos e traduzir ações autorizadas | Reimplementar decisões do Analyzer |
| DashboardController | Gerenciar conta selecionada, ciclo de vida e composição dos componentes | Acessar Inventory diretamente |
| Componentes | Renderizar estado próprio e emitir intenções por sinais/callbacks | Importar tipos privados da Engine, executar I/O de sincronização |

Os modelos da GUI pertencem ao módulo de apresentação e não reutilizam estruturas internas da Engine. A interface pública de cada componente deve ser testável sem login, rede, Inventory ou Engine. Os exemplos de API abaixo são **contratos propostos**, não assinaturas já existentes no repositório; a implementação deve seguir convenções reais de `glib::ObjectSubclass`, propriedades, sinais e ownership GTK.

### Modelo de atualização

- O Adapter recebe snapshots/atualizações da Engine e produz ViewModels imutáveis ou valores equivalentes.
- Os componentes expõem setters/propriedades observáveis e sinais de intenção, em vez de depender de referências ao Adapter.
- O Controller conecta e desconecta assinaturas ao trocar de conta ou destruir widgets; eventos atrasados da conta anterior não podem atualizar a atual.
- A thread principal GLib é responsável por atualizar widgets; operações de I/O ficam fora dela.
- `SynchronizationSubscription` é um fluxo de **último estado, sujeito a coalescência**, não um log completo de transições. Para fases ou progresso em bytes, ampliar a API pública somente quando o evento correspondente exigir.
- A assinatura de status existente informa atividade, item atual, contagem de operações, bloqueios e ciclo de vida; não presumir que já informa bytes transferidos.
- `Idle` não comprova conclusão: **“Tudo sincronizado” exige resultado positivo de `Engine::synchronization_audit`** para o escopo e horizonte observados. Um novo evento invalida a apresentação anterior de conclusão até nova comprovação.

## 3. Contratos compartilhados da camada de apresentação

Os tipos são indicativos; nomes, ownership e serialização serão adaptados ao código GTK existente.

```rust
// Modelos da GUI: não importar orchestrator::*, analyzer::*, inventory::*.
struct ScopeViewId(String); // identidade opaca da conta/escopo da GUI
struct ItemViewId(String);
struct OperationViewId(String);

// Nunca usar apenas o nome de arquivo como identidade.
enum SyncDirection { LocalToRemote, RemoteToLocal, LocalOnly, RemoteOnly, None }
enum ActivityKind {
    Download, Upload, CreateLocal, CreateRemote,
    UpdateLocal, UpdateRemote, Rename, Move, Trash, Recover, Verify,
}
enum ActivityPhase { Preparing, Running, Verifying, Completing, Failed }
enum ProgressView {
    Indeterminate,
    Bytes { transferred: u64, total: u64 },
    Steps { completed: u64, total: u64 },
}
```

`Bytes` e `Steps` têm semânticas distintas: **não mostrar “64% baixado” quando só houver 64% das operações de um plano**. Valores inválidos, total zero ou ausência de medição devem ser tratados explicitamente. Textos localizados e descrições acessíveis fazem parte do contrato visual.

## 4. Componentes — construir antes de integrar os eventos

### 4.1 `SyncStatus` — estado geral

**Objetivo:** informar a situação da conta selecionada de forma sintética e contínua.

**Exibe:** ícone, título e descrição curta. Estados visuais: `Initializing`, `Scanning`, `Synchronizing`, `Synced`, `Attention`, `Offline`, `Failed`, `Stopped`. `Synced` só pode ser informado pelo Controller após auditoria positiva; um bloqueio ativo impede a afirmação de convergência.

**API própria (conceitual):**

```rust
SyncStatus::new()
SyncStatus::set_state(StatusView)
SyncStatus::state() -> StatusView
SyncStatus::reset()
```

**Estados internos:** estado atual e apresentação derivada; não guarda credenciais, Inventory ou decisão de sincronização. **Sem comandos de Engine.**

**Critérios de aceite isolado:** renderizar todos os estados com fixtures, trocar estados repetidamente sem widgets duplicados, conferir acessibilidade e localização, não mostrar “sincronizado” a partir de `Idle` isoladamente.

### 4.2 `SyncActivity` — operação atual

**Objetivo:** informar **o que está sendo feito agora**, sobre qual item e com qual progresso efetivo.

**Exibe:** tipo da ação, nome do arquivo/diretório, direção, fase atual e barra de progresso apenas se mensurável. Exemplos: “Baixando Relatorio.docx”, “Verificando conteúdo”, “Renomeando pasta”. Oculta a atividade quando não houver operação, sem presumir que o escopo está sincronizado.

**API própria:**

```rust
SyncActivity::new()
SyncActivity::set_activity(ActivityView)
SyncActivity::update_progress(ProgressView)
SyncActivity::clear()
SyncActivity::state() -> Option<ActivityView>
```

`ActivityView` contém `operation_id`, `item_id`, nome de exibição, tipo, direção, fase e progresso opcional. Uma atualização de progresso deve corresponder à mesma operação; rejeitar dados atrasados de outra operação ou conta. A UI não deve calcular progresso a partir do relógio.

**Critérios de aceite isolado:** download, upload, ação sem bytes, progresso indeterminado, mudança de operação, cancelamento visual, erro e finalização; nomes longos/Unicode; ausência de saltos falsos para 100%.

### 4.3 `SyncAttention` — atenção necessária

**Objetivo:** reunir somente problemas atuais que exigem conhecimento ou intervenção do usuário.

**Exibe:** grupos concisos de conflitos, operações bloqueadas, falhas relevantes e tipos não suportados; quantidade e resumo. Não é um histórico nem uma fila. Ocultar quando vazio.

**API própria:**

```rust
SyncAttention::new()
SyncAttention::set_items(Vec<AttentionItemView>)
SyncAttention::clear()
SyncAttention::connect_activated(callback: Fn(AttentionAction))
```

`AttentionItemView` inclui ID opaco, tipo, severidade, título, resumo e escopo. Ao clicar, emite `AttentionAction::OpenConflictList` ou `AttentionAction::OpenIssue(id)`; o Controller abre o diálogo adequado. **Não chama a Engine.**

**Critérios de aceite isolado:** zero/um/vários problemas; agrupamento sem duplicação; remoção de problema resolvido; clique correto; persistência visual após troca de snapshot; acessibilidade.

### 4.4 `ConflictResolver` — escolha entre versões

**Objetivo:** apresentar um conflito persistido e coletar uma escolha explícita e revisável.

**Exibe:** nome e localização, atributos divergentes, versão do computador e do Drive, metadados disponíveis (sem inventar datas/tamanhos), aviso de preservação das versões e ações `KeepLocal`, `KeepRemote`, `Decidir depois`.

**Estados próprios:** `Loading`, `Reviewing`, `Confirming`, `Submitting`, `Applying`, `Resolved`, `Stale`, `Failed`. `Submitting` não significa resolvido; a remoção do conflito depende de convergência confirmada pela Engine.

**API própria:**

```rust
ConflictResolver::new()
ConflictResolver::set_conflict(ConflictView)
ConflictResolver::set_state(ConflictResolverState)
ConflictResolver::connect_choice(callback: Fn(ConflictChoiceRequest))
ConflictResolver::connect_dismissed(callback: Fn())
```

A intenção contém ID opaco do conflito, revisão exibida e escolha. O Controller/Adapter encaminha à API pública (`list_conflicts`, `resolve_conflict`), valida o resultado e atualiza o diálogo. Mudança da revisão exige nova revisão, nunca reaplicação silenciosa. **“Manter ambos” não integra o contrato atual** e não deve ser exibido sem uma operação de Engine própria, incluindo regras de identidade, cópia e colisão.

**Critérios de aceite isolado:** duas versões, atributos distintos, metadados ausentes, escolha/confirmar, envio duplicado bloqueado, revisão obsoleta, adiar, erro, resolução efetivamente confirmada. O componente não deve prometer preservação concluída antes de receber evidência da Engine.

### 4.5 `SyncIssueDialog` — diagnóstico de bloqueios

**Objetivo:** detalhar um problema técnico sem tratá-lo como conflito de versões.

**Exibe:** item afetado, tipo de problema, descrição compreensível, estado de segurança e orientação quando disponível. Exemplos: falta de acesso, tipo não suportado, operação incerta, falha de lixeira ou raiz portal não admitida.

**API própria:**

```rust
SyncIssueDialog::new()
SyncIssueDialog::set_issue(IssueView)
SyncIssueDialog::present()
SyncIssueDialog::close()
```

Inicialmente informativo. Não apresentar `Forçar`, exclusão permanente ou `Tentar novamente` sem contrato real, seguro e autorizado da Engine.

**Critérios de aceite isolado:** diferentes severidades, mensagem longa, orientação ausente, abertura/fechamento, dados não sensíveis, acessibilidade.

### 4.6 `DashboardController` e `EngineAdapter` — integração, não widgets de negócio

**Controller:** compõe os cinco componentes na página inicial; recebe a seleção da sidebar existente; cria e encerra assinaturas; evita callbacks de conta anterior; encaminha intenções e apresenta diálogos. **Não cria seletor de contas adicional.**

**Adapter:** depende apenas da API pública estável da Engine, mapeia status/auditoria/conflitos/erros para ViewModels e envia comandos autorizados. Deve oferecer uma interface substituível por `FakeSyncBackend` em testes de GUI. O Controller não deve conhecer `Engine` concretamente.

```rust
// Interface ilustrativa na fronteira de apresentação.
trait SyncBackend {
    // Assinaturas/snapshots mapeados para ViewModels.
    // Consultar auditoria e conflitos do escopo selecionado.
    // Submeter escolha de conflito com ID e revisão.
    // Não expor operações privadas do Analyzer/Operations.
}
```

Não antecipar endpoints ou semânticas que a Engine ainda não possui. Documentar lacunas e ampliar a API pública na atividade funcional que necessitar delas.

## 5. Layout da página inicial

A sidebar GNOME existente controla a conta ativa. A área principal contém:

1. **`SyncStatus`** — estado geral, sempre visível.
2. **`SyncActivity`** — atividade atual, visível somente enquanto houver operação real.
3. **`SyncAttention`** — problemas atuais, visível somente quando houver itens.

`ConflictResolver` e `SyncIssueDialog` são diálogos abertos a partir de `SyncAttention`; não ocupam permanentemente o dashboard. Não mostrar simultaneamente “Sincronizando” e “Tudo sincronizado”. Não exibir resumo estatístico, histórico ou duplicar a seleção de conta.

## 6. Estratégia de construção e validação

### Atividade inicial obrigatória — construção dos componentes

**Antes de qualquer validação de operações de arquivo:**

1. Inspecionar os widgets, controladores, modelos e sinais existentes; **reutilizar e adaptar** em vez de duplicar.
2. Criar os modelos próprios da GUI e contratos públicos de `SyncStatus`, `SyncActivity`, `SyncAttention`, `ConflictResolver`, `SyncIssueDialog`.
3. Construir os cinco componentes com fixtures determinísticas e `FakeSyncBackend` (sem Engine/Google).
4. Compor o dashboard mantendo a sidebar atual; conectar os sinais ao Controller, ainda sem ações reais.
5. Implementar ciclo de vida de conta, limpeza de listeners e isolamento de estados.
6. Validar isoladamente renderização, propriedades, sinais, responsividade, textos PT-BR, acessibilidade e ausência de bloqueio da thread GTK.
7. Registrar lacunas conhecidas da API da Engine, **sem antecipar implementação** de todos os eventos.

**Aceite:** todos os componentes podem ser criados, alimentados, atualizados e destruídos em testes sem inicializar a Engine; nenhum deles importa módulos privados da sincronização. O dashboard mantém as funcionalidades existentes.

### Atividades funcionais sequenciais — um evento de cada vez

**Para cada evento, executar integralmente este ciclo:**

1. Preparar fixture isolada com estado inicial conhecido e evidências de identidade/baseline.
2. Executar **uma ação principal** no filesystem ou Google Drive real.
3. Validar detecção → Context → Analyzer → Operations → Inventory, conforme aplicável.
4. Validar resultado independente nos dois lados: identidade, nome, localização, conteúdo, versão, tamanho e SHA-256 quando aplicável.
5. Verificar se a API pública fornece os dados necessários ao componente. Se não, ampliar **somente o contrato necessário para este evento**, mantendo compatibilidade com os consumidores existentes quando possível.
6. Integrar o evento ao `EngineAdapter`, Controller e componente(s) correspondente(s).
7. Validar o feedback real na aplicação GTK/Flatpak instalada: início, operação, progresso verdadeiro ou indeterminado, término, erro e atenção.
8. Corrigir falhas; repetir o cenário isolado e regressões direcionadas; registrar evidências e aceite **antes de iniciar o próximo evento**.

**Sequência sugerida (atividades internas da Etapa 12, não novas etapas de projeto):**

| Ordem | Evento/ação isolada | Validação Engine | Integração GUI e aceite |
|---|---|---|---|
| 1 | Download de arquivo remoto existente | Transferência, SHA-256, versão e baseline | `SyncActivity` mostra item, direção, fase e progresso real; `SyncStatus` consistente |
| 2 | Upload de arquivo local existente | Upload, ID remoto, conteúdo e baseline | Mesmos componentes, direção inversa |
| 3 | Criação de arquivo remoto | Observação, registro de identidade, download e convergência | Informar criação e transferência, sem evento fictício |
| 4 | Criação de arquivo local | Observação, registro de identidade, upload e convergência | Informar criação e envio |
| 5 | Edição de conteúdo remoto (DOCX) | **Reproduzir falha observada**: atualizar local e verificar bytes | Mostrar atualização remota → local e conclusão real |
| 6 | Edição de conteúdo local | Atualização remota condicional e integridade | Mostrar atualização local → remoto |
| 7 | Renomeação local e remota | Identidade preservada e sem reupload desnecessário | Nome e direção corretos |
| 8 | Movimentação local e remota | Pais, identidade, escopo e hierarquia | Pasta/arquivo movimentado corretamente |
| 9 | Criação e estrutura de diretórios | Dependências pai-filho e pastas vazias | Informar operação sem progresso em bytes fictício |
| 10 | Divergência concorrente | **Reproduzir conflito não exibido**: evidência, persistência e listagem pública | `SyncAttention` sinaliza; `ConflictResolver` abre com revisão correta |
| 11 | Escolha de versão em conflito | `KeepLocal` e `KeepRemote`, arquivo original preservado, convergência | Estado `Submitting` → `Applying` → `Resolved`; conflito some apenas após confirmação |
| 12 | Exclusão autorizada para lixeira | Opt-in, recibos e recusa segura; investigar falha Flatpak da etapa 11 | Informar execução, bloqueio ou recuperação sem promessa falsa |
| 13 | Documento Google Workspace | Trocar projeção `.desktop` por `.html`; validar identidade, nome e integridade | Mostrar criação/atualização de atalho sem confundir com upload de documento |
| 14 | Interrupção e recuperação | Checkpoints, efeitos incertos, ausência de replay cego | Estado de recuperação ou atenção coerente |

**Observação:** testes de renomeação e movimentação devem ser feitos separadamente por direção, ainda que documentados na mesma linha de planejamento. Não combinar múltiplas ações no mesmo cenário de aceite. A ordem pode ser alterada somente por dependência demonstrada, registrada no relatório.

## 7. Política de projeção Google Workspace — alteração explícita

A etapa 9 classifica Docs, Sheets, Slides, Drawings e Forms como projeções derivadas somente leitura em `.desktop`. **Alterar a representação para `.html`**, com página mínima que abre o documento original no navegador usando ID remoto codificado em URL HTTPS fixa.

- Gerar HTML seguro, com escaping correto, sem scripts, recursos externos desnecessários ou execução de comandos.
- Criar com permissão somente leitura (`0444`, quando suportada); reconhecer que o proprietário ainda pode alterar permissões ou substituir o arquivo.
- Tratar bytes HTML como **artefato derivado**, não como conteúdo nativo do Google. Edição, substituição, renomeação ou remoção local não autoriza escrita remota.
- Preservar `ItemId`, `RemoteId`, associação de pais e a semântica do baseline de projeção; revisar os nomes de tipos internos somente se necessário.
- Validar colisões de nomes (`arquivo.html`), limites UTF-8, escape, URL, atualização de nome/localização e retomada após falha.
- **Não converter DOCX/PDF em Google Workspace** e não implementar edição offline de documentos nativos.
- Não assumir migração automática de projeções antigas: definir comportamento seguro para fixtures atuais sem apagar dados do usuário nem introduzir retrocompatibilidade desnecessária.

## 8. Casos de falha e limites conhecidos

- **DOCX remoto não atualizado localmente:** identificar em qual fronteira a atualização deixa de avançar; não presumir que é erro de GUI ou do detector.
- **Conflito não apresentado:** separar (a) ausência de detecção, (b) ausência de persistência, (c) ausência na API pública, (d) falha de assinatura/controller e (e) diálogo não aberto. A etapa 8 documenta `list_conflicts`, `resolve_conflict` e botões reais; verificar integração instalada, não reconstruir automaticamente.
- **Lixeira no Flatpak:** etapa 11 registrou recusa segura de `RemoveLocal`; GIO portal funcionou separadamente, sem equivalência comprovada com o recibo privado da Engine. Não trocar o mecanismo sem preservar garantias de recuperação.
- **Raiz de documentos via portal FUSE:** etapa 11 registrou recusa de admissão para escrita. Exibir diagnóstico claro; suporte pleno requer decisão técnica e teste próprio.
- **Múltiplas contas reais:** isolamento foi testado com identidades controladas, não com duas contas Google reais simultâneas. Não declarar homologação além da evidência.
- **Progresso:** a assinatura atual pode coalescer estados e informar progresso por operações; medir bytes exige fonte real de telemetria. Não confundir bytes, passos ou contagem de arquivos.
- **Sem exclusão permanente:** toda exclusão depende da política explícita e de lixeira recuperável; falha segura preserva o original.

## 9. Testes e evidências

### Testes de componentes (antes da integração)

- Construção/destruição repetida, propriedades, sinais, estados vazios, estados inválidos e nomes longos.
- Troca de conta durante atualização assíncrona; callbacks antigos ignorados.
- Fixtures de progresso conhecido, desconhecido e sem total; nenhuma porcentagem fabricada.
- `ConflictResolver` com revisão obsoleta, duplo clique, adiamento, falha e resolução real simulada pelo backend falso.
- Localização PT-BR, teclado, foco, leitores de tela e redimensionamento de janela.

### Testes por evento (durante integração)

- Unitários direcionados do módulo alterado.
- Integração real Engine/Google para operação isolada, quando necessária e autorizada.
- GUI real GTK no Flatpak Devel instalado; teste de sinais **não substitui** inspeção da apresentação e fluxo real do usuário.
- Leitura independente do provedor e comparação dos bytes/metadados; logs, capturas e IDs de execução.
- Teste de reinício somente quando relevante ao evento atual.
- Regressão direcionada para contratos e componentes já integrados; suite completa em marcos de estabilidade e aceite final, não após cada pequena mudança.

**Segurança de testes:** utilizar apenas pastas de fixture isoladas e conta autorizada, nunca arquivos pessoais ou escopos não selecionados; preservar credenciais, configurações e fixtures anteriores. Testes destrutivos somente em fixtures e com política explícita.

## 10. Modelo obrigatório de registro por atividade

```markdown
### Evento: <nome>
- Estado inicial / fixture / escopo:
- Ação única executada:
- Resultado esperado na Engine:
- Resultado esperado na GUI:
- API pública disponível / lacuna identificada:
- Resultado observado (local, remoto, Inventory, GUI):
- Diagnóstico por fronteira:
- Correção mínima aplicada:
- Testes direcionados e regressões:
- Evidências (logs, hashes, IDs, capturas):
- Limitações remanescentes:
- Resultado: APROVADO | REPROVADO | BLOQUEADO
```

**Não declarar APROVADO** se apenas os testes da Engine passarem e a GUI não apresentar o evento corretamente, ou vice-versa. Falha parcial deve permanecer explícita; não usar estado `Idle` como aceite.

## 11. Critérios globais de conclusão

1. Os cinco componentes são reutilizáveis, possuem estado/API próprios, são testáveis com backend falso e não importam estruturas internas da Engine.
2. A página inicial usa a sidebar existente e apresenta apenas estado geral, operação atual e atenção; diálogos são contextuais.
3. Cada evento planejado foi validado funcionalmente e integrado à GUI **na mesma atividade**, com evidências separadas de Engine e interface.
4. Alterações remotas de DOCX refletem corretamente no arquivo local; divergências reais geram conflitos visíveis e resolvíveis.
5. Google Workspace usa `.html` derivado e protegido, sem propagação de alterações locais do atalho.
6. A interface nunca informa conclusão sem auditoria positiva nem apresenta progresso sem medição correspondente.
7. Nenhuma correção enfraquece isolamento por conta, identidade, precondições, recuperação ou política de exclusão segura.
8. Os limites não resolvidos são registrados com comportamento seguro e estado visual correspondente; não são apresentados como homologados.
9. Ao fim das atividades, executar regressão integrada final e verificar o Flatpak instalado, sem substituir os aceites isolados.

## 12. Ordem de execução resumida

**Primeiro:** inspecionar GUI existente → construir componentes e contratos → testar isoladamente com fixtures → integrar composição do dashboard e backend substituível.

**Depois, sequencialmente:** selecionar evento → validar Engine → complementar API pública apenas se necessário → conectar Adapter/Controller/componentes → validar GUI real → corrigir → registrar aceite → próximo evento.

**Princípio:** *componentes antes; integração e validação juntas, evento por evento; nenhuma antecipação funcional de eventos futuros.*

## Referências do repositório

- [`docs/ORCHESTRATOR.md`](https://github.com/rafatosta/drivesync/blob/refactor/engine/docs/ORCHESTRATOR.md) — API pública, assinatura de status e limites de `Idle`.
- [`docs/OPERATIONS.md`](https://github.com/rafatosta/drivesync/blob/refactor/engine/docs/OPERATIONS.md) — operações, recuperação e segurança.
- [`docs/FUNCTIONAL_CORRECTION_STAGE_8.md`](https://github.com/rafatosta/drivesync/blob/refactor/engine/docs/FUNCTIONAL_CORRECTION_STAGE_8.md) — conflitos persistentes e fluxo GTK.
- [`docs/FUNCTIONAL_CORRECTION_STAGE_9.md`](https://github.com/rafatosta/drivesync/blob/refactor/engine/docs/FUNCTIONAL_CORRECTION_STAGE_9.md) — política atual de projeções.
- [`docs/FUNCTIONAL_CORRECTION_STAGE_11.md`](https://github.com/rafatosta/drivesync/blob/refactor/engine/docs/FUNCTIONAL_CORRECTION_STAGE_11.md) — aceite instalado e limitações verificadas.
