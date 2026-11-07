# Testes e evidências de validação

## Ambiente e resultados executados

As verificações Rust foram executadas dentro do Flatpak Devel definido por
`flatpak/com.rtosta.DriveSync.Devel.yml`, usando o GNOME SDK 50 e a extensão
Rust declarada pelo projeto. Cargo não foi executado no host.

| Comando interno executado | Resultado observado |
| --- | --- |
| `cargo build --release --locked` | Sucesso. |
| `cargo test --workspace --quiet` | Status do comando dentro do SDK: 0; 158 testes passaram e 21 foram ignorados. |
| `python3 scripts/tests/test_i18n.py` | Falhou: `test_catalog_covers_marked_messages` encontrou 10 mensagens do código não correspondentes ao catálogo `po/pt_BR.po`; `test_compiled_catalog_and_english_fallback` passou. |

`flatpak-builder --run` retornou status 1 ao redor do comando de teste, apesar
de o processo Cargo interno reportar status 0. Um probe separado executando
`exit 0` pelo mesmo wrapper também retornou 1. Por isso, o resultado da suíte
Rust acima é o status interno de Cargo, e não o status externo do wrapper.
Essa particularidade não foi corrigida.

Os testes não foram executados com uma conta Google real, Secret Service
persistente, portal desktop autorizado ou instalação/execução final do
aplicativo empacotado. A execução no SDK comprova somente compilação e testes
no ambiente de desenvolvimento definido no manifesto.

## Cobertura observada por área

| Área | Evidência de teste | O que verifica / limites |
| --- | --- | --- |
| Contas e validação | `engine/src/accounts/tests.rs` | Serialização/validação e operações do repositório JSON, incluindo rejeição de estados inválidos; não verifica migração ou execução real do app. |
| OAuth e credenciais | `engine/src/auth/tests.rs`, `engine/src/auth/storage/tests.rs` | Provider tests usam `FakeGoogle` e `MemoryVault`. Dois testes de vault estão ignorados por exigirem Secret Service privado. A autenticação real com Google não foi executada. |
| Paths e portal de documentos | `engine/src/configuration/tests.rs` | Testes de paths/casos locais; quatro testes relacionados a portal/exportações descartáveis estão ignorados por exigir grants/paths explícitos. |
| Detector local | `engine/src/local_detector/watcher/tests.rs` | Mudanças reais em diretórios temporários, filtragem, overflow/gaps, limites e rechecagem; não valida comportamento em filesystem de usuário em produção. |
| Estado local SQLite | `engine/src/local_state/store/tests.rs` | Persistência, transações, validação, limites, conflito de geração e recuperação do payload em banco temporário. |
| Detector remoto e journal | `engine/src/remote_detector/tests.rs`, `journal.rs` | Cursor, eventos e processamento usando providers de teste/fakes; não consulta Drive real. |
| Download e retomada | `engine/src/download.rs`, `engine/src/download/resume.rs` | Cenários com filesystem local/fixtures, checkpoints, verificação e reconciliação; não comprova download real de uma conta Drive. |
| Histórico | `engine/src/activity_history.rs` | Gravação, paginação e agrupamento/supressão de eventos no SQLite de teste. |
| UI GTK e navegação | `src/app/navigation/tests.rs`, `src/app/layout_tests.rs`, `src/features/preferences/window.rs` | Há verificações de rotas/layout e controles; testes que precisam de display estão ignorados e, portanto, não foram executados na suíte sem desktop. |
| Observação em tela | `src/features/dashboard/components/local_history/tests.rs` | Parte dos testes requer display e `XDG_DATA_HOME` isolado, por isso está ignorada. Os testes restantes não comprovam execução longa da UI em desktop. |
| Background | `src/core/background/tests.rs` | Teste requer sandbox isolado e permissão do portal; ignorado. |
| Internacionalização | `scripts/tests/test_i18n.py` | Extrai strings marcadas com `gettext`, compara ao catálogo e verifica compilação/fallback. A cobertura do catálogo falhou; compilação/fallback passou. |

## Testes ignorados e condições externas

O total de 21 testes ignorados na execução inclui casos condicionados por
display GTK, portal de documentos/background, caminhos/exportações
descartáveis e Secret Service privado. Os atributos `#[ignore]` registram
essas condições nos testes, por exemplo em
`src/app/layout_tests.rs`, `src/features/preferences/window.rs`,
`src/features/dashboard/components/local_history/tests.rs`,
`engine/src/configuration/tests.rs`,
`engine/src/auth/storage/tests.rs` e `src/core/background/tests.rs`.
Como foram ignorados, não contam como aprovação.

## Lacunas e limitações verificadas

- Não há evidência de teste de integração ponta a ponta da aplicação
  empacotada com Google Drive real.
- Upload de conteúdo local não foi identificado na implementação nem há teste
  correspondente identificado.
- A camada GTK tem testes que exigem display, mas eles estavam ignorados; o
  comportamento visual/interativo real não foi validado.
- Os testes externos de portal e Secret Service não foram executados.
- O teste de catálogo de português brasileiro falhou com 10 divergências.
- Não foi identificado teste de operação real da notificação/background em
  desktop autorizado.

***

Os testes automatizados do workspace incluem testes de unidade e testes locais
com doubles/fixtures. Não foi identificada uma suíte separada de integração
real contra serviços Google/portais. Os dois scripts diagnósticos existentes
(`src/bin/drivesync-local-diagnostic.rs` e
`src/bin/drivesync-remote-diagnostic.rs`) são ferramentas do produto, não
evidência por si mesmas de execução ou aprovação de teste de produção.

