# Etapa 12 — Atividade 10 — Divergência concorrente

### Evento: edição diferente da mesma réplica de cada lado

- Estado inicial / fixture / escopo — Em 24/11/2026, o Flatpak Devel estabeleceu uma cópia de `fixture-concurrent.bin` na conta de teste `rtosta.coc@gmail.com` (conta `63c390be462dab92901945ca4186e16b`). A pasta remota isolada era `1ZJZLvzQvDPph33--bzmajB1z3TbcZuXd`. A auditoria registrou cobertura completa, um item convergente e zero conflitos. O Drive estava na versão 2 e o Inventory na revisão 6.
- Ação — Com a aplicação parada, sobrescrevi a cópia local no mesmo inode e atualizei o conteúdo do mesmo ID remoto com bytes diferentes. Só então reiniciei a aplicação.
- Resultado esperado — Persistir a divergência sem transferência. Exibir a entrada em `SyncAttention` e permitir revisar as duas réplicas, adiar e reabrir.
- Detecção e evidência — O observador registrou o ID remoto `1PtzzpWZKfCXiV5sE-HwWyazqClhXT6Np` na versão 4. O Context apresentou os dois hashes para o item `4010b1118827a683d256f76fe624ba1a`. O Analyzer respondeu `Conflict []`, sem operações de propagação.
- Persistência — O Inventory preservou o baseline e gravou `08688f75606da7d44e237df5fee3ba4f` com motivo `Content`, evidência observada na revisão 6 e revisão pública 7. A releitura final encontrou `resolved=false` e `resolution=null`.
- API pública — `Engine::list_conflicts` recupera o registro pelo escopo ativo depois da reabertura. A extensão do resumo projeta os metadados observados de cada lado. A revisão do Inventory continua identificando a evidência submetida à revisão.
- Assinatura pública — Na primeira reconciliação, a versão anterior chegou a publicar o conflito. Na reabertura, o dashboard anterior mostrou inicialmente apenas o erro genérico, mesmo com o conflito já persistido. A versão corrigida confirmou via assinatura `Failed/OperationFailed` e via listagem o mesmo conflito, simultaneamente. A assinatura segue apresentando o estado corrente, sujeito à coalescência.
- Adapter e `SyncAttention` — A entrada antes dependia do bloqueio publicado no status naquele instante. O Adapter passou a combinar o status com a lista durável e a exibir a entrada de revisão ao lado do diagnóstico atual quando necessário.
- Controller e `ConflictResolver` — A seleção da sidebar continua controlando a assinatura e os diálogos do escopo ativo. A lista abre o componente de revisão para o item selecionado. O componente exibiu o tamanho, a data e o SHA-256 de cada lado, além da versão 4 do Drive. A reabertura exibiu a revisão 7. A troca para um registro local sem credenciais fechou a lista e retirou a atenção do dashboard. Ao retornar à conta de teste, a revisão reapareceu.
- Evidências — Em `relatorios/etapa-12-atividade-10` no workspace de validação estão os logs `baseline-retry.log`, `before.log`, `before-restart.log`, `after-switch.log`, a conferência `inventory-verified.json` e as capturas `attention.png`, `review.png`, `other-account.png` e `reopened.png`. A versão instalada de validação, com instrumentação temporária de leitura e cliques GTK fora do commit, foi `com.rtosta.DriveSync.Devel//stage12-event10`, OSTree `18bf852cb9cd05101f25fc280bd071a74c7929171e129eac6c597c09c617a8db`.
- Regressões — No SDK GNOME 50 com extensão Rust do manifesto, passaram `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace`, `cargo doc --offline --workspace --no-deps --document-private-items` e `cargo test --offline --workspace --doc`. O teste GTK `isolated_components_and_fake_backend` passou com `--ignored --test-threads=1`.
- Limites — Uma conta Google real e um registro de teste sem credenciais para troca de conta. A leitura independente no Drive confirmou o ID, tipo e tamanho de 43 bytes. O SHA-256 remoto veio do observador Google e coincidiu com o payload enviado. O diagnóstico transitório `OperationFailed` da reabertura permanece visível. O fluxo instalado acionou somente revisão, adiamento e troca de conta.
- Resultado — **APROVADO** para detecção e revisão da divergência concorrente. O conflito segue pendente, com ambas as réplicas intactas.

| Réplica | Bytes | SHA-256 |
| --- | ---: | --- |
| Baseline | 35 | `09846b6305f85ed84c25b320a6d42a994011f79ff3c047df8e906ab6b16bf746` |
| Computador | 40 | `c4f8558078c7bf85a5432337022fb76a4a8d004163f8ea5807b78be32715e433` |
| Drive | 43 | `d1898b439334b36acef68c9d50929c8364336d52a92a48cf0fe07f2e66c188ab` |
