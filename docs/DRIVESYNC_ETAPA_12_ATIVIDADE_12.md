# Etapa 12 — Atividade 12 — Exclusão autorizada para lixeira

**Resultado: BLOQUEADO.** A configuração padrão do Flatpak separa a pasta sincronizada da lixeira privada em montagens distintas. A remoção local continua recusada quando a movimentação atômica e o recibo da Engine não podem ser garantidos.

Em 24/11/2026, testei a Engine com a conta `rtosta.coc@gmail.com`, no escopo descartável `1hVKA2thKgb8aYPTNW2S1W1gt5YSL2ssB`. Os itens dos cenários principais tinham IDs e baselines próprios. O roteiro independente foi executado no ambiente Flatpak Devel, usando uma cópia isolada dos metadados da conta.

| Cenário | Observação |
| --- | --- |
| Opt-in desligado | `local-trash.bin` já tinha recibo local válido. O Drive manteve o mesmo ID com `trashed=false` e o Inventory ficou intacto. |
| `TrashRemote` autorizado | Com o recibo local e opt-in, o Inventory confirmou a operação e os dois estados ausentes. Uma leitura independente do mesmo ID no Drive retornou `trashed=true`. Os bytes locais continuaram na lixeira da fixture. |
| Saída de escopo | Mover `outside.bin` para fora da raiz sem criar recibo não enviou o item remoto para a lixeira, mesmo com opt-in. A leitura do Drive confirmou `trashed=false`. |
| `RemoveLocal` autorizado na mesma montagem | O envio de `remote-trash.bin` para a lixeira do Drive levou à confirmação do `RemoveLocal`. O Inventory preservou a identidade, os dois estados ausentes e a referência à lixeira. O arquivo retido e o recibo com o caminho original permaneceram na fixture. |
| `RemoveLocal` entre montagens | O envio de `unavailable.bin` para a lixeira do Drive produziu uma operação local falha. A cópia local permaneceu legível, com os bytes originais, e os baselines permaneceram intactos. |
| Reinício | O primeiro reinício voltou a usar a lixeira da mesma montagem. Com a condição técnica restaurada e a sessão novamente autorizada, a Engine fez uma nova execução válida para `unavailable.bin`, o que invalidou a asserção de igualdade com o baseline anterior. Um segundo reinício verificou estabilidade dos tombstones e ausência de replay das operações confirmadas. |

No Flatpak instalado, a sonda executou uma única chamada de renomeação em arquivo descartável. Origem e destino estavam no dispositivo 54 e em montagens distintas (2632 e 2631). O kernel retornou `EXDEV` (18). O arquivo permaneceu na origem com SHA-256 `1bb505bf486385b90b9240a8553ab3799a36eee5f26ea304ea993f12835cf257`. A Engine agora recusa a diferença de montagem antes de publicar um recibo. O mecanismo GIO/portal da etapa 11 permanece independente do recibo privado. A operação segue restrita a lixeira recuperável.

A API pública informa destino, operação em andamento, recusa ou necessidade de verificar a recuperação. `SyncActivity` mostra a confirmação apenas após verificação persistida e sem progresso em bytes. `SyncAttention` abre o `SyncIssueDialog` com a descrição do próprio item. No pacote instalado `stage12-event12`, OSTree `c5b4dbac6a89411d04e18850015d58a15521f7634fa591ad7d24f95a740f9297`, uma fixture de sessão padrão produziu a recusa de `outside.bin`. O clique por AT-SPI abriu o diálogo com o mesmo item. Uma biblioteca de teste isolou a instância GTK para não encaminhar a invocação a outro processo. A configuração da GUI continua usando a sessão padrão sem opt-in. A atividade em execução, a confirmação e o aviso de recuperação foram verificados por testes de mapeamento e regressões da Engine. A apresentação desses estados durante uma sessão autorizada no pacote instalado permanece sem homologação.

Regressões no SDK GNOME 50: `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` e teste GTK isolado de componentes. O conjunto inclui pré-condições de identidade e escopo, lixeira indisponível, validação de recibo local, substituição do diretório do recibo e recuperação sem replay.

As evidências estão no workspace de validação, em `relatorios/etapa-12-atividade-12`: `live-*.log`, `installed-mount.log`, `retained-files.log`, `installed-atspi.log`, `workspace.log` e `gtk-components.log`. Os arquivos enviados para a lixeira e seus recibos foram preservados.
