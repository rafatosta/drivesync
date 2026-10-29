# LOCAL-001/002 — inventário e estabilização

Requisitos e políticas: [SYNC-003](../../../docs/SOFTWARE_SPECIFICATION.md#sync-003--inventário-e-reconciliação).

Entradas: raiz autorizada, cancelamento. Saída: inventário completo, ordenado,
com caminhos relativos e evidências; ou erro explícito sem publicação parcial.
Varredura limitada a 100.000 itens, sem symlinks, por descritores relativos.
Diretórios inacessíveis invalidam a varredura; arquivos inacessíveis permanecem
UNKNOWN. Arquivos internos são excluídos; temporários conhecidos ficam visíveis
como pendência. Arquivos regulares recebem assinatura device/inode/tamanho/
mtime/ctime, comparação após quiescência e SHA-256 de leitura revalidada.

Não seguir links, não escrever dados do usuário, não inferir identidade de nome
ou bytes, não deduzir exclusão por erro. Cancelamento é verificado na varredura,
na espera em intervalos curtos e durante a leitura. Não há retry ilimitado:
arquivo instável fica pendente até nova observação no próximo ciclo.

O upload revalida a assinatura observada antes do snapshot; alterações durante
sua cópia invalidam o snapshot. O snapshot privado limita a versão enviada aos
bytes verificados; não comprova que um aplicativo não retomará uma gravação.

Oráculos: arquivo regular/oculto legítimo, temporários, edição na janela,
substituição por mesmo nome/bytes, symlink de ancestral, raiz trocada, cancelamento,
falha de leitura, limite, repetição e eventos ausentes durante offline.

Fundamentação consultada em 2026-10-29:
- [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html): notificações
  podem perder eventos; cache exige reconstrução e verificação de consistência.
- [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html): eventos são
  notificações, não journal durável nem prova de fechamento de escritor.
- [Drive files](https://developers.google.com/workspace/drive/api/reference/rest/v3/files):
  versão e checksums são evidências distintas; checksum não é universal para todos os tipos.
