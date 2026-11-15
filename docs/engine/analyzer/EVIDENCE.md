# Evidências reais para análise

O coordenador mede SHA-256 somente para o arquivo local regular observado.
Abre a raiz e cada componente por descritor sem seguir symlinks; recusa arquivos
especiais, metadados divergentes e mudança durante a leitura. Compara dispositivo,
inode, tamanho e tempos antes/depois. Não escreve conteúdo nem confirma baseline.
Falhas preservam `Unknown`; nunca usam tamanho/data como igualdade de conteúdo.

Checksums remotos precisam vir explicitamente de `sha256_checksum`. Checksums do
baseline precisam usar `sha256:<hexadecimal de 64 caracteres>`; checksum sem
algoritmo, MD5, versão remota ou campos ausentes não viram prova SHA-256.
As comparações são fornecidas ao Analyzer e persistidas em seu recibo. A medição
não associa itens nem altera o baseline. Sem hash remoto disponível, a comparação
correspondente continua pendente; não se implementa download para medir conteúdo.

Observações statx de arquivos regulares com único link e diretórios ganham uma
identidade local baseada em dispositivo, inode e nascimento. O Scanner procura
essa identidade antes de criar um item por caminho, preservando o vínculo após
renomeação/movimentação. Sem nascimento confiável ou em hardlinks, não inventa ID.
Identidade física diferente em caminho ocupado exige rechecagem, não associação
local↔remoto por nome; o estado pode continuar desconhecido. A execução futura
precisará revalidar recurso e precondições, inclusive após a medição atual.

A leitura ocorre somente quando há checksum SHA-256 remoto ou do baseline para
comparar. Novos arquivos sem referência não são lidos inutilmente. Cancelamento
da sessão é verificado entre blocos, sem publicar checksum parcial ou falso.
