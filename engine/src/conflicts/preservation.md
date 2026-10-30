# SYNC-010 — preservação de conteúdo divergente (P02/P08)

## Política e limites

Uma divergência de conteúdo comprovada em binário gerenciado mantém o arquivo
local no caminho original e preserva a versão remota observada em uma cópia
local adicional. Nenhuma preferência escolhe um vencedor. A pendência permanece;
a cópia não confirma sincronização e nunca avança B. Resolução assistida,
substituição remota, exclusões, retenção e limpeza automática não são habilitadas.
Incompatibilidade, ausência, tipo especial, localização ambígua ou prova
incompleta bloqueiam efeitos e informam impossibilidade; não inventam cópias.

## Identidade e armazenamento

Reservar `DriveSync backup conflict-<SHA-256>.bak` ao lado do caminho original.
A chave deriva dos metadados remotos completos e do caminho de origem, dentro
do escopo cliente/conta/raiz remota/raiz local do SQLite. O namespace de backups
já é reservado pelo planner e excluído do scan/upload. O journal de preservação
é separado do journal de instalações: registra origem, ID/revisão remota,
identidades de raiz/pai/arquivo, tamanho, hash e fase. Registros não são apagados.
O nome é opaco e limitado; o vínculo legível fica no journal e no relatório.

## Algoritmo e recuperação

1. Exigir base confirmada, decisão de divergência de conteúdo, fingerprint local
   verificado e localização original. Revalidar raiz, pai e fingerprint local.
2. Buscar o registro pela chave determinística. `Installed` exige reabrir a
   cópia sem symlinks e verificar identidade/tamanho/SHA-256 antes de anunciá-la.
3. `Ready` primeiro procura a cópia instalada: se intacta, fsync do diretório e
   confirmar `Installed`, mesmo offline. Se ausente, verificar staging por
   identidade/tamanho/hash, revalidar metadados remotos/configuração e instalar
   com `RENAME_NOREPLACE`. Destino ocupado inesperado bloqueia, mesmo com bytes
   iguais; nunca adotar nem substituir uma cópia de origem desconhecida.
4. Sem registro, persistir `Started` antes de criar staging exclusivo. Baixar
   com tamanho/MD5 esperados e calcular SHA-256. Sincronizar arquivo/diretório,
   reler staging e revalidar metadados remotos, raiz/pai e fingerprint local.
5. Persistir `Ready` antes do rename sem substituição. Após rename, fsync do
   diretório, verificar novamente identidade/hash e persistir `Installed`.
6. `Started` interrompido permite outro staging exclusivo, conservando o parcial
   anterior. O destino continua determinístico: repetição nunca duplica cópias
   finais. Falha SQLite interrompe o ciclo; falhas de rede/disco/integridade/
   colisão mantêm originais e a pendência com erro específico.

Uma revisão remota posterior ganha outro registro/nome; os anteriores continuam
recuperáveis. Reinício recupera snapshots `Ready` inclusive quando a revisão remota mudou ou
saiu do inventário, sem rede: a prova foi revalidada antes de `Ready`. Nunca
aplica versões obsoletas ao caminho original. Um estágio
antigo não confirmado nunca é anunciado como cópia íntegra. Alterações externas
posteriores em cópias são detectadas na revalidação, sem reparo destrutivo.

## Oráculos

P02: L/R divergentes preservados, nenhuma seleção/avanço de B, falha offline,
checksum/revisão/fingerprint obsoletos, cancelamento, reabertura SQLite e falha
antes/depois de publicação e confirmação. P08: nome ocupado por arquivo,
symlink ou diretório, criação concorrente, isolamento de escopos, repetição e
revisão seguinte sem colisão. Provedor simulado; arquivos e SQLite reais isolados.
Queda real de energia e integração Google/portal exigem validação separada.

O banco principal usa formato exato v3; v2 é rejeitado sem migração/reset.

Fundamentação primária consultada em 2026-10-30:
[rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html),
[fsync(2)](https://man7.org/linux/man-pages/man2/fsync.2.html),
[SQLite atomic commit](https://www.sqlite.org/atomiccommit.html).
NOREPLACE depende do filesystem; ausência de suporte falha de forma conservadora.
SQLite e filesystem não formam uma transação única; o replay verifica evidência.
