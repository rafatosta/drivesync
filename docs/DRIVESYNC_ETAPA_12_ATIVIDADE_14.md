# Etapa 12 — Atividade 14 — Interrupção e recuperação

Em 24/11/2026, dois perfis descartáveis da aplicação Devel usaram a conta de teste `rtosta.coc@gmail.com` e raízes vazias dedicadas. Os checkpoints foram injetados no executor de teste, com persistência no Inventory. O reinício ocorreu em outro processo, com o Flatpak instalado abrindo o registro deixado pelo executor. Para admitir essas raízes já semeadas, o preparo das fixtures gravou o vínculo de teste após conferir dispositivo, inode e data de criação do diretório contra o registro durável. A admissão normal da Engine manteve suas verificações.

A Engine publica verificação quando encontra uma operação durável e passa a examiná-la. Enquanto o efeito permanecer incerto, observações do mesmo item e a varredura não apagam o bloqueio. A GUI apresenta a verificação atual sem progresso, direção ou conclusão fabricados. Quando o efeito continua incerto, o diálogo identifica o item e orienta a preservar as cópias. A assinatura continua coalescida e pode omitir estados transitórios.

## Matriz de evidências

| Operação e checkpoint | Resultado observado | Evidência |
| --- | --- | --- |
| Upload real interrompido após confirmar 262.144 de 786.432 bytes | Primeira abertura instalada mostrou a verificação de `resume.bin`. O reinício retomou o mesmo ID de execução, sessão e RemoteId reservado. A leitura independente pelo cliente Drive confirmou o SHA-256. O ItemId foi preservado e o inventário ficou com uma única operação confirmada e baseline `Synced`. | `live-prepare.log`, `checkpoints.log`, `installed-fixture-first-recovery-atspi.log`, `inventory-after.log`, `live-verify.log` |
| Upload real no mesmo checkpoint, com mudança dos bytes de origem entre processos | A origem modificada permaneceu intacta. O mesmo registro ficou `AppliedUnconfirmed`, no offset anterior, sem RemoteId associado nem baseline. A listagem direta da pasta no Drive, incluindo itens na lixeira, ficou vazia. O pacote final exibiu a atenção e abriu o diagnóstico em PT-BR via AT-SPI. | `live-blocked-prepare.log`, `local-after.log`, `live-blocked-verify.log`, `installed-blocked-atspi.log` |
| Intenção persistida antes do despacho e interrupção entre despacho e publicação | Os testes reabriram o Inventory e distinguiram intenção não despachada de efeito sem evidência suficiente. Staging incompleto e referências da transferência foram preservados. | `recovery_tests` e `operations::tests` em `final-source.log` |
| Download e edição com publicação ou troca local antes do recibo | Testes com servidor controlado confirmaram download sem nova transferência, preservação de bytes anteriores recuperáveis e de identidade após troca de inode. Uma origem alterada durante a reconciliação impediu confirmação. | `downloaded_bytes_including_empty_files_are_recovered_without_another_download`, `local_update_after_exchange_recovers_new_inode_and_preserves_old_bytes_in_trash`, `source_changes_during_remote_reconciliation_prevent_confirmation` |
| Renomeação, movimento, criação de diretório, projeção e lixeira entre efeito e recibo | Fixtures dos testes integrados cobriram revalidação de raiz, pais e marca de execução. Efeitos atribuíveis foram confirmados sem replay cego. Na lixeira local, a indisponibilidade inicial conservou a incerteza até a revalidação do recibo recuperável. | `recovery_tests` em `final-source.log` |
| Auditoria após novo reinício | A auditoria pública verificou uma réplica convergente, rodadas completas, zero operações incertas e zero conflitos no escopo recuperado. A reabertura do Flatpak final exibiu “Tudo sincronizado”. | `live-verify.log`, `installed-fixture-atspi.log` |

## Regressão e limites

No SDK GNOME 50, a suíte de workspace cobriu as integrações das atividades 1–13. Passaram 707 testes e 27 ficaram ignorados no conjunto padrão. O teste GTK de componentes e backend falso foi executado separadamente e passou. Também passaram rustfmt, Clippy com warnings tratados como erro, geração de documentação, doctests e `msgfmt --check`. As operações anteriores foram cobertas pela regressão automatizada. Seus aceites reais individuais não foram repetidos.

Flatpak final `stage12-event14`, OSTree `d72b013f66fa0afc4dcdb83cec822ae2041dd278fcf835ad7cc139585d0d2170`. A primeira abertura do pacote anterior à correção do texto exibiu também o cabeçalho gettext por causa de uma direção vazia. O pacote final trata a direção vazia sem consultar essa tradução. A reabertura final do escopo recuperado mostrou transitoriamente uma recusa de envio à lixeira antes da captura completa. A auditoria posterior confirmou convergência e o arquivo foi preservado. Esse diagnóstico transitório permanece pendente.

O bloqueio com origem alterada permanece ativo por construção da fixture. O aceite instalado cobre o upload retomável e a abertura do diálogo de recuperação via AT-SPI. Os outros checkpoints da matriz foram exercitados no SDK com fixtures controladas. Permanecem os limites de raiz portal FUSE para escrita, lixeira local instalada, teste simultâneo com duas contas Google e validação com leitor de tela.

Evidências em `relatorios/etapa-12-atividade-14` no workspace de validação. As duas fixtures foram preservadas para inspeção.

**Resultado da atividade 14** — Cenários de recuperação e bloqueio selecionados aprovados, com os limites registrados acima.
