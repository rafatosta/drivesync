# Conflitos — testes

A suíte em `engine/src/analyzer/conflicts/tests.rs` utiliza snapshots sintéticos
de Models/Inventário, com expectativas independentes. Não acessa rede, banco ou
filesystem, não chama Reconciliação e não executa ações.

| Critério | Cobertura |
| --- | --- |
| CF-01 | Equivalência confirmada, convergência após mudanças idênticas e conteúdo desconhecido apesar de tamanho igual |
| CF-02 | Mudança unilateral de conteúdo ou nome nos dois sentidos |
| CF-03 | Conteúdos concorrentes comprovadamente distintos exigem intervenção |
| CF-04 | Nome alterado localmente e conteúdo alterado remotamente permanecem compatíveis |
| CF-05 | Exclusão versus mudança de conteúdo, nome e pai, nos dois sentidos |
| CF-06 | Matriz de presença/cobertura, snapshots ausentes, baseline não confirmado e campos recentes desconhecidos |
| CF-07 | Resolução automática retirada do contrato por solicitação do usuário; não há ações/opções na saída |
| CF-08 | IDs/revisões inválidos, conta/provedor incompatível, tipo contraditório, ausência de snapshot obrigatório e evidência contraditória |
| CF-09 | Classificação coerente com findings; intervenção somente em conflito comprovado; requisitos preservados mesmo com conflito |

Os casos transversais CF-S01 a CF-S08 e CF-S10 são verificados por repetibilidade
com chamadas intercaladas, identidades qualificadas, metadados desconhecidos,
baseline ausente, nomes em bytes, conflitos por dimensão e ausência de inferências
por tamanho/tempo. CF-S09 pertence à futura implementação de opções de resolução:
nenhuma alternativa `KeepBoth` ou outra ação é oferecida nesta etapa.

Verificar separadamente que `NoConflict` não equivale a sincronização concluída,
que falta de evidência não solicita escolha humana e que a presença de um conflito
não oculta lacunas em outras dimensões. A cobertura não abrange metadados
específicos sem política/baseline suficientes.


## Validação executada

No `org.gnome.Sdk//50`, com a extensão `rust-stable` definida no manifesto
Flatpak e cache local em `CARGO_HOME=/home/tosta/.cargo`:

```sh
cargo fmt --all --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace
cargo doc --offline --workspace --no-deps --document-private-items
cargo test --offline --workspace --doc
cargo test --offline -p drivesync-engine analyzer::conflicts --lib
```

Os 22 testes do Conflicts passaram; o workspace teve 753 testes aprovados e
10 ignorados, além de 6 doctests aprovados. Formatação, Clippy e rustdoc passaram.
Links relativos e `git diff --check` foram verificados. Não houve teste com
Google real ou aplicativo instalado; esta entrega identifica conflitos sem I/O.
