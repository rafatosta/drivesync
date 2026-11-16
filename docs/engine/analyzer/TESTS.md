# Analyzer — testes da integração de persistência

`engine/tests/analyzer.rs` usa a fachada pública com Inventory SQLite real e
temporário, snapshots sintéticos e expectativas independentes dos algoritmos.
Os testes não executam transferências nem acessam contas reais.

| Cenário | Verificação |
| --- | --- |
| Item consistente | Recibo `NoChanges` durável; nenhum comando, operação ou avanço do baseline |
| Mudança unilateral nos dois sentidos | Decisão correta de conteúdo, `OperationId` vinculado e estado `Blocked` |
| Conflito de conteúdo com rename independente | Findings dos algoritmos e rename persistem juntos; nenhuma resolução |
| Evidência insuficiente | Pendência de evidência, sem conflito humano inventado |
| Retry/reanálise idêntica | Mesmo recibo e ausência de duplicação da fila |
| Snapshot adulterado ou escopo errado | Erro sanitizado e zero gravações |
| Evidência contraditória | Erro tipado, limpeza da atividade e Inventory intacto |
| Falha SQL injetada | Rollback de avaliação/status/revisão/fila e retry posterior |
| Construção com conta incompatível | Recusa antes de analisar |
| Worker em fila e cancelamento do future | Status ativo e `AlreadyRunning`; reserva dura até fim do worker; recibo recuperável |
| Arquivo remoto novo e ausência local comprovada | Criação semântica persistida, bloqueada por destino/evidência |
| Duas instâncias no mesmo item | Um único recibo durável |
| Baseline existente não confirmado | Evidência pendente, sem reinterpretar como criação inicial |

O teste de cancelamento satura o pool bloqueante com canal de sincronização e
verifica a reserva real da análise antes de liberar o worker, sem sleeps como
prova de progresso. Doctests verificam uso da fachada e falha de compilação ao
importar os algoritmos privados.

`engine/tests/inventory_analysis.rs` complementa a suíte com reabertura, paginação,
obsolescência, duas conexões, unicidade, rollback integral e recusa de claim/requeue
para intenções incompletas. As suítes puras de Reconciler/Conflicts permanecem
independentes e são executadas junto ao workspace.

Execução de comandos, progresso de transferência, confirmação após efeitos,
opções de resolução e recuperação de efeitos físicos continuam fora do escopo.
Aprovar estes testes não comprova essas funcionalidades ou integração real com
Google, aplicativo instalado ou GUI. A integração pós-scan do Orchestrator possui
suíte própria em `engine/tests/orchestrator.rs`.

## Validação executada nesta entrega

No `org.gnome.Sdk//50`, com a extensão `rust-stable` do manifesto e cache local
`CARGO_HOME=/home/tosta/.cargo`:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline -p drivesync-engine --test analyzer
cargo test --offline --workspace
cargo doc --offline --workspace --no-deps --document-private-items
cargo test --offline --workspace --doc
```

Os 13 testes públicos da fachada passaram. A validação integrada teve 776 testes
aprovados, 10 ignorados e 9 doctests aprovados; formatação, Clippy e rustdoc sem
avisos. Links relativos e `git diff --check` passaram. Os 10 testes novos da
persistência foram validados também com as suítes públicas existentes do Inventory
(32 testes aprovados nessa etapa). Nenhum banco real de usuário foi aberto.

## Direção de replicação inicial

A suíte pura verifica os dois sentidos para itens inicialmente sem vínculo,
conservação de `Unknown`, requisitos explícitos e determinismo. Vínculo prévio,
baseline ou observação parcial não autorizam esse caminho. A suíte pública grava
um download a partir de observação remota inicial, verifica retry idempotente,
reabertura SQLite, fila bloqueada, baseline ausente e nenhum arquivo criado.
A suíte do Orchestrator verifica uploads após scan e após criação de arquivo/pasta
com a sessão aberta, sem chamar análise manualmente. Conflitos continuam cobertos
pelos cenários públicos de mudanças concorrentes.

## Aceitação posterior com Google Drive real

A validação autorizada de 2026-11-15 incluiu 23 verificações sobre arquivos reais,
SQLite real, Accounts/Secret Service, captura remota Google e um teste gráfico do
hook de restauração. Resultados, comandos de repetição e limites concretos em
[LIVE_VALIDATION.md](LIVE_VALIDATION.md). Os números históricos acima descrevem
as entregas anteriores, não essa rodada de aceitação.
