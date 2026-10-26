# Preparação durável de downloads (STATE-001 / PLAN-001)

O journal de cada raiz local fica em `.drivesync-state.sqlite3` dentro da pasta
selecionada. `.drivesync/identity.json` registra a associação ao cliente OAuth, ao
subject estável da conta e à pasta remota. Ambos sobrevivem à remoção dos dados
privados do aplicativo e são excluídos do inventário de upload. Não armazenam
tokens. Checkpoints de changes e preferências de upload continuam no diretório
privado do aplicativo.

O journal por raiz usa schema 2, application_id próprio, foreign_keys ON,
synchronous FULL e journal DELETE. Transações IMMEDIATE publicam atomicamente uma
nova geração de inventário e plano da raiz. Itens não observados permanecem como
evidência antiga, sem gerar exclusões. O journal portátil preserva evidência
operacional de downloads entre reinstalações, mas não restaura autenticação nem
substitui a verificação de identidade e conteúdo feita pelo executor.
Chaves compostas contêm cliente OAuth + conta + raiz remota + raiz local; itens
referenciam essa identidade por chave estrangeira. Corrupção, schema estranho,
symlink e banco de outra aplicação são erros: nunca apagar/recriar para recuperar.
Falhas de inventário/plano não publicam uma geração; cancelamento observado
antes do commit provoca rollback.

Planejamento puro: recebe inventário, base confirmada opcional e observações
locais tipadas. Sem base, destino ausente é candidato a download; destino
preexistente é conflito. UNKNOWN adia. Com base verificada, local intacto e
versão remota igual resultam em NoOp; remoto alterado com local intacto resulta
em Download; divergência local bloqueia. Nunca produzir upload ou exclusão.
Arquivos existentes sem evidência de instalação confirmada continuam sendo
conflitos; a presença do marcador não autoriza sua adoção por si só.

Resolver caminhos por IDs/pais; bloquear nomes repetidos no mesmo pai e toda a
subárvore ambígua, nomes inseguros, ciclos, pais desconhecidos e componentes
simbólicos. A observação inicial lê somente metadados. Ela NÃO elimina TOCTOU;
o executor futuro terá de abrir/revalidar destinos por capacidade e pré-condições
atômicas antes de escrever. O plano persistido não é uma autorização de escrita.

Testes: isolamento de cliente/conta/raiz, reinício, atomicidade/rollback,
corrupção e versão incompatível preservadas, permissões, nomes iguais, pastas
aninhadas, symlinks, UNKNOWN e tabela B/L/R, cancelamento sem avanço do estado.

Referências oficiais consultadas em 2026-10-23:
- https://www.sqlite.org/atomiccommit.html
- https://www.sqlite.org/pragma.html#pragma_integrity_check
- https://docs.rs/rusqlite/0.37.0/rusqlite/struct.OpenFlags.html
