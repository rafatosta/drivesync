# Analyzer — aceitação com conta real

## Execução de 2026-11-15

Com autorização do usuário, a conta RTosta foi observada pelo código atual no
SDK GNOME 50, usando a pasta local autorizada e Google Drive real. O Inventory
foi criado no esquema 6 depois de o usuário remover o banco antigo. Não houve
migração ou remoção de banco pelo teste. A outra conta não foi iniciada.

O cliente do próprio aplicativo autenticou via Accounts/Secret Service e recebeu
cobertura remota completa, sem falhas. O conector Drive criou/modificou somente
fixtures identificadas por `drivesync-e2e-20261115-2132` e pela pasta
`DriveSync Analyzer E2E 2026-11-15 2132`. Esses efeitos prepararam os cenários;
não foram feitos pelo Analyzer ou pelo Operator.

Foram aprovadas 23 verificações automatizadas com snapshots do SQLite real:

| Grupo | Resultado observado |
| --- | --- |
| Criação local | Pasta e arquivo geraram CreateDirectory/Upload |
| Criação remota | Pasta e arquivo geraram CreateDirectory/Download |
| Homônimos local/remoto | IDs distintos e colisão contextual, sem vínculo automático |
| Alteração e rename locais | Bytes novos observados e identidade preservada |
| Move local | Mesmo ID com novo caminho |
| Rename remoto | Mesmo file ID com novo nome observado |
| Conteúdo remoto | SHA-256 recebido igual ao calculado sobre os bytes de teste |
| Move remoto | Mesmo file ID com novo pai observado |
| Reinício | Scanner recuperou criação e rename durante a parada |
| Associação explícita | Escolha por IDs, proveniência persistida e nenhum baseline criado |
| Pais associados explicitamente | Destinos dos filhos resolvidos pelos IDs existentes |
| Rajada | 25 arquivos criados rapidamente receberam análises |
| Lixeira local | Ausência confirmada; restauração preservou identidade e foi reanalisada |
| Comparação de conteúdo | SHA-256 real classificou como Different os recursos explicitamente associados |
| Duplicatas remotas | Mesmo nome e bytes mantiveram IDs distintos, com colisão |
| Atualidade | Todas as análises ativas mais recentes coincidiram com a revisão e a época de observação atuais |
| Persistência e efeitos | SQLite íntegro, operações Blocked com zero tentativas, zero baselines, sem download/sobrescrita |

Adicionalmente, o teste gráfico abaixo passou com autenticação Google real,
perfil GTK privado e metadados da conta RTosta. O hook real de restauração da
aplicação iniciou o pipeline. Um arquivo criado após a abertura apareceu no
Analyzer sem refresh manual; o painel continha direções Upload e Download.
O teste usa somente uma conta, verificada antes de iniciar a restauração.

## Repetição controlada

`engine/examples/observe_account.rs` usa as fachadas públicas Engine e
Orchestrator, sem Provider falso e sem executor. Exige opt-in e configuração
explícita por variáveis; não contém IDs, credenciais ou pastas de contas fixos.

- `DRIVESYNC_LIVE_OBSERVATION=1`
- `DRIVESYNC_LIVE_ACCOUNT`: ID local da conta autorizada.
- `DRIVESYNC_LIVE_APPLICATION`: identidade original usada pelo Secret Service.
- `DRIVESYNC_LIVE_ACCOUNTS`: diretório de metadados Accounts.
- `DRIVESYNC_LIVE_ROOT`: pasta local autorizada.
- `DRIVESYNC_LIVE_REMOTE_ROOT`: ID real da raiz remota configurada.
- `DRIVESYNC_GOOGLE_OAUTH_FILE`: configuração OAuth de desenvolvimento.

Executar somente dentro do SDK do manifesto, com acesso à pasta autorizada,
rede e Secret Service da sessão original:

```sh
cargo run --offline -p drivesync-engine --example observe_account
```

Comandos de entrada: `status`, `prepare`, `quit`. O comando
`associate LOCAL_ID REMOTE_ID` registra uma escolha explícita desses dois IDs;
não compara nomes e não confirma baseline. Após uma associação, usar `prepare`
para reavaliar. Não executar dois observadores sobre o mesmo escopo ao mesmo tempo.

O teste gráfico opt-in, também dentro do SDK, requer display Wayland isolado,
perfil `XDG_DATA_HOME` sob `/tmp/drivesync-ui-test-real-`, backend GSettings memory
e acesso ao Secret Service original. Fornecer um diretório Accounts privado com
apenas a conta escolhida, preservando a identidade original da aplicação e sua
referência ao cofre; não copiar ou imprimir tokens. O teste deixa uma fixture
`drivesync-e2e-ui-*.txt` para inspeção.

```sh
cargo test --offline --bin drivesync \
  ui::developer::analyzer::page::tests::real_account_startup_observes_drive_and_local_file_in_analyzer \
  -- --ignored --exact --test-threads=1 --nocapture
```

## Limites concretos

- Sem baseline confirmado depois do reset, associação e mudanças de recursos já
  existentes conservam requisitos de baseline; não se atribui uma direção de
  atualização/rename/move sem a prova necessária. Os cenários com baseline e
  conflitos concorrentes continuam cobertos pelas suítes isoladas do workspace.
- A lixeira/restauração remota não foi exercitada nesta execução: o conector não
  expõe trash e o navegador disponível não tinha sessão Google autenticada.
  Não se usou exclusão permanente como alternativa.
- O teste gráfico usa o código no SDK, não uma instalação/atualização do Flatpak.
  Avisos dos portais no display isolado não constituem aceitação dos portais ou
  da acessibilidade do aplicativo instalado.
- O Operator permanece sem implementação e não foi chamado. Os arquivos de teste
  ficam disponíveis para inspeção; não há limpeza permanente.

## Regressão da infraestrutura de testes

Uma repetição paralela expôs reutilização de porta no teste HTTP de conexão
recusada, causando duas falhas correlacionadas nos servidores simulados. A fixture
agora mantém uma porta TCP reservada sem listener, impedindo que a requisição
alcance outro teste. Duas execuções consecutivas dos 301 testes do Provider
passaram após a correção, sem alteração da implementação do Provider.
