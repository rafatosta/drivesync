# Fontes técnicas de referência (catálogo v0.3)

Este catálogo contém **referências primárias** para orientar as especificações; cada algoritmo declara quais utiliza e quais diferenças exigem política própria.

| Chave | Fonte técnica | Aplicação | Limite relevante |
|---|---|---|---|
| R-OAUTH | [Google OAuth para apps desktop](https://developers.google.com/identity/protocols/oauth2/native-app) | auth: navegador externo, PKCE, tokens | autorização/consentimento e configuração Google Cloud |
| R-PKCE | [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636) | auth: proof key | não define armazenamento seguro de tokens |
| R-NATIVE | [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252) | auth: apps nativos | regras para redirect e clientes públicos |
| R-SCOPES | [Google Drive API: escopos](https://developers.google.com/workspace/drive/api/guides/api-specific-auth) | auth/remote: escopo drive vs drive.file | amplo acesso exige avaliação de escopo restrito |
| R-CHANGES | [Google Drive: retrieve changes](https://developers.google.com/workspace/drive/api/guides/manage-changes) | changes: delta, cursor, paginação | token/cursor e mudanças remotas não bastam para conflitos locais |
| R-UPLOAD | [Google Drive: upload data](https://developers.google.com/workspace/drive/api/guides/manage-uploads) | upload: sessões resumable | sessão/offset não equivalem a operação transacional local+remota |
| R-DOWNLOAD | [Google Drive: download/export](https://developers.google.com/workspace/drive/api/guides/manage-downloads) | download: arquivos binários e documentos nativos | arquivos nativos exigem tratamento separado |
| R-ERRORS | [Google Drive: handle errors](https://developers.google.com/workspace/drive/api/guides/handle-errors) | remote/queue: errors, retry/backoff | 404 pode ser não encontrado **ou inacessível**; interpretar reason |
| R-SQLITE | [SQLite: atomic commit](https://www.sqlite.org/atomiccommit.html) | state: commit durável | não abrange transação atômica com Google Drive API |
| R-WAL | [SQLite: WAL](https://www.sqlite.org/wal.html) | state: journaling e concorrência | fsync/durabilidade dependem da configuração e do filesystem |
| R-RENAME | [Rust std::fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html) | download: staging no mesmo filesystem | sem garantias universais em mounts diversos ou races |
| R-MONITOR | [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html) | local: eventos filesystem | eventos podem exigir varredura de reconciliação |
| R-HOLD | [GIO Application hold](https://docs.gtk.org/gio/method.Application.hold.html) | ui: janela oculta/processo ativo | ciclo de vida do Flatpak deve ser testado no alvo |
| R-HTTP | [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110) | remote/upload: semântica HTTP | não presumir suporte da Drive API a precondições condicionais sem prova |
| R-DBUS | [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html) | ipc/notifications | IPC externo é opcional, não necessário entre GUI e motor |
| R-FLATPAK | [Background portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html) | configuration/ui: autostart | permissão e suporte variam conforme ambiente |

## Verificação de fonte
O link não representa aprovação de uma implementação particular. Para cada algoritmo, registrar URL e assunto específico aplicado, além do comportamento que **não** é garantido pela fonte.

## Lacuna crítica a investigar
Antes de habilitar substituições remotas bidirecionais, confirmar empiricamente e na documentação **qual mecanismo de precondição atômica por revisão/versão está disponível na Drive API v3**. Comparar metadados antes do upload não elimina uma corrida entre leitura e escrita; se não houver precondição adequada, definir política conservadora que preserve versões e reconcilie conflitos. Não inventar garantia de compare-and-swap.
