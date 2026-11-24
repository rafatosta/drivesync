# Etapa 12 — Atividade 13 — Projeções do Google Workspace

Docs, Sheets, Slides, Drawings e Forms são projetados como `<nome>.html`, com redirecionamento HTTPS e link alternativo para o ID original codificado. O arquivo é publicado com modo `0444`. DOCX e PDF continuam no fluxo binário. A Engine preserva associações existentes com `.desktop` e sinaliza a divergência. A troca de representação não faz migração automática nem remove arquivos antigos.

Em 24/11/2026, a validação com `rtosta.coc@gmail.com` no SDK GNOME 50 usou o escopo descartável `1QMaQjw4-eLMwnnuPw3Vh4EkPskyLkdmb`.

| Cenário | Evidência |
| --- | --- |
| Criação e colisão | Docs e Sheets geraram HTML canônico de modo `0444`. A colisão com `Collision.html` bloqueou a projeção. O teste unitário cobriu os cinco MIME types, limites UTF-8, escape e DOCX/PDF. |
| Renomeação e movimentação | `Report.html` tornou-se `Renamed.html` e depois `Folder/Renamed.html`, com os mesmos ItemId, RemoteId e inode. O baseline confirmou o novo caminho. |
| Edição e substituição locais | A Engine preservou os bytes adulterados e o baseline do item. A leitura independente do ID no Drive confirmou o mesmo MIME e a mesma versão. |
| Reinício e perda do recibo | O reinício real preservou os registros confirmados. Um teste com recibo perdido recuperou a publicação e confirmou uma revisão remota mais recente sem repetir a criação. |
| Flatpak instalado | O ramo `stage12-event13`, OSTree `4ac931e119754808825a36eebf11a48dc828959f15b0697c81c9c85f55c5751f`, gerou `Folder/Inside.html` e `Folder/Renamed.html`, ambos com modo `0444`. AT-SPI registrou a atividade de criação do atalho. O mapeamento automatizado cobriu criação e atualização, em execução e confirmadas, sem progresso em bytes. A atividade de atualização e a abertura no navegador não foram observadas no pacote instalado. |

No SDK passaram `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (706 testes, 27 opt-in ignorados no conjunto padrão), `cargo doc --offline --workspace --no-deps --document-private-items` e `cargo test --offline --workspace --doc`.

As evidências estão em `relatorios/etapa-12-atividade-13` no workspace de validação, incluindo `live-*.log`, `final-checks.log`, `final-regressions.log`, `package.log`, `installed-atspi.log` e `installed-files.log`.
