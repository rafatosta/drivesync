# DriveSync — MVP 0.1: sincronização unidirecional segura

> **Roteiro de implementação do projeto**  
> **Estado:** PLANEJADO — nenhum marco foi validado.  
> **Data da definição:** 20/10/2026.  
> **Entrada no projeto:** [README](../README.md) · [Arquitetura](architecture.md) · [Metodologia](development-methodology.md) · [Invariantes](invariants.md) · [Viabilidade](feasibility-validation.md).
>
> **Regra de execução:** as fases são sequenciais. A fase N+1 só começa quando a fase N possui evidências, critérios de saída atendidos e registro de conclusão. Evidência ou descoberta posterior pode reabrir fases anteriores, alterar decisões e exigir nova validação das dependentes.

## 0. Por que este MVP existe?

O produto final pretendido é um cliente GNOME de sincronização bidirecional, confiável e recuperável, para arquivos do Google Drive. **Este MVP não é esse produto completo.** Ele é a menor entrega utilizável que demonstra, com arquivos reais de teste, a cadeia essencial de autenticação → observação remota → estado persistente → transferência segura → atualização incremental → recuperação → execução no Flatpak.

**Hipótese a comprovar:** o DriveSync consegue manter uma cópia local **não destrutiva**, progressivamente atualizada, de uma pasta do Google Drive, depois de alterações e reinicializações, sem substituir arquivos locais divergentes nem interpretar indisponibilidade como exclusão.

Uma execução que somente abre a janela, faz login ou baixa um arquivo avulso **não** satisfaz o MVP. É necessário um fluxo integrado e repetível, com testes adversos e estado persistente.

### Como o usuário utilizará o MVP

1. Abrir o DriveSync no Fedora GNOME, pelo lançador/Builder/Flatpak de desenvolvimento.
2. Autorizar uma conta Google de teste via navegador do sistema.
3. Selecionar **uma pasta do Google Drive** e **uma pasta local**.
4. Iniciar sincronização somente **Google Drive → pasta local**.
5. Ver arquivos binários comuns e subpastas aparecerem completos e íntegros localmente.
6. Alterar ou acrescentar um arquivo remoto, solicitar/aguardar próximo ciclo e observar somente a alteração correspondente.
7. Ocultar a janela; o motor permanece ativo enquanto o processo estiver autorizado e em execução. Reabrir restaura a mesma janela e estado.
8. Interromper/reiniciar o processo e comprovar retomada sem perda, duplicação ou sobrescrita silenciosa.

**Nota de produto:** não anunciar bidirecionalidade no MVP. Mudanças locais **não** são enviadas para o Google Drive. A interface deve explicitar “Somente download / prévia técnica”.

## 1. Escopo fechado (o que DEVE funcionar)

- **1 conta Google e 1 raiz remota + 1 raiz local**, com pasta remota previamente existente.
- OAuth 2.0 de aplicativo desktop, navegador do sistema, PKCE, renovação de credencial quando concedida, armazenamento seguro; nenhum segredo em log.
- Consulta dos arquivos existentes para os quais o OAuth de teste conceder acesso. **Acesso a arquivos previamente existentes é um bloqueio de viabilidade**, não algo que o MVP deve fingir que consegue fazer.
- Inventário inicial de **arquivos binários comuns** (ex.: TXT, PDF, JPG, ZIP) e **pastas aninhadas**. Downloads em streaming, arquivos temporários e verificação de integridade quando o provedor oferecer metadados adequados.
- SQLite com mapeamento `remote_id ↔ local_path`, versão observada, estado de operação e cursor de mudanças. Nomes não são identidades.
- Processamento incremental pela Drive API (`changes`) com paginação e cursor persistido corretamente, complementado por reconciliação quando necessário; após reiniciar, não transferir conteúdo inalterado.
- Não sobrescrever arquivo preexistente não gerenciado, modificado localmente ou com versão desconhecida. Registrar conflito/bloqueio; **não exigir interface completa de resolução de conflitos**.
- Se arquivo remoto for removido, movido ou deixar de ser acessível: **nunca apagar automaticamente o local**; registrar evento/estado pendente, preservar bytes. Um novo caminho remoto válido pode criar novo arquivo local, sem exclusão da cópia antiga.
- Erros visíveis: credencial expirada/revogada, acesso negado, erro temporário/limite, arquivo não baixável, disco cheio, interrupção, conflito e estado inconsistente.
- Janela GTK4/Libadwaita simples: conta, pastas, conectar, iniciar/pausar, progresso/estado, conflitos ou itens ignorados e opção **Sair**. X oculta sem sair; reabrir exibe a instância existente.
- Execução e validação em **Flatpak de desenvolvimento no GNOME**. Solicitação opcional de início automático na sessão, se o portal e o consentimento permitirem; comportamento negado/indisponível deve ser claro.

## 2. Fora do MVP — sem bloquear a arquitetura futura

| Funcionalidade adiada | Preparação obrigatória desde o MVP |
|---|---|
| Upload e sincronização bidirecional | `RemoteProvider`, `SyncPlan`, `BaseVersion`, `queue` e `coordinator` não podem assumir fluxo unidirecional como regra permanente |
| Exclusão/propagação de exclusões | Registrar observações `ABSENT/UNKNOWN` e tombstones, mas nunca executar deleção no MVP |
| Resolução avançada de conflitos | Modelo e estados `Conflict` preservados; UI pode mostrar “ação manual necessária” |
| Google Docs/Sheets/Slides | Detectar MIME nativo e **ignorar explicitamente**, sem baixar conteúdo inválido ou exportar silenciosamente |
| Contas múltiplas, múltiplas raízes e sincronização seletiva | IDs de conta/raiz nos contratos e schema, mesmo que a UI limite a uma por vez |
| Shared Drives, atalhos, arquivos especiais, symlinks remotos | Reconhecer tipos não suportados; não simular arquivos regulares |
| Daemon separado, system tray e integração Nautilus/D-Bus | Manter biblioteca independente; `ipc` continua opcional e não é pré-requisito |
| Envio por blocos diferenciais, histórico complexo, Flathub público | Não introduzir abstrações ou permissões prematuras; seguir contratos e ADRs |

A versão final do produto permanece orientada à sincronização bidirecional; **a ausência de upload neste MVP é uma decisão de escopo, não uma decisão arquitetural definitiva**.

## 3. Restrições inegociáveis

1. Nunca sobrescrever sem conhecer o estado anterior e verificar que o arquivo continua seguro para substituição.
2. Nunca considerar erro `404/403`, timeout ou falha de token como prova suficiente de exclusão.
3. Nunca declarar `SYNCED` até bytes locais válidos, versão confirmada e operação persistida.
4. Nunca apagar automaticamente arquivo local no MVP, inclusive quando remoto for apagado/renomeado.
5. Nunca renomear automaticamente arquivo local modificado pelo usuário para fazê-lo “caber” na sincronização.
6. Nunca registrar refresh/access token, URL sensível de autorização ou conteúdo privado nos logs.
7. Nunca avançar cursor de mudanças se eventos correspondentes não estiverem duravelmente registrados.
8. Nunca recriar banco de estado corrompido silenciosamente.
9. Código Rust deve seguir contratos do `engine/`; GUI não importa `auth`, `download`, `state` ou `planner` diretamente.
10. Operação ambígua requer reconciliação; **repetir mecanicamente não é uma estratégia de recuperação**.

Consultar [invariantes globais](invariants.md), [máquinas de estado](state-machines.md) e [fluxo de recuperação](flows/recovery.md).

## 4. Contrato mínimo do produto

### Fluxo de ponta a ponta

```text
UI GNOME → DriveSyncEngine (API pública Rust)
                    ↓
               Coordinator
                    ↓
  Auth → Remote → Changes → Planner → Queue
                 ↑           ↑          ↓
            State(SQLite) ← Download ← Local
                    ↓
           Status / Notifications → UI
```

**O `Planner` permanece puro**: recebe `BaseVersion`, `LocalObservation` e `RemoteObservation`; devolve `Download`, `NoOp`, `BlockedConflict` ou `DeferredUnknown`. Operações `Upload` e `Delete` continuam no modelo conceitual, porém **não são habilitadas para execução no MVP**.

### Estados observáveis mínimos

`NOT_CONFIGURED`, `AUTH_REQUIRED`, `READY`, `SCANNING`, `DOWNLOADING`, `SYNCED`, `PAUSED`, `RETRY_WAIT`, `BLOCKED_CONFLICT`, `DEGRADED`, `ERROR`.

- `SYNCED` descreve **somente os itens elegíveis gerenciados e verificados**; itens ignorados/incompatíveis devem ter contagem própria.
- `UNKNOWN` (incerteza) não é `ABSENT` (ausência confirmada).
- Uma conta válida não substitui a validação de autorização sobre o conteúdo de cada arquivo.
- Primeira sincronização não deve ser confundida com “estado-base já conhecido”.

### Exemplos de critérios de aceite

| Cenário | Resultado exigido |
|---|---|
| 10 arquivos de teste em 2 níveis de pastas | 10 arquivos íntegros e caminhos corretos; nenhuma transferência parcial apresentada como concluída |
| Rodar ciclo idêntico mais duas vezes | **0 downloads de conteúdo** desnecessários, independentemente das consultas de metadados |
| Criar novo arquivo remoto | Apenas novo item transferido |
| Alterar arquivo remoto com local intacto | Local atualizado após validação e confirmação |
| Alterar mesmo arquivo local e remotamente | Não sobrescrever local; reportar conflito |
| Remover/renomear arquivo remotamente | Preservar conteúdo local antigo; status explícito e eventual novo caminho seguro |
| Desligar rede no meio do download | Original local preservado; operação recuperável |
| Encerrar abruptamente no meio de operação | Reiniciar, reconciliar e alcançar estado correto sem duplicação |
| Revogar permissão ou expirar credencial | Estado de autenticação correto; nenhum arquivo local apagado |
| Ocultar janela durante transferência | Processo continua e progresso aparece ao reabrir, dentro das permissões concedidas |
| Encerrar DriveSync explicitamente | Estado seguro em disco; sem serviço oculto órfão |
| Negar permissão de background/autostart | Aplicativo informa limitação; não mente que está em execução automática |

### Evidências mínimas do MVP

- Vídeo/capturas ou log de **execução real** no GNOME Builder e no Flatpak de desenvolvimento.
- Saídas documentadas de `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (ou comando de workspace equivalente), `cargo test --workspace`, testes de integração e cenários adversos.
- Lista de itens, hashes conferidos quando disponíveis, versões/cursor antes e depois; credenciais mascaradas.
- Testes de reinicialização, offline, conflito e idempotência.
- Registro das limitações do escopo OAuth e do funcionamento em segundo plano.
- Uma demonstração que não depende de editar dados pessoais de produção.

## 5. Checklist progressivo de implementação

**Como usar:** somente marque `[x]` após executar, revisar e registrar evidência no repositório. O scaffold Rust/GTK atual **já existe**, mas o usuário ainda não executou Builder nem validou os testes; portanto **tudo começa desmarcado**. Não marcar uma fase como pronta só por seus arquivos terem sido criados.

Cada fase depende integralmente da aprovação da anterior (`G00` → `G01` → … → `G14`). As tarefas podem gerar **pequenos PRs/commits** dentro da fase, mas não se inicia implementação funcional da seguinte antes do aceite.

### F00 — Conferir o ponto de partida e registrar o ambiente

**Pré-requisito:** repositório recém-clonado; nenhuma fase aprovada.

- [ ] F00.1 — Clonar `rafatosta/drivesync` e abrir `README.md`, `docs/architecture.md`, invariantes, metodologia, este MVP e documentação dos módulos.
- [ ] F00.2 — Verificar `Cargo.toml`, `engine/Cargo.toml`, `src/main.rs` e `engine/src/lib.rs`; documentar o que **existe** vs. o que **funciona**.
- [ ] F00.3 — Registrar versões reais de Fedora, GNOME, Rust/Cargo, GTK4, Libadwaita, GNOME Builder e Flatpak; instalação das dependências de desenvolvimento.
- [ ] F00.4 — Definir conta Google **de teste**, pasta remota exclusiva e pasta local temporária. Preparar arquivos descartáveis e proibir testes destrutivos fora desse isolamento.
- [ ] F00.5 — Criar registro de evidências (issue/PR ou `docs/implementation-log.md`) com data, ambiente, comandos e resultados.

**Gate G00:** ambiente identificado, arquitetura compreendida e dados de teste isolados. **Entrega:** inventário inicial e registro do ambiente.

### F01 — Rodar a aplicação inicial em Rust e no GNOME Builder

**Depende:** G00.

- [ ] F01.1 — Executar `cargo check --workspace`; corrigir apenas problemas de dependências/configuração encontrados.
- [ ] F01.2 — Executar `cargo run` e comprovar que aparece a janela simples “DriveSync”.
- [ ] F01.3 — Abrir o repositório existente no GNOME Builder (não criar outro projeto separado).
- [ ] F01.4 — Configurar e **validar** a construção/execução pelo Builder (Meson/Cargo conforme necessário); registrar ajustes de build sem acoplar motor à UI.
- [ ] F01.5 — Confirmar que a UI chama apenas `DriveSyncEngine` pela API pública; nenhuma regra de sincronização em `main.rs`.

**Gate G01:** janela executa pelo terminal e pelo Builder; erros de compilação registrados e resolvidos. **Entrega:** aplicação mínima executável e ambiente reprodutível.

### F02 — Validar cedo o ambiente Flatpak

**Depende:** G01. Não adiar validação do sandbox até depois do motor.

- [ ] F02.1 — Criar/configurar manifesto **de desenvolvimento** Flatpak, app ID coerente e permissões mínimas; escolher runtime/SDK GNOME apropriados.
- [ ] F02.2 — Construir e executar a **mesma** janela no sandbox pelo Builder/Flatpak.
- [ ] F02.3 — Provar acesso autorizado à pasta local de teste pelo mecanismo previsto (portal de seleção de pasta e/ou permissão documentada); não solicitar acesso irrestrito ao sistema de arquivos sem necessidade.
- [ ] F02.4 — Provar que URL no navegador externo/fluxo de retorno OAuth é tecnicamente viável no sandbox, sem ainda implementar o login completo.
- [ ] F02.5 — Registrar limitações reais de rede, armazenamento e runtime; corrigir manifesto/build antes de prosseguir.

**Gate G02:** janela Flatpak funciona e foram demonstrados caminho local autorizado e mecanismo viável de navegador/retorno. **Entrega:** pacote de desenvolvimento testável.

### F03 — Estabelecer testes, CI e contratos mínimos

**Depende:** G02.

- [ ] F03.1 — Executar teste já existente da fachada (`cargo test -p drivesync-engine`); não presumir aprovação por existir código de teste.
- [ ] F03.2 — Configurar `fmt`, `clippy` e testes automatizados em CI (sem credenciais reais).
- [ ] F03.3 — Consolidar tipos conceituais mínimos: `AccountId`, `SyncRootId`, `RemoteFileId`, `RemoteRevision`, `LocalFingerprint`, `Observation`, `BaseVersion`, `Operation`, `EngineError` e `SyncStatus`.
- [ ] F03.4 — Definir portas de autenticação, armazenamento remoto, estado e filesystem; criar **fakes** simples para casos de erro, sem implementar recursos reais prematuramente.
- [ ] F03.5 — Especificar testes com fixtures e oráculos independentes para os algoritmos do MVP; revisar contratos antes de implementar.

**Gate G03:** pipeline de testes executado e contratos suficientes para testar motor sem Google. **Entrega:** base verificável do workspace.

### F04 — Confirmar viabilidade OAuth e permissões sobre arquivos existentes

**Depende:** G03. **Gate de alto risco:** pode alterar as decisões anteriores.

- [ ] F04.1 — Criar projeto Google Cloud de **desenvolvimento** e habilitar Google Drive API; registrar categoria Desktop App e configuração OAuth.
- [ ] F04.2 — Estudar `drive.file`, `drive.readonly` e, para produto bidirecional futuro, `drive`, incluindo restrições de verificação/publicação. Escolher o **menor escopo que realmente permita o caso de uso**.
- [ ] F04.3 — Preparar experimento com pasta/arquivos **pré-existentes** na conta, não apenas arquivos criados pelo aplicativo.
- [ ] F04.4 — Registrar comportamento real de consentimento, redirect PKCE, acesso aos arquivos e renovação; distinguir limitações de ambiente OAuth Testing.
- [ ] F04.5 — Escrever ADR de escopos e estratégia de publicação. Se `drive.file` não atender, documentar necessidade/impacto de escopo restrito; **não contornar** a autorização.

**Gate G04:** há uma estratégia de autorização comprovada para acessar a pasta remota de teste e um risco conhecido para distribuição pública. **Se não houver**, marcar `BLOCKED` e rever escopo/arquitetura antes de seguir.

### F05 — Implementar `auth` (OAuth real e seguro)

**Depende:** G04. Algoritmos: `AUTH-001`, `AUTH-002` e tratamento de revogação.

- [ ] F05.1 — Completar entradas, saídas, pseudocódigo e oráculos em `engine/src/auth/`, com links à RFC 7636, RFC 8252 e documentação Google.
- [ ] F05.2 — Implementar fluxo do navegador com PKCE/state/callback único e tratamento de cancelamento.
- [ ] F05.3 — Armazenar tokens por mecanismo seguro do desktop; não em texto simples no banco, logs ou repositório.
- [ ] F05.4 — Implementar renovação e classificação distinta de offline, token inválido/revogado e falha temporária.
- [ ] F05.5 — Testar fake, fluxo real no Flatpak, reconexão e reinstância; revisar código Rust e segurança.

**Gate G05:** uma conta de teste pode ser autorizada, reutilizada e detectada como inválida quando necessário, sem segredo exposto. **Entrega:** `AuthProvider` validado.

### F06 — Implementar `remote` (leitura tipada da API Drive)

**Depende:** G05. Algoritmos: `REMOTE-001/002/003`.

- [ ] F06.1 — Especificar operações mínimas `list(folder)`, `get_metadata(id)`, `download_stream(id)`, `get_start_token()`, `list_changes(token)`.
- [ ] F06.2 — Implementar paginação, máscaras de campos, IDs e MIME sem pressupor nomes únicos.
- [ ] F06.3 — Distinguir `403`, `404`, `429`, `5xx`, timeout, cancelamento e perda de autorização; backoff bounded.
- [ ] F06.4 — Testar com fake e com pasta de teste real, incluindo subpastas e arquivos não baixáveis.
- [ ] F06.5 — Impedir que recursos Workspace nativos ou itens especiais entrem na rota de arquivo binário comum.

**Gate G06:** inventário remoto tipado e completo, com erros corretos e paginação provada. **Entrega:** adaptador de leitura.

### F07 — Implementar `configuration`, `accounts`, `local` mínimo

**Depende:** G06.

- [ ] F07.1 — Registrar identidade da conta, uma raiz remota por ID e raiz local autorizada, sem pressupor unicidade de nome.
- [ ] F07.2 — Validar caminhos, colisões, permissões, nomes inseguros, links simbólicos e limites do diretório local.
- [ ] F07.3 — Criar snapshots/fingerprints locais para diferenciar arquivo intocado, modificado, preexistente e desconhecido.
- [ ] F07.4 — Persistir configuração versionada com teste de leitura/reinicialização; exclusões não fazem parte da configuração deste MVP.
- [ ] F07.5 — Testar filesystem temporário, pastas aninhadas e nomes de teste Unicode.

**Gate G07:** raiz local segura, vínculo inequívoco com remoto e observações locais confiáveis. **Entrega:** preparação de pasta e perfil.

### F08 — Implementar `state` SQLite e base durável

**Depende:** G07. Algoritmos: `STATE-001/002`.

- [ ] F08.1 — Definir schema e migrações mínimas para conta/raiz, IDs remotos, versões, fingerprints locais, operações, conflitos e cursor.
- [ ] F08.2 — Implementar transações de intenção/confirmacão, com estados `PENDING/RUNNING/VERIFYING/COMMITTED/RECONCILE`.
- [ ] F08.3 — Gravar cursores somente após mudanças serem persistidas e classificadas.
- [ ] F08.4 — Testar crash/reabertura e corrupção do banco: preservar DB e bloquear operações arriscadas.
- [ ] F08.5 — Garantir que `UNKNOWN` jamais seja gravado como deleção confirmada.

**Gate G08:** histórico recuperável e invariantes de persistência validados. **Entrega:** estado local confiável.

### F09 — Implementar `planner`, `queue` e decisões não destrutivas

**Depende:** G08.

- [ ] F09.1 — Definir matriz B/L/R completa para o **subconjunto do MVP**, incluindo primeira descoberta, modificação unilateral, arquivo local preexistente, conflito e estado desconhecido.
- [ ] F09.2 — Implementar `Planner` puro, sem rede nem escrita; aceitar futuro `Upload/Delete` no modelo, mas bloquear sua execução neste marco.
- [ ] F09.3 — Implementar fila durável de **downloads** com dependência pai→filho e recuperação de operação interrompida.
- [ ] F09.4 — Aplicar deduplicação/idempotência de operações: mesmo ciclo não baixa arquivo imutável outra vez.
- [ ] F09.5 — Criar fixtures B/L/R, casos de conflito local, múltiplas revisões e fake remote.

**Gate G09:** plano e fila produzem ações determinísticas, não destrutivas e recuperáveis. **Entrega:** decisão sem efeitos colaterais.

### F10 — Implementar `download` seguro

**Depende:** G09. Algoritmos: `DOWNLOAD-001/002`.

- [ ] F10.1 — Definir fluxo com streaming, staging, tamanho/checksum quando disponível, revalidação e instalação sem arquivo parcial visível.
- [ ] F10.2 — Garantir temporário no filesystem apropriado; tratar interrupção, erro de escrita, falta de espaço e falha de rename.
- [ ] F10.3 — Confirmar no SQLite somente depois de instalação verificada; preservar arquivo antigo divergente.
- [ ] F10.4 — Testar falha em pontos intermediários (network, I/O, validação, substituição, confirmação).
- [ ] F10.5 — Demonstrar download de arquivo binário pequeno, grande e arquivo com caracteres Unicode em nome.

**Gate G10:** arquivos completos, íntegros e jamais substituídos por incompletos. **Entrega:** executor de download.

### F11 — Integrar a primeira sincronização completa

**Depende:** G10.

- [ ] F11.1 — Implementar `Coordinator` com pré-condições: conta autorizada, raiz válida, banco saudável e exclusividade por raiz.
- [ ] F11.2 — Na primeira varredura, estabelecer estratégia correta para não perder alterações ocorridas durante inventário inicial (token inicial + reconciliação posterior).
- [ ] F11.3 — Criar diretórios e baixar arquivos elegíveis; registrar itens ignorados/pendentes.
- [ ] F11.4 — Integrar status pela fachada `DriveSyncEngine` e janela mínima.
- [ ] F11.5 — Repetir ciclo sem alterações e comprovar **zero downloads de conteúdo** desnecessários.

**Gate G11:** primeiro fluxo real ponta a ponta funciona e é repetível. **Entrega:** cópia inicial segura.

### F12 — Incremental, conflitos conservadores e recuperação

**Depende:** G11. Algoritmos: `CHANGES-002`, `RECOVERY-002`, decisões de `conflicts`.

- [ ] F12.1 — Consumir `changes.list` paginado, persistir eventos e avançar cursor só no fim validado.
- [ ] F12.2 — Adicionar/atualizar arquivos remotos quando local não estiver divergente; detectar e preservar conflitos, sem pedir decisão automática.
- [ ] F12.3 — Tratar exclusão, perda de acesso, renomeação e recurso não suportado **sem deletar arquivos locais**.
- [ ] F12.4 — Recuperar após rede cair, `SIGKILL`, cursor interrompido e disco cheio; conferir idempotência.
- [ ] F12.5 — Testar reinícios consecutivos, alterações entre ciclos e operações pendentes.

**Gate G12:** alterações incrementais e recovery de falhas centrais comprovados. **Entrega:** sincronização funcional não destrutiva.

### F13 — Finalizar comportamento GNOME/Flatpak em segundo plano

**Depende:** G12.

- [ ] F13.1 — Implementar fechar → ocultar com retenção explícita do `GApplication` e estado separado do motor.
- [ ] F13.2 — Reabrir pelo lançador deve recuperar janela existente sem iniciar segunda sincronização.
- [ ] F13.3 — Incluir Iniciar/Pausar/Sair e informações honestas sobre estado, conflitos e itens não suportados.
- [ ] F13.4 — Validar solicitação de background/autostart pelo portal, distinção entre solicitação e autorização, e iniciar oculto se habilitado. Se negado, mostrar limitação sem impedir uso manual.
- [ ] F13.5 — Testar pelo **Flatpak** no GNOME com janela oculta, sem extensão Nautilus ou daemon.

**Gate G13:** aplicativo continua sincronizando com janela oculta conforme permissões e retoma apresentação corretamente. **Entrega:** comportamento prático de desktop.

### F14 — Aceitação do MVP e preparação para evolução

**Depende:** G13.

- [ ] F14.1 — Rodar toda a matriz funcional da seção 4 com pasta Google de teste e registrar evidência.
- [ ] F14.2 — Executar pipeline Rust (fmt/clippy/test) e testes adversos; corrigir falhas antes de aprovação.
- [ ] F14.3 — Conferir proteção de tokens, permissões Flatpak e comportamento sem rede/disco.
- [ ] F14.4 — Revisar APIs e schema contra as funcionalidades futuras: upload, delete, multi-conta, conflitos, Nautilus; verificar que não há dependência estrutural de read-only.
- [ ] F14.5 — Registrar decisões, limitações, bugs conhecidos e possíveis gates de distribuição OAuth/Flathub (separados da prova técnica do MVP).
- [ ] F14.6 — Marcar MVP `VALIDATED` apenas após demonstrar critérios, ou `BLOCKED` com causa e plano de revisão.

**Gate G14 = MVP ACEITO:** demonstração ponta a ponta reprodutível, base em SQLite recuperável, arquivos preservados sob falha, mudanças incrementais, aplicação funcional em Flatpak/GNOME e riscos restantes documentados.

## 6. Como fases posteriores podem modificar fases anteriores

O desenvolvimento é **sequencial, mas não irreversível**. Regressões e novas evidências exigem retorno controlado:

1. Se uma fase N revelar que uma premissa anterior é falsa, marcar o gate original como **REOPENED** e registrar qual evidência falhou.
2. Identificar documentos de algoritmo, contrato, schema, testes e decisões afetados. Não mudar apenas o código.
3. Criar alteração pequena (issue/PR) corrigindo a fase de origem e adicionando o teste de regressão que reproduz o problema.
4. Reexecutar o gate reaberto e todos os gates posteriores cuja validade depende da alteração.
5. Só então continuar a próxima fase. Nunca apagar um histórico de falha para marcar “concluído”.

**Exemplos:** OAuth `drive.file` não vê arquivo antigo → reabrir F04, eventualmente F02/F05; renomeação entre listagem e download → reabrir F06/F09/F10/F11; corrupção de estado após kill → reabrir F08, F09 e F12; permissão Flatpak impede autostart → reabrir F02 e F13 sem comprometer proteção de arquivos.

### Modelo de registro de gate

```text
Gate: G__
Status: NOT_STARTED | IN_PROGRESS | BLOCKED | PASSED | REOPENED
Data/ambiente:
Versão/commit:
Documentos e algoritmos:
Comandos e testes executados:
Resultado esperado / observado:
Evidências (links/logs sem segredos):
Riscos remanescentes:
Validação humana:
Gates impactados:
```

O registro pode ser feito inicialmente em issues/PRs do GitHub; um arquivo de log pode ser criado na F00. A lista de tarefas deste documento é a **fonte principal do escopo** e não deve ser duplicada em quadros sem referência.

## 7. Como escolher as primeiras tarefas para a IA

Um commit/PR deve tratar **um algoritmo ou uma unidade pequena de infraestrutura**. Fornecer sempre `docs/invariants.md`, `docs/development-methodology.md`, `docs/ai-development-guidelines.md`, contrato e `algorithms.md` do módulo. A IA deve devolver código revisável, testes e descrição do que não foi validado; não deve declarar fase aprovada.

**Primeira tarefa efetiva após este planejamento:** F00.1–F00.5. Depois, **F01**: provar a compilação/execução atual no Fedora e no GNOME Builder antes de acrescentar autenticação, APIs ou banco.

## 8. Referências e vínculo com o projeto completo

- [Método de desenvolvimento com IA](development-methodology.md) — processo e estados de maturidade.
- [Modelo obrigatório de algoritmo](algorithm-spec-template.md) — documentação por problema.
- [Catálogo de técnicas conhecidas](problem-catalog.md) e [referências primárias](reference-catalog.md).
- [Arquitetura](architecture.md), [modelo de dados](data-model.md), [dependências](module-dependencies.md), [API pública](public-engine-api.md), [recuperação](flows/recovery.md).
- [Viabilidade geral](feasibility-validation.md) — experimentos além do MVP.
- [OAuth desktop e PKCE](https://developers.google.com/identity/protocols/oauth2/native-app); [escopos Drive](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).
- [Google Drive changes](https://developers.google.com/workspace/drive/api/guides/manage-changes), [downloads](https://developers.google.com/workspace/drive/api/guides/manage-downloads), [erros](https://developers.google.com/workspace/drive/api/guides/handle-errors).
- [Background Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html).

**Critério de compatibilidade futura:** o MVP implementa um caso de uso limitado de uma arquitetura preparada para novos adaptadores e decisões. Não acrescentar recursos não necessários, mas também não gravar regras como “somente download para sempre” nas interfaces globais nem no schema.

---

**Situação na data deste documento:** escopo e roteiro definidos; **nenhuma fase foi executada ou validada** e nenhuma transferência de arquivos foi implementada. A existência de `Cargo.toml` e de testes de exemplo não comprova que a aplicação já tenha sido executada no Builder.
