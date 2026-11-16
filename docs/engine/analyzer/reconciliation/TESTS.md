# Reconciliador — TESTS

## Objetivo

Verificar que `reconcile` recebe exatamente um `SyncItem`, compara os estados disponíveis e devolve uma lista determinística de operações propostas, divergências e requisitos, **sem efeitos colaterais**.

Fonte de verdade: [INTERFACES.md](INTERFACES.md). Os casos descrevem testes unitários para implementação futura, com entradas sintéticas e expectativas construídas independentemente do algoritmo.

## Fixtures

- Um `SyncItem` com ID, revisão, baseline confirmado, identidades locais/remotas e metadados conhecidos.
- Estados `Present`, `AbsentConfirmed` e `Unknown`, com coberturas `Complete`, `Partial` e `Unknown`.
- Versões A, B e C com evidência confiável de conteúdo igual/diferente, sem inferir hash a partir de timestamps.
- Diretórios com pais identificados, renomeações e movimentações.
- Evidências e campos opcionais ausentes, snapshots contraditórios e baseline inexistente.
- Referências remotas qualificadas por conta para testar isolamento.

## Casos unitários de `reconcile`

| Caso | Entrada | Resultado esperado |
| --- | --- | --- |
| RC-01 | Baseline/local/remoto equivalentes e comprovados | Sem operações, divergências ou requisitos; item/revisão preservados |
| RC-02 | Só local alterou conteúdo | `UpdateContent` remoto com ID e precondições corretos |
| RC-03 | Só remoto alterou conteúdo | `UpdateContent` local, sem criar outro arquivo |
| RC-04 | Arquivo novo local, remoto `AbsentConfirmed` | `Upload` com pasta destino e nome verificados |
| RC-05 | Arquivo novo remoto, local `AbsentConfirmed` | `Download` para destino local seguro |
| RC-06 | Diretório novo local ou remoto | `CreateDirectory` no lado correspondente, com pai conhecido |
| RC-07 | Nome alterado exclusivamente no local | `Rename` remoto, preservando ID e conteúdo |
| RC-08 | Nome alterado exclusivamente no remoto | `Rename` local, preservando recurso |
| RC-09 | Pasta pai alterada exclusivamente no local | `Move` remoto por ID, sem reupload |
| RC-10 | Pasta pai alterada exclusivamente no remoto | `Move` local, sem download |
| RC-11 | Exclusão comprovada de um lado com outro inalterado | Só propor exclusão se autorização, identidade e precondições forem suficientes; caso contrário requisito |
| RC-12 | Local e remoto mudaram conteúdo diferentemente | Divergência `ConcurrentChanges`, sem escolher lado |
| RC-13 | Exclusão comprovada de um lado e edição do outro | Divergência `DeleteVsChange`, sem comando destrutivo |
| RC-14 | Nome local e conteúdo remoto alterados de modo compatível | Duas operações apropriadas, em ordem determinística, sem descartar mudanças |
| RC-15 | Presença `Unknown` ou cobertura parcial decisiva | `requirements` solicita reobservação; nenhum comando destrutivo |
| RC-16 | Baseline ausente e dois recursos preexistentes | Nenhuma equivalência presumida; exigência de verificar identidade/conteúdo |
| RC-17 | Dados suficientes para mudança, mas destino/pai não identificado | Requisito `VerifyParent` ou dependência, sem `OperationCommand` incompleto |
| RC-18 | Entrada contraditória ou identidade incompatível | `ReconciliationError` tipado, sem efeitos |
| RC-19 | Duas chamadas com a mesma entrada | Mesma lista, IDs de etapas, ordem, divergências e requisitos |
| RC-20 | Operações dependentes dentro do mesmo plano | IDs únicos, dependências válidas, ordenação sem ciclos |

## Testes transversais

| Caso | Garantia | Critério |
| --- | --- | --- |
| RC-S01 | Sem I/O | Nenhum acesso a Operador, Inventário, Providers ou filesystem |
| RC-S02 | Identidade remota | IDs iguais em contas diferentes não são correlacionados |
| RC-S03 | Exclusão segura | `None` em snapshot não equivale a `AbsentConfirmed` |
| RC-S04 | Integridade | Tamanho ou mtime iguais não bastam para afirmar mesmo conteúdo |
| RC-S05 | Preservação de dados | Conflito de conteúdo não produz `Upload`/`Download` que substitua a outra versão |
| RC-S06 | Compatibilidade de mudanças | Alterações de dimensões diferentes não são descartadas |
| RC-S07 | Comandos tipados | Cada operação contém origem/destino, IDs e precondições necessários |
| RC-S08 | Dependências | Nenhuma etapa pronta depende de outra pendente não confirmada |
| RC-S09 | Campos desconhecidos | Metadados `NotCollected`, `Unsupported`, `Unavailable` não são inventados |
| RC-S10 | Um item por chamada | Nenhuma operação de outro `SyncItem` é fabricada ou executada |
| RC-S11 | Plano vazio | Não inferir sincronização concluída quando há requisitos/divergências |
| RC-S12 | Revisão otimista | Revisão retornada corresponde exatamente ao item analisado |

## Critérios de aceite

1. Cobrir os vinte casos `RC-01` a `RC-20` e os doze casos transversais, incluindo negativos e invariantes.
2. Cada comando `Ready` deve ser completo e verificável com o contrato do Operador; caso contrário, devolver requisito/divergência.
3. O teste examina o conteúdo dos comandos, a ordem e a lista de dependências, e não apenas o número de operações.
4. As expectativas são construídas sem chamar o Reconciliador para gerar o valor esperado.
5. Testes não acessam rede, contas reais, SQLite nem filesystem; o algoritmo deve permanecer puro.
6. Onde tipos ou políticas externos ainda não estiverem definidos, marcar o cenário `BLOCKED` com a lacuna explícita, não inventar capacidades.
7. Testes Rust futuros serão executados exclusivamente no ambiente Flatpak do projeto; registrar comandos e resultados reais.

## Cobertura da implementação atual

A suíte unitária em `engine/src/analyzer/reconciliation/tests.rs` cobre o núcleo
sem I/O: comparações de conteúdo, nomes em bytes, pais, criações semânticas,
exclusão versus alteração, cobertura/presença, ausência de baseline, snapshots
recentes desconhecidos, identidades qualificadas e repetibilidade.

RC-01, RC-12, RC-13, RC-15, RC-16, RC-18 e RC-19 possuem verificações diretas.
RC-02 a RC-10 e RC-14 validam **decisões semânticas**, sem comprovar requisições do
Operador. RC-11 valida bloqueio seguro sem autorização de exclusão; RC-17 valida
mapeamento insuficiente. RC-20 e RC-S07/RC-S08 estão **BLOCKED** na integração de
comandos tipados, precondições e dependências. Os demais invariantes transversais
são exercitados no núcleo; RC-S06 não abrange propagação de metadados específicos,
que permanece pendente de política e baseline suficientes.

Os vinte casos originais continuam como critérios de integração; a aprovação dos
testes do núcleo não significa conformidade com todos os comandos previstos.

## Validação executada nesta entrega

No SDK `org.gnome.Sdk//50`, com a extensão `rust-stable` definida no manifesto
Flatpak e dependências locais em `CARGO_HOME=/home/tosta/.cargo`:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace
cargo doc --offline --workspace --no-deps --document-private-items
cargo test --offline --workspace --doc
```

Resultado: 20 testes do núcleo aprovados; 731 testes do workspace aprovados,
10 ignorados, além de 6 doctests aprovados. Formatação, Clippy e rustdoc passaram.
Links relativos e `git diff --check` também passaram. A tentativa inicial sem
cache explícito não conseguiu resolver crates.io; a validação final usou o cache
local em modo offline. Nenhuma integração com Google ou aplicativo instalado foi
exercitada; o Reconciler não acessa esses serviços.
