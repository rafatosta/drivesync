# Revisão independente do DriveSync

Data: 26/10/2026 (America/Bahia). Repositório: `/home/tosta/Projetos/drivesync`, branch `main`, revisão `d039f059f001ec352356ffdae7fa38c38a668008`.

## Escopo e método

Revisão de código, testes, dependências travadas, scripts, configuração OAuth e manifestos Flatpak. O checkout estava limpo no início. Somente este relatório foi criado como arquivo de entrega; aplicação e testes não foram editados. Os comandos de validação geraram/atualizaram artefatos ignorados em `target/`. Não foram usados resultados de revisões anteriores como evidência de aprovação.

Foram lidos `AGENTS.md`, `README.md`, `docs/README.md`, a estrutura de `docs/SOFTWARE_SPECIFICATION.md`, invariantes e metodologia arquivados, contratos de download/upload/remote e implementações e testes dos fluxos examinados. A inspeção foi direcionada a autenticação, isolamento, persistência, caminhos, transferências e recuperação; não constitui auditoria exaustiva de cada linha ou do histórico Git.

**Classificação:** confirmado significa comportamento ou configuração demonstrável no código ou nas ferramentas executadas; não significa necessariamente reprodução dinâmica. As duas falhas de recuperação abaixo foram confirmadas por inspeção do fluxo, sem injeção de crash executada. Hipóteses de exploração ficam em seção separada. Gravidades refletem impacto e pré-condições neste projeto.

## Arquitetura e funcionalidades

Workspace Rust com dois pacotes:

- `drivesync`: aplicação GTK4/Libadwaita; interface em `src/`, trabalhadores para operações bloqueantes, agendamento em `automatic/` e integração de background/autostart via portais.
- `drivesync-engine`: fachada pública em `engine/src/facade.rs`; adaptadores OAuth/Google HTTP e Secret Service; índice JSON de contas/configurações; descoberta remota, feed incremental, planejamento, downloads e uploads opcionais.
- Estado SQLite por raiz em `.drivesync/`: diário de operações, checkpoints e intenções de upload. Marcador associa pasta a cliente OAuth, conta e pasta remota. Downloads usam staging, verificação MD5/SHA-256, descritores de diretório, renomeação sem sobrescrita e backups para atualizações. Upload cria somente itens novos e mantém snapshots privados e IDs remotos para retomada.

O produto oferece múltiplas contas, uma pasta remota/local por conta, downloads e atualizações locais, upload de criação mediante opt-in, verificações periódicas, pausa e histórico. Não há sincronização bidirecional completa; alterações/exclusões locais não são propagadas. Documentos Workspace viram links. `coordinator`, `queue`, `local`, `ipc` e outros módulos contêm contratos/placeholders; o fluxo entregue está principalmente na fachada e nos executores. Não se deve inferir implementação a partir de documentação conceitual.

## Achados confirmados

### R1 — Média: criação inicial do banco não é recuperável após interrupção

**Local:** `engine/src/state/mod.rs:47`, `:54`, `:72` e `:98`; consumidores em `engine/src/facade.rs:277` e nos stores de checkpoints/uploads.

**Evidência:** `open_format` primeiro cria o arquivo definitivo e sincroniza o arquivo vazio; somente depois abre SQLite e inicia a transação que grava schema, `application_id` e `user_version`. A inicialização só ocorre quando `create_new` teve sucesso nesta chamada. Se houver interrupção antes do commit, a próxima abertura encontra arquivo existente, pula a inicialização e rejeita identificadores/schema zerados. Uma falha transitória na primeira transação também pode deixar o mesmo estado. Não há mecanismo de completar a inicialização identificando sua propriedade.

**Impacto:** uma queda na primeira utilização pode tornar a sincronização persistentemente indisponível para a raiz, mesmo depois de resolvida a causa. Não foi demonstrada perda de conteúdo do usuário. Rejeitar bancos arbitrariamente vazios é uma proteção válida; o defeito é publicar o arquivo definitivo antes de concluir sua própria inicialização.

**Cenário de reprodução proposto, não executado:** em pasta descartável, interromper o processo depois do `file.sync_all()` da linha 54 e antes do commit da linha 81; reiniciar e solicitar o mesmo fluxo. Esperado pelo código: `StateStorage` nas novas tentativas. Cobrir também falha da transação inicial e concorrência entre primeiras aberturas.

**Correção recomendada:** inicializar um banco temporário privado, concluir e sincronizar schema e metadados, fechar a conexão e publicar sem substituir destino existente, sincronizando o diretório; reconciliar inicializações concorrentes. Preservar a rejeição de bancos estrangeiros/corrompidos e não apagar automaticamente o diário de operações.

### R2 — Média: snapshots de upload podem ficar retidos indefinidamente

**Local:** `engine/src/upload/local.rs:166`; `engine/src/upload/execution.rs:161`, `:183`, `:193` e `:345–348`.

**Evidência:** após produzir o snapshot completo, `snapshot` desarma a limpeza antes de a intenção ser persistida. Se `uploads.save` falhar ou o processo morrer nesse intervalo, sobra um arquivo sem intenção correspondente. Além disso, a confirmação é persistida antes de remover o snapshot; falha de remoção é ignorada, e futuras execuções retornam cedo para `op.confirmed`, sem repetir a limpeza. A busca pelas referências a snapshots não encontrou coleta de órfãos. Um crash durante a cópia também não executa o `Drop` que remove snapshots incompletos.

**Impacto:** retenção de cópias integrais de arquivos, ocupação progressiva de disco e possível indisponibilidade de outras escritas/sincronizações. Os snapshots são criados com modo `0600`; não foi demonstrado acesso por outro usuário ou exfiltração.

**Cenários de reprodução propostos, não executados:** (a) abortar durante a preparação ou entre a criação do snapshot e `uploads.save`, reiniciar e repetir; (b) abortar depois da confirmação persistida e antes de `remove_file`; (c) fazer a remoção falhar após confirmação. Inspecionar os `upload-*.snapshot` remanescentes em ciclos posteriores. Usar arquivos descartáveis e disco com limite.

**Correção recomendada:** manter proteção de limpeza até a intenção durável, registrar a etapa de preparação de forma recuperável e reconciliar snapshots órfãos/incompletos sob o lock da raiz. Repetir limpeza de snapshots confirmados, validando nome, identidade e vínculo à operação. Não excluir snapshots ainda necessários para retomada.

### R3 — Baixa: lockfile contém Tokio com alerta conhecido, sem caminho afetado identificado nesta configuração

**Local:** `Cargo.lock:2264–2265` (`tokio 1.54.0`), transitivo de `reqwest`/`hyper`.

**Evidência:** consulta OSV identificou `RUSTSEC-2026-0384` e `GHSA-5wjr-9356-4wc8`, aliases da **mesma** vulnerabilidade. O aviso descreve corrida/use-after-free de `Interval` em runtime multithread, corrigida em `1.54.1`. A gravidade publicada no GHSA é moderada. [RustSec](https://rustsec.org/advisories/RUSTSEC-2026-0384.html).

**Aplicabilidade/impacto:** a presença da versão é confirmada; exploração no DriveSync não foi demonstrada. `cargo tree -e features -i tokio` não mostrou `rt-multi-thread`; o Reqwest bloqueante em cache cria runtime `new_current_thread` (`reqwest-0.12.28/src/blocking/client.rs:1369`). A condição descrita no aviso não foi identificada no build Linux revisado. Não classificar este resultado como corrupção de memória comprovada no aplicativo.

**Reprodução executada:** consultar os pares nome/versão do lockfile na API OSV e executar `cargo tree --locked --offline -i tokio`. Reprodução da vulnerabilidade não executada; dependeria do uso de `Interval` no runtime afetado.

**Correção recomendada:** atualizar seletivamente Tokio para versão corrigida compatível, renovar o lockfile e validar o workspace; manter verificação automatizada de advisories e reavaliar se features/dependentes mudarem.

### R4 — Baixa: testes de upload não verificam independentemente parte do contrato de produção

**Local:** `engine/src/upload/tests.rs:35`, `:118–132`; `engine/src/upload/transport.rs:262–274`.

**Evidência:** o fake conclui por tamanho e constrói os metadados remotos a partir da intenção, incluindo o checksum esperado. `validate_parent` sempre retorna sucesso. O teste de transporte cria uma resposta HTTP local usando uma requisição GET auxiliar; apenas exige que tenha chegado algum byte, sem exercitar `send` de produção ou verificar método PUT, `Content-Range`, corpo, autenticação e sequência de retomada.

**Impacto:** a suíte pode continuar verde diante de regressões de validação do adaptador, checksum remoto ou montagem da requisição. Há testes úteis de estados, bytes e interrupções, mas eles não comprovam esses contratos de integração.

**Cenário de reprodução proposto, não executado:** em cópia descartável, remover a validação do adaptador ou introduzir erro no cabeçalho de produção e verificar quais testes falham. A inspeção mostra que os testes citados não passam por essas operações. Não foi feita análise de mutação nem declarado que toda a suíte sobreviveria à mutação.

**Correção recomendada:** acrescentar testes de contrato do adaptador com oráculo independente; derivar checksum dos bytes efetivamente recebidos; rejeitar ancestral movido/fora do escopo e validar requisições reais construídas pelo transporte. Preservar restrição de origem das sessões; uma seam de teste não deve permitir endpoint arbitrário em produção.

### R5 — Baixa: instruções obrigatórias apontam para documentos inexistentes nos caminhos indicados

**Local:** `AGENTS.md:14` e `:23`; `engine/src/download/contract.md:19`; `engine/src/upload/contract.md:19`.

**Evidência:** os caminhos `docs/invariants.md`, `docs/development-methodology.md` e `docs/MVP.md` foram movidos para `docs/old/`. `docs/README.md:17` avisa que o acervo histórico não é automaticamente vigente, enquanto a nova especificação ainda contém somente estrutura de seções.

**Impacto:** ambiguidade sobre contratos obrigatórios para manutenção futura, especialmente recuperação e operações destrutivas. Não é falha de segurança explorável demonstrada.

**Reprodução executada:** inventário com `rg --files docs`, leitura das instruções e do índice; os arquivos estão em `docs/old/`.

**Correção recomendada:** atualizar as referências nas instruções/contratos e definir explicitamente quais invariantes continuam vigentes durante a migração documental.

### I1 — Informativo: três dependências travadas têm avisos de descontinuação

**Local:** `Cargo.lock:2830` (`zbus_names 3.0.0`), `:2941` (`zvariant 4.2.0`) e `:2954` (`zvariant_derive 4.2.0`).

**Evidência/reprodução executada:** OSV retornou os avisos [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html), [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) e [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html). São avisos de manutenção, não três vulnerabilidades exploráveis. `cargo tree -i zvariant` e `-i zbus_names` não mostraram dependentes no alvo/configuração atuais.

**Impacto:** risco futuro de manutenção para configurações que ativem esses componentes; nenhum impacto de execução demonstrado aqui. **Recomendação:** identificar os caminhos em outros alvos/features antes de atualizar ou remover dependências; acompanhar a migração indicada pelos mantenedores, sem forçar troca incompatível apenas pelo alerta.

## Suspeitas e riscos que exigem validação adicional

### S1 — Potencial média: corrida local na resolução dos caminhos dos metadados

**Local:** `engine/src/configuration/identity.rs:151–157`; `engine/src/state/mod.rs:41–65`; `engine/src/upload/local.rs:129–133`.

**Evidência:** o marcador é verificado usando descritores e `NOFOLLOW`, mas o resultado volta a ser um caminho textual. Criação/abertura de banco e snapshot resolvem ancestrais novamente. `SQLITE_OPEN_NOFOLLOW`/`O_NOFOLLOW` na abertura final não equivalem ao percurso seguro de todos os componentes.

**Pré-condição/impacto potencial:** outro processo com permissão para substituir a raiz ou `.drivesync` durante a janela poderia redirecionar criação/acesso a metadados para outro diretório acessível. Não foi reproduzido desvio de escrita, corrupção nem acesso entre contas. O lock é consultivo e não impede processo local hostil; este cenário não demonstra ataque remoto.

**Experimento recomendado, não executado:** em árvore descartável, intercalar troca de ancestral por symlink entre `ensure` e abertura, observar por rastreamento de syscalls onde o arquivo foi criado e se o fluxo prossegue. **Correção caso confirmada:** manter descritores/identidades dos diretórios de metadados e adotar abertura segura compatível com SQLite, eliminando a separação entre checagem e uso.

### S2 — Potencial média: instâncias independentes podem perder atualizações do índice de contas

**Local:** `engine/src/accounts/repository.rs:25–34`, `:94–102` e `:128–133`; `engine/src/facade.rs:22–23`.

**Evidência:** cada `Accounts` possui seu próprio mutex e escreve por read/modify/rename. O próprio contrato restringe a serialização à instância. Dois `DriveSyncEngine::new()` independentes podem ler o mesmo índice e publicar alterações concorrentes, perdendo uma delas.

**Limite:** a GUI usa aplicação de instância única e compartilha o motor; não foi demonstrado que o fluxo normal dispara o problema. **Reprodução proposta, não executada:** dois processos/motores com mesmo `XDG_DATA_HOME` isolado, barreira após leitura, gravações distintas, verificação de ambas após reinício. **Impacto potencial:** perda de configuração/perfis, sem demonstração de perda de arquivos. **Recomendação:** se múltiplos consumidores forem suportados, lock entre instâncias/processos ou transação com detecção de versão; caso contrário explicitar e impor o contrato de instância única na API.

## Segurança examinada

- OAuth usa loopback, PKCE e comparação de state resistente a diferenças de tempo; callback valida host, método, caminho e parâmetros duplicados. Identidade é obtida do provedor e vinculada à conta. Testes usam provedor/cofre substitutos; login real não foi executado.
- Credenciais ficam no Secret Service, separadas por cliente e conta; erros HTTP são tipados e não retornam corpos brutos do provedor. Não foi identificado vazamento concreto de token nos trechos examinados. Varredura textual limitada dos arquivos rastreados não encontrou padrões de chave privada, Google API key, token `ya29` ou token GitHub. Isso não detecta todo segredo, não examina histórico e não atesta arquivos ignorados.
- Clientes HTTP autenticados desabilitam redirects; sessões de upload restringem esquema/origem/caminho. Consultas de nomes escapam aspas e barras, IDs são validados, SQL usa parâmetros. Não foi demonstrada injeção SQL, de consulta Drive ou execução de comando por nome remoto.
- Downloads percorrem componentes por descritor sem seguir symlinks e usam instalação sem substituir destino. Existem testes adversariais de symlinks, troca de inode/ancestral, corrupção e confirmação perdida. Isso não elimina corridas com processos locais hostis em todos os pontos, como S1.
- OAuth solicita escopo completo do Drive mesmo no modo download. É uma escolha explícita, coberta por teste; opt-in local de upload não reduz o poder do token comprometido. Reavaliar menor privilégio conforme requisitos, sem tratar a escolha como bypass demonstrado de autorização.
- Manifestos não concedem filesystem global; incluem rede, renderização gráfica, IPC e Secret Service. Desenvolvimento e candidato de produção fazem build com rede e fontes `type: dir`; não são uma receita de release inteiramente offline/reproduzível. Essa limitação está declarada nos próprios manifestos. Build/instalação de produção e verificação Google não foram validados.

## Execuções e resultados

O host não tem `rustc`/`cargo` no PATH (`command not found`). Havia SDK GNOME 50 e Rust estável instalados. A primeira tentativa Flatpak sem `--user` falhou por ambiguidade entre instalação de usuário e sistema; repetida explicitamente com `--user`, funcionou.

Os comandos Cargo/Python abaixo foram executados na raiz do repositório, neste ambiente:

```sh
flatpak run --user --command=sh \
  --filesystem=/home/tosta/Projetos/drivesync \
  --env=CARGO_HOME=/home/tosta/Projetos/drivesync/target/demo-cargo \
  org.gnome.Sdk//50
# Dentro do SDK:
export PATH=/usr/lib/sdk/rust-stable/bin:$PATH
```

Rust `1.98.1 (48a229cea 2026-09-01)`, Cargo `1.98.1`. Foi reutilizado o cache de dependências e build existente; não foi feito build limpo em máquina nova.

| Comando executado | Resultado observado |
| --- | --- |
| `cargo test --workspace --locked --offline` | 156 passaram, 0 falharam, 8 ignorados: GUI 18/6, engine 136/2, dois testes de integração aprovados. Doctests: zero. |
| `cargo fmt --all --check` | Concluiu sem diferenças/erros. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Concluiu sem diagnósticos. |
| `cargo doc --workspace --no-deps --document-private-items --locked --offline` | Documentação gerada, sem diagnósticos. |
| `cargo test --workspace --doc --locked --offline` | Concluiu, mas executou **0 testes**. |
| `python3 scripts/check-oauth-build.py` | Aprovou fixtures de configuração OAuth; não fez login Google. |
| `cargo audit --version` | Falhou: subcomando `audit` não instalado; auditoria cargo-audit não executada. |
| `cargo tree --locked --offline -i tokio` e `-e features -i tokio` | Dependência ativa confirmada; sem feature `rt-multi-thread` na saída. |
| `cargo tree --locked --offline -i zvariant` e `-i zbus_names` | Nenhum dependente no alvo/configuração atuais; não equivale a ausência no lock ou em outros alvos. |
| `desktop-file-validate data/com.rtosta.DriveSync.Devel.desktop` | Concluiu com sugestão: mais de uma categoria principal pode duplicar entrada no menu. |
| `appstreamcli validate --no-net data/com.rtosta.DriveSync.Devel.metainfo.xml` | Validação bem-sucedida, indicação `pedantic: 2`; não fez validação online. |
| `git diff --check` | Sem erro no momento da execução. |
| Varredura textual por padrões de segredos em `git ls-files` | 253 arquivos de texto examinados, nenhum padrão encontrado. Não foi scanner especializado. |
| Consulta Python à API `https://api.osv.dev/v1/querybatch` | 286 pares nome/versão do Cargo.lock consultados; 4 pacotes sinalizados, 5 IDs, correspondentes a 1 vulnerabilidade e 3 avisos de manutenção. |

A consulta OSV leu `Cargo.lock` com `tomllib`, selecionou pacotes `source` iniciados por `registry+` e enviou `{"queries":[{"package":{"name":NOME,"ecosystem":"crates.io"},"version":VERSAO},...]}` via POST JSON. Foram lidos os detalhes em `/v1/vulns/<ID>` e as páginas RustSec, em 26/10/2026. Somente nomes/versões foram enviados, sem conteúdo de arquivos ou credenciais. Limitações: base de vulnerabilidades conhecidas, não análise de alcançabilidade, não scan das bibliotecas do runtime GNOME/sistema nem auditoria do SQLite bundled como produto C independente. Os dois IDs Tokio são duplicados por alias.

`command -v` não localizou `gitleaks`, `trivy`, `osv-scanner`, `semgrep` ou `flatpak-builder`. Não se declara que essas ferramentas aprovaram o projeto. Não foram instaladas ferramentas nem alteradas dependências para a revisão.

## Resumo dos achados por gravidade

| Gravidade | Confirmados | Suspeitas, separadas |
| --- | --- | --- |
| Crítica | 0 | 0 |
| Alta | 0 | 0 |
| Média | 2 — R1, R2: recuperação/persistência e retenção de snapshots | 2 — S1, S2: corridas locais/instâncias independentes |
| Baixa | 3 — R3, R4, R5: versão com advisory sem condição afetada identificada, oráculos de testes e documentação | 0 |
| Informativa | 1 — I1, agrupando três avisos de manutenção | 0 |

## Resultado dos testes e verificações

156 testes aprovados, nenhum reprovado e 8 ignorados. Formatação, Clippy, geração de documentação e fixtures OAuth concluíram com sucesso; o comando de doctests não possui casos. Validadores desktop/AppStream concluíram, com observações acima. A consulta OSV produziu alertas que exigem acompanhamento; **não** houve aprovação de cargo-audit, ferramenta indisponível. Os resultados verdes não cobrem as falhas de recuperação identificadas por inspeção.

## Lacunas de cobertura e partes não avaliadas

- Oito testes ignorados: quatro de desktop/background/autostart (`hidden_window_runs_automatic_cycles`, `denied_startup_and_late_success_preserve_manual_use`, `session_launch_stays_hidden_and_launcher_activation_reopens`, `desktop_close_and_reopen`), dois de layout/screenshots, um de aliases do document portal e um de navegação de pastas com conta Google real. Não foram forçados em sessão pessoal.
- Ausência de injeção de crash na inicialização dos três bancos e nas transições de criação/limpeza de snapshots. Testes existentes de recuperação de download têm verificações relevantes de bytes/identidade, mas simulação de estado e falhas retornadas não substituem kill de processo em cada fronteira de durabilidade.
- Oráculos de fake/transporte descritos em R4. Faltam evidências independentes de requisições de upload reais, incluindo arquivo vazio, retomada parcial, expiração durante envio e reconciliação com mudança remota concorrente.
- Não foram executados login/logout real, cofre bloqueado real, renovação/revogação Google, transferências com Drive, `scripts/check-live-sync.py`, portais de pasta/background/autostart, abertura de links Workspace no GNOME Files ou instalação/build final Flatpak. O script live requer opt-in e fixtures de conta controlada e pode mover arquivos da fixture; não foi executado contra dados pessoais.
- Não foram medidos cobertura percentual, mutation score, desempenho com inventários grandes, disco cheio, comportamento em filesystems sem as operações exigidas, suspensão/retomada, falhas reais de energia ou durabilidade em hardware. Não houve SAST/DAST especializado, fuzzing, auditoria do histórico de segredos ou revisão completa do código transitivo.
- O acervo histórico e os documentos de casos de teste não foram contados como testes executáveis. Não há workflows de validação em `.github/workflows` no checkout; execução local desta revisão não estabelece controle contínuo de regressões.

## Parecer

**Com ressalvas.** Não foi identificado bloqueador crítico/alto comprovado no escopo examinado. Há duas falhas concretas de recuperação que devem ser resolvidas e testadas antes de alegar retomada robusta após falhas, além de lacunas de integração real e manutenção de dependências. A versão Tokio sinalizada merece atualização, embora a condição vulnerável não tenha sido identificada no build atual. O resultado não autoriza concluir prontidão para release estável, ausência de perda de dados em qualquer cenário ou garantia de segurança.
