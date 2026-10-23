# Atualização de arquivos gerenciados — DOWNLOAD-003

Política autorizada: atualizar binários já instalados pelo DriveSync somente quando
ID remoto, raiz, pai, nome, tipo e identidade local correspondem à base confirmada,
o SHA-256 local permanece intacto e a versão remota cresce. Versões iguais são
NoOp apenas com metadados iguais; regressões ou divergência sem incremento são
UNKNOWN/conflito. Arquivos preexistentes não gerenciados não são adotados.

## Algoritmo e estados

1. Revalidar a base Installed/Updated por device+inode, tamanho antigo e SHA-256.
2. Persistir Updating antes de staging. Baixar a nova versão em temporário
   exclusivo; validar tamanho/MD5, sincronizar e verificar SHA-256 local.
3. Revalidar configuração, raiz/pai, cadeia remota e arquivo local antigo.
   Persistir UpdateReady contendo identidade/hash novos, base antiga e nome
   exclusivo `DriveSync backup <op>.bak`.
4. Sem cancelamento cooperativo dentro desta curta seção de commit: mover o
   destino antigo para a cópia com RENAME_NOREPLACE e sincronizar o diretório.
   Reabrir/verificar a cópia movida para detectar troca/edição concorrente.
5. Se a cópia não corresponde à base, tentar restaurá-la com NOREPLACE. Se
   alguém criou um destino no intervalo, preservá-lo e manter a cópia, informando
   conflito. Nunca sobrescrever esse destino concorrente.
6. Instalar o staging verificado com NOREPLACE e sincronizar o diretório. Em
   falha, tentar restaurar a cópia sem sobrescrever. Persistir Updated somente
   após a instalação. A cópia anterior permanece, sem limpeza automática.

Não usar rename comum ou EXCHANGE como substituição cega. Cada rename é atômico,
mas os dois renames não formam uma transação única: o caminho principal pode
ficar temporariamente ausente, com a versão anterior durável no backup. A intenção
persistida permite reconciliar essa janela sem inferir instalação por nome/hash
apenas. Backups são visíveis e seu caminho relativo aparece no resultado.

## Recuperação

UpdateReady + destino novo identificado/verificado + backup antigo verificado:
confirmar Updated. Destino ausente + backup intacto + staging novo intacto:
revalidar remoto/configuração e concluir. Destino antigo intacto + staging intacto
+ backup ausente: revalidar e retomar. Se o remoto mudou, restaurar primeiro o
backup quando necessário, marcar UpdateAbandoned e exigir novo ciclo. Um destino
inesperado ou backup divergente é conflito; manter todas as cópias. Updating sem
UpdateReady não autoriza mover o destino. Updated fornece a nova base para o
ciclo seguinte; o backup não é pré-condição para verificar essa nova base.

O journal mantém o mesmo schema e os mesmos formatos de registros existentes;
novos estados são variantes explícitas, não migradores. Versões antigas do motor
não entendem os novos estados e devem rejeitá-los sem reinicializar o banco.

## Oráculos e limites

Testar atualização/repetição/reinício, tamanho diferente e arquivo vazio; edição
local antes/durante o download e antes da movimentação; destino concorrente após
backup; symlink/troca de inode; versão remota regressiva ou alterada durante I/O;
falha de rede/checksum/disco/cancelamento; interrupção antes/depois de cada rename
ou confirmação SQLite; restauração sem sobrescrita; isolamento de cliente/conta.
Nenhuma falha autoriza exclusão de arquivo nem limpeza de backups.

Sem testes reais de queda de energia, Google ou portal nesta unidade. Processos
com os mesmos privilégios podem continuar escrevendo em descritores antigos;
esses bytes permanecem na cópia preservada. Não prometer isolamento contra
processo local hostil nem snapshot distribuído. Backups consomem espaço e não são
um histórico com política de retenção. Incremental, upload, remoções remotas,
renomeações e sincronização automática permanecem fora do escopo.

Referências oficiais consultadas em 2026-10-23:
- https://man7.org/linux/man-pages/man2/rename.2.html (NOREPLACE; descritores abertos)
- https://man7.org/linux/man-pages/man2/fsync.2.html (arquivo e diretório)
- https://www.sqlite.org/atomiccommit.html (intenção/commit locais)
- https://developers.google.com/workspace/drive/api/reference/rest/v3/files (version)
