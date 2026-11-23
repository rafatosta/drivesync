# DriveSync — Etapa 12: prompts para Codex

## Atividade prévia obrigatória — Construir os componentes

> Leia `docs/DRIVESYNC_ETAPA_12_PROMPTS_CODEX.md`, as instruções do repositório e os componentes GTK/libadwaita atuais. Execute **somente a atividade inicial obrigatória da seção 6**. Construa ou adapte `SyncStatus`, `SyncActivity`, `SyncAttention`, `ConflictResolver` e `SyncIssueDialog` com estados, APIs e sinais próprios, ViewModels independentes da Engine, `DashboardController` e interface substituível para `EngineAdapter`. Reutilize a sidebar de contas existente; não crie histórico, estatísticas nem novo seletor. Use fixtures determinísticas e backend falso; nenhum widget deve importar tipos internos da Engine ou chamar operações de sincronização. Teste construção/destruição, transições de estado, sinais, troca de conta, callbacks obsoletos, progresso desconhecido, foco e responsividade. **Não integre ainda eventos reais.** Registre resultados e limitações no documento, execute verificações pertinentes e faça commit em inglês apenas após validação. Não avance automaticamente.

## Regras comuns para os prompts 1–14

Cada prompt abaixo é **autônomo** e inclui as seguintes exigências: ler `docs/DRIVESYNC_ETAPA_12_PROMPTS_CODEX.md` e as instruções do repositório; examinar o estado real do código e contratos; executar somente a atividade indicada; usar fixtures isoladas e conta Google explicitamente autorizada; não tocar arquivos pessoais nem fazer exclusão permanente; verificar Engine (detecção → contexto → análise → operação → inventário/baseline, conforme aplicável), API pública, Adapter, Controller e GUI instalada; adicionar à API somente dados necessários ao evento; nunca inventar progresso, sucesso ou eventos; testar os dois lados e regressões anteriores; documentar evidências e resultado `APROVADO`, `REPROVADO` ou `BLOQUEADO` conforme seção 10; fazer commit em inglês após validação e **não iniciar a atividade seguinte**. Se faltarem acesso real, autorização ou condições de teste, marcar `BLOQUEADO` em vez de simular homologação.

## 1 — Download de arquivo remoto existente

> Execute exclusivamente a **atividade 1 — download de arquivo remoto existente** da Etapa 12, obedecendo às regras comuns do documento. Prepare um arquivo binário remoto já existente em fixture isolada e confirme que ele ainda não existe localmente. Verifique detecção, plano, download, identidade, revisão/versão, SHA-256 dos bytes e baseline confirmado; confira por leitura independente no Google Drive e filesystem. Integre o evento ao `EngineAdapter`, `SyncActivity` e `SyncStatus`: nome, direção Drive → computador, fase e progresso em bytes **somente se medido pela Engine**; caso contrário, progresso indeterminado. `Tudo sincronizado` exige auditoria positiva, não apenas fila vazia. Valide na GUI GTK/Flatpak instalada, inclusive erro e conclusão. Corrija falhas no escopo, registre evidências e commit; não avance.

## 2 — Upload de arquivo local existente

> Execute exclusivamente a **atividade 2 — upload de arquivo local existente**. Parta de um arquivo local binário em fixture isolada, ainda ausente no escopo remoto. Verifique observação, upload, criação e persistência do ID remoto, bytes/SHA-256, revisão e baseline; confira o conteúdo diretamente no provedor. Reutilize `SyncActivity` e `SyncStatus` da atividade 1, agora com direção computador → Drive, sem duplicar widgets nem lógica de apresentação. Mostre progresso real apenas quando houver telemetria; valide erro, recuperação e conclusão com auditoria positiva. Execute teste integrado na aplicação instalada, regressão do download, registro de evidências e commit. Não avance.

## 3 — Criação de arquivo remoto

> Execute exclusivamente a **atividade 3 — criação de arquivo remoto**. Com sincronização previamente estabelecida, crie **um** arquivo novo diretamente no Google Drive de teste. Valide descoberta, identidade remota, vínculo de pais, download, conteúdo/SHA-256 e baseline; confirme que não houve criação duplicada nem associação incorreta. Na GUI, mostre a criação observada e a transferência efetivamente executada, diferenciando eventos comprovados de fases internas; não invente dois eventos quando a API só comprova um. Reutilize componentes e adapte somente contratos necessários. Teste no Flatpak instalado, regressões e documente evidências antes do commit. Não avance.

## 4 — Criação de arquivo local

> Execute exclusivamente a **atividade 4 — criação de arquivo local**. Após convergência inicial, crie **um** arquivo novo no diretório local de teste. Verifique detector, identidade local, operação de criação remota, ID gerado, conteúdo/SHA-256, baseline e ausência de duplicatas. Integre à GUI a criação observada e o envio real, com direção computador → Drive, sem fabricar progresso ou sucesso. Confirme o resultado por leitura independente e auditoria, execute regressões dos eventos anteriores, registre evidências e faça commit. Não avance.

## 5 — Edição de conteúdo remoto (DOCX)

> Execute exclusivamente a **atividade 5 — edição remota de DOCX**. **Reproduza primeiro a falha relatada**: editar no Google Drive um `.docx` já sincronizado e observar que a cópia local não atualiza. Use um DOCX binário, sem convertê-lo em Google Workspace. Instrumente as fronteiras detector remoto → contexto → Analyzer → Operations → Inventory → API pública → GUI para localizar a causa, sem presumir o módulo defeituoso. Corrija o mínimo necessário para atualizar bytes locais, preservar identidade, revisar baseline e confirmar SHA-256 contra leitura remota independente. `SyncActivity` deve mostrar atualização Drive → computador e fases verdadeiras; `SyncStatus` só conclui após auditoria. Inclua teste de regressão que falhe antes da correção, teste no Flatpak instalado, evidências e commit. Não avance.

## 6 — Edição de conteúdo local

> Execute exclusivamente a **atividade 6 — edição de conteúdo local**. Edite os bytes de um arquivo já sincronizado no diretório de teste, sem mudar nome ou caminho. Valide detecção de alteração, precondições da revisão remota, atualização condicional, integridade SHA-256, manutenção do ID e novo baseline; confirme no provedor por leitura independente. Mostre na GUI a atualização computador → Drive, sem confundi-la com criação ou novo upload de identidade. Teste falha de precondição de forma segura, preservando dados e encaminhando eventual conflito ao fluxo existente. Rode testes instalados, regressões, evidências e commit. Não avance.

## 7 — Renomeação local e remota

> Execute exclusivamente a **atividade 7 — renomeação**. Faça **dois cenários separados**, com fixtures independentes: (a) renomear localmente um arquivo já sincronizado; (b) renomeá-lo remotamente. Em cada cenário, valide preservação de ID remoto, identidade lógica, pais, conteúdo, baseline e ausência de retransmissão desnecessária de bytes. Diferencie nome anterior/novo e origem da ação na API/GUI; `SyncActivity` não deve exibir progresso de transferência para alteração apenas de metadados. Confirme via provider e filesystem, teste no Flatpak instalado, regressões e registre evidências por direção. Commit somente após os dois cenários; não avance.

## 8 — Movimentação local e remota

> Execute exclusivamente a **atividade 8 — movimentação**. Em fixtures separadas, mova um arquivo entre duas pastas gerenciadas (a) localmente e (b) remotamente. Verifique identidade preservada, IDs de pais, caminho, escopo autorizado, hierarquia, baseline e ausência de reupload de conteúdo quando a API suporta movimento por metadados. Não misture movimento com renomeação no mesmo cenário. Integre a apresentação da origem, destino e direção reais na GUI; sem porcentagem fictícia. Teste recusas seguras para destino fora do escopo, sem afetar dados reais. Valide Flatpak, regressões, evidências e commit. Não avance.

## 9 — Criação e estrutura de diretórios

> Execute exclusivamente a **atividade 9 — diretórios**. Em cenários isolados, valide criação de pasta vazia local e remota e criação de subpasta com dependência pai-filho; não confunda ausência de conteúdo com exclusão. Verifique IDs, pais, caminhos, ordenação de operações, baseline e convergência, incluindo reinício se necessário. Na GUI, mostre criação de diretório e fase real, sem progresso em bytes e sem apresentar pasta vazia como download de arquivo. Teste aplicação instalada, regressões, evidências e commit. Não avance.

## 10 — Divergência concorrente

> Execute exclusivamente a **atividade 10 — divergência concorrente**. **Reproduza a falha observada de conflito não exibido**: partindo de baseline comum, produza alterações divergentes de conteúdo local e remoto antes da reconciliação. Investigue separadamente detecção, evidência do conflito, persistência, `Engine::list_conflicts`, assinatura pública, Adapter, Controller, `SyncAttention` e abertura de `ConflictResolver`. Corrija a fronteira efetivamente defeituosa, sem decidir automaticamente qual versão vence. Mostre duas versões/revisões com metadados apenas quando disponíveis; mantenha ambas intactas. Teste o fluxo real na GUI instalada, inclusive reabertura e troca de conta; registre hashes, IDs, capturas e regressões. Faça commit; **não resolva o conflito nesta atividade** nem avance.

## 11 — Escolha de versão em conflito

> Execute exclusivamente a **atividade 11 — resolução de conflito**. Prepare **dois conflitos independentes**: um para `KeepLocal`, outro para `KeepRemote`. Verifique a API real de `Engine::resolve_conflict`, identidade/revisão do conflito e proteção contra decisão obsoleta; preserve as duas versões originais conforme o mecanismo de arquivamento existente. Integre o diálogo `ConflictResolver` com estados `Reviewing`, `Confirming`, `Submitting`, `Applying`, `Resolved`, `Stale` e `Failed` conforme necessário; impedir duplo envio e manter o item em `SyncAttention` até confirmação real de convergência. Teste decisão adiada, revisão alterada, falha e os dois resultados na aplicação instalada; compare bytes e baseline. Não implemente `Manter ambos` sem contrato aprovado. Registre evidências e commit; não avance.

## 12 — Exclusão autorizada para lixeira

> Execute exclusivamente a **atividade 12 — exclusão para lixeira**. Releia os limites da etapa 11: `RemoveLocal` recusou com segurança no Flatpak, enquanto GIO portal funcionou isoladamente sem comprovar equivalência ao mecanismo privado da Engine. Em fixtures descartáveis, teste separadamente exclusão autorizada local e remota, opt-in, pré-condições, recibos recuperáveis e falha segura; **nunca faça exclusão permanente nem fallback destrutivo**. Diagnostique a falha instalada sem enfraquecer garantias de recuperação. Integre `SyncActivity`, `SyncAttention` e `SyncIssueDialog` para informar execução comprovada, recusa ou recuperação, sem prometer sucesso inexistente. Se não houver garantia técnica, mantenha bloqueado e registre `BLOQUEADO`. Evidências, regressões e commit apenas para correções validadas. Não avance.

## 13 — Documento Google Workspace

> Execute exclusivamente a **atividade 13 — projeção Google Workspace**. Examine `engine/src/projections/mod.rs` e os contratos atuais; substitua projeções `.desktop` de Docs, Sheets, Slides, Drawings e Forms por arquivos `.html` derivados, somente leitura, que abrem o ID original via URL HTTPS segura. Preserve `ItemId`, `RemoteId`, pais e baseline de projeção; escape corretamente HTML/URL e não use scripts externos. Teste nome, atualização, movimentação, colisão `.html`, permissão `0444` quando suportada, substituição/edição local maliciosa e reinício; **nunca propague mudanças do HTML como edição do documento remoto**. Não converta `.docx`/`.pdf` em Google Workspace. Na GUI, informe criação/atualização de atalho, não upload de conteúdo nativo. Não apague projeções antigas sem prova de propriedade. Valide no Flatpak, registre evidências e commit. Não avance.

## 14 — Interrupção e recuperação

> Execute exclusivamente a **atividade 14 — interrupção e recuperação**. Selecione operações previamente homologadas e injete interrupções controladas em checkpoints relevantes, usando apenas fixtures isoladas. Após reiniciar, verifique efeitos pendentes/incertos, revalidação de precondições, ausência de replay cego, preservação de bytes, identidade, baseline e reconciliação final. Não converta incerteza em sucesso nem em exclusão. Integre estados verdadeiros de recuperação e bloqueio ao `SyncStatus`, `SyncActivity`, `SyncAttention` e `SyncIssueDialog`, sem inventar sequência de eventos perdida por assinatura coalescida. Teste a aplicação GTK/Flatpak instalada, regressões integradas das atividades 1–13, auditoria final e limites conhecidos. Registre matriz de evidências, resultados e pendências; faça commit apenas após validação. Não avance para outra etapa do projeto.
