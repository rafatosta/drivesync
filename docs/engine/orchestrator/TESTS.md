# Orchestrator — testes

`engine/tests/orchestrator.rs` utiliza Detector/Scanner/Analyzer/Inventory reais,
SQLite temporário, contas sintéticas sem OAuth e arquivos isolados. A coordenação
não acessa armazenamento interno nem SQL; apenas os testes usam conexão SQLite
adicional para injetar falha e verificar preservação.

## Preparação existente

Os sete cenários originais permanecem: início das duas fontes e conector;
fila pausada/reabertura/isolamento sem ack; scan parcial e repetição; esquema
incompatível preservado; parada sem publicação final de Ready; preparação
concorrente rejeitada; recuperação após falha inicial da captura local.

## Integração do Analyzer

| Cenário | Garantia |
| --- | --- |
| Scan completo com arquivo/diretório | Um recibo durável por item; ausência remota não presumida; fila pausada sem ack |
| Rename remoto previamente observado e baseline confirmado | Decisão vinculada à operação Blocked; arquivo/nome local e baseline permanecem intactos |
| Ausência remota confirmada no Inventory | Intenção de criação Upload persistida sem upload ou tentativa de execução |
| Scan Partial | Nenhuma nova análise até recuperar cobertura completa |
| Falha SQLite durante registro da análise | Estado Failed com causa Analyzer; relatório Scanner completo preservado; retry posterior válido |
| Duas contas e reabertura | Recibos e itens isolados e histórico persistente preservado |
| Chamador sem runtime Tokio | `prepare` completa scan e análise por workers privados sem exigir runtime externo |

O teste de rename deixa o item em `Synced` antes da nova observação remota e prova
que a passagem visita todos os status, sem perder uma análise invalidada. Intenções
são conferidas pelo tipo/direção, `OperationId`, estado `Blocked`, tentativas zero
e ausência de `started_at`; comparar também bytes/nome locais e baseline.

A página Scanner adapta o estado `Analyzing`, mantém o rescan desabilitado durante
a passagem e informa decisões registradas. Seu teste gráfico existente acompanha
a preparação da aplicação em perfis sintéticos; teste ignorado por padrão exige
ambiente gráfico isolado para execução explícita. Compilar os alvos não comprova
validação visual.

## Limites

Não há ingestão contínua de lotes do Detector nesta entrega; ack/redelivery,
coleta de hashes locais, cobertura remota global, transferências e resolução de
conflitos permanecem fora desta suíte. Nenhum teste declara sincronização real
com Google ou aceitação do aplicativo instalado. As interfaces proprietárias dos
três módulos são verificadas também pela ausência de diff em seus arquivos.

## Validação executada

No `org.gnome.Sdk//50`, com a extensão `rust-stable` definida pelo manifesto e
cache `CARGO_HOME=/home/tosta/.cargo`:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline -p drivesync-engine --test orchestrator
cargo test --offline --workspace
cargo doc --offline --workspace --no-deps --document-private-items
cargo test --offline --workspace --doc
```

Passaram os 15 testes públicos do Orchestrator (oito novos), incluindo proteção
contra inferência de pai diferente ao comparar um alias de raiz remota. A execução
integrada teve 784 testes aprovados, 10 ignorados e 9 doctests aprovados. Formatação,
Clippy e rustdoc passaram sem avisos. Links relativos e `git diff --check` passaram.
`git diff --exit-code HEAD -- engine/src/detectors engine/src/scanner engine/src/inventory`
confirmou ausência de alterações nos três módulos nesta entrega. O teste gráfico
foi adaptado à mensagem real, compilado, mas não executado explicitamente.
