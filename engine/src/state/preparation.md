# Preparação durável de downloads (STATE-001 / PLAN-001)

Escopo desta unidade: persistir observações remotas e um plano inicial, sem
transferir bytes, registrar instalação confirmada, avançar cursores ou apagar
arquivos. Um inventário anterior não é uma base de sincronização confirmada.

SQLite privado em XDG_DATA_HOME/drivesync/state.sqlite3. Schema 1, application_id
próprio, foreign_keys ON, synchronous FULL e journal DELETE. Transações IMMEDIATE
publicam atomicamente uma nova geração de inventário e plano da raiz. Itens
não observados permanecem como evidência antiga, sem gerar exclusões.
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
Nesta unidade ainda não há gravador de bases confirmadas nem leitura segura de
hashes locais: a integração inicial é conservadora e não adota arquivos existentes.

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
