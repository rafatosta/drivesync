# Orchestrator — testes do fluxo contínuo

A suíte `engine/tests/orchestrator.rs` usa Detector/Scanner/Analyzer/Inventory reais,
contas sintéticas e SQLite temporário. A aplicação não usa SQL no coordenador;
conexões SQL adicionais existem somente nos testes de injeção de falha.

## Casos verificáveis

- Início de Local e Remote antes do scan, barreira de Scanner completo e entrega
  Enabled somente depois da preparação/análise inicial.
- Criação real de pasta e arquivo depois de `prepare`: ingestão e recibos atuais
  aparecem sem rescan manual, sem cruzar contas e sem executar sincronização.
- Rename e remoção locais: observações/rechecagens atualizam Inventory/Analyzer,
  sem atribuir ao Orchestrator uma política física de rename/delete.
- Stop/restart e criação durante parada: Scanner recupera o estado, seguido por
  consumo contínuo; eventos em captura permanecem duráveis.
- Falhas em `receipts` e `analyses` durante processamento ao vivo: pausa visível,
  lease liberada, zero ack antecipado e recuperação sem duplicar File.id.
- Scan parcial: entrega continua pausada e nenhuma análise é liberada indevidamente.
- Esquema incompatível preservado, configuração por conta, alias de raiz remoto,
  caller sem Tokio, preparação concorrente e parada sem Ready posterior.
- Intenções de upload/rename ficam Blocked com zero tentativas; bytes e baseline
  permanecem intactos. A interpretação atual do Analyzer não foi alterada.

Os testes privados `orchestrator::ingestion::tests` exercitam observações remotas
com metadados reais dos modelos: ingestão → Analyzer → recibo, reentrega idempotente,
remoção indeterminada, aviso de raiz sem arquivo fictício e rejeição de escopo ou
recurso desconhecido sem snapshot. Não requerem OAuth ou rede.

## Aplicação e interface

O teste gráfico `application_startup_delivers_live_files_to_the_visible_analyzer_without_manual_refresh`
usa o mesmo hook de restauração da aplicação. Antes de existir página Developer,
a conta é restaurada e o pipeline começa. Depois de abrir o Analyzer, o teste cria
pasta/arquivo e aguarda a linha correspondente sem chamar prepare/scan/analyze ou
Refresh. Também verifica ack, contador da sessão, preservação dos bytes e parada
na remoção da conta. Perfil XDG/Wayland e conta são sintéticos.

Os testes gráficos anteriores de Detector/Scanner foram ajustados para entrega
ativa e consumo, mantendo pausa somente nos casos Partial/parada. Capturar e
confirmar eventos não comprova upload/download ou integração com Google real.

## Limites

Leases antigas aguardam expiração pela API do Detector; não há renovação ou acesso
SQL para roubar lease. Eventos remotos sem snapshot e sem associação conhecida
continuam pendentes com erro explícito. A segurança entre bancos depende de
idempotência/reentrega e do guard de release, não de transação distribuída.
Não declarar validação de Operator, política final do Analyzer ou contas reais.

## Validação executada nesta entrega

SDK `org.gnome.Sdk//50`, extensão rust-stable do manifesto e cache
`CARGO_HOME=/home/tosta/.cargo`:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo build --offline --bin drivesync
cargo test --offline -p drivesync-engine --test orchestrator
cargo test --offline -p drivesync-engine orchestrator::ingestion --lib
cargo test --offline --workspace
cargo doc --offline --workspace --no-deps --document-private-items
cargo test --offline --workspace --doc
```

Passaram 17 testes públicos do Orchestrator, três testes privados de ingestão,
792 testes no workspace e 9 doctests; 12 testes dependentes de ambiente ficaram
ignorados por padrão. Formatação, Clippy, build e rustdoc passaram sem avisos Rust.
O teste gráfico de abertura real/atualização ao vivo foi executado explicitamente
com `--ignored --exact --test-threads=1` em Wayland/Mutter headless, perfil XDG
sintético e renderer Cairo, e passou. A captura resultante foi inspecionada.

`git diff --exit-code HEAD -- engine/src/detectors engine/src/scanner engine/src/inventory engine/src/analyzer`
confirmou que as interfaces/esquemas dos módulos e as regras do Analyzer não foram
alterados. Não houve instalação de aplicativo nem teste com conta Google real.


## Integração de criação de réplicas

A suíte do Operator combina Analyzer real, Inventory SQLite, filesystem temporário e
Provider real contra HTTP loopback: download/upload, pastas, falha na resposta,
backoff, recuperação do mesmo ID e baseline somente após reobservação. Nova análise
sem mudanças conserva `Synced` e não repete a transferência. Os testes antigos de
observação usam `execute_creations: false` explicitamente.

As páginas Operator/Orchestrator foram verificadas no teste gráfico isolado
`ui::developer::detector::page::tests::live_account_selection_and_detector_layout`,
em 1000×720 e 480×640, usando a mesma sessão sintética e sem iniciar outro pipeline.

O harness `observe_account` só habilita execução real com
`DRIVESYNC_LIVE_OBSERVATION=1` **e** `DRIVESYNC_LIVE_EXECUTION=1`, além da conta e raiz
explícitas. Na conta de testes, houve download e criação de pastas reais; análise
posterior confirmou 12 itens como sincronizados. A validação real revelou HTTP 411
na consulta de retomada de upload, por falta de `Content-Length: 0`. Com autorização
do usuário, o Provider passou a enviar o comprimento explícito das consultas e
blocos, inclusive vazios. O teste de cabeçalho falhou antes e passou após a correção.
Na nova execução real, a sessão original concluiu o envio do arquivo vazio com o
mesmo ID e checkpoint `Completed/Applied`, sem HTTP 411. A confirmação de baseline
permaneceu bloqueada porque a versão remota avançou de 1 para 2 na reobservação;
o upload físico concluído não foi apresentado como sincronização confirmada.
Isso não equivale a validar instalação do Flatpak.
