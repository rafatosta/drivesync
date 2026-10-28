# Algoritmos — state

## Escopo e fontes de verdade

STATE-001 persiste observações e intenções; STATE-002 lê evidências e rejeita
estado inválido. O recorte implementado da SYNC-001 é o journal de downloads,
não uma recuperação geral de sincronização bidirecional. Requisitos na
[especificação](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização),
organização da raiz e [rastreabilidade](preparation.md) complementam este contrato.

## STATE-001 — gravação

Entradas: caminho absoluto do banco, cliente OAuth, conta, raízes remota/local,
inventário/plano consistentes e cancelamento; para o journal, `Operation` com
ID de operação, item remoto, caminho relativo, temporário, identidades da raiz/pai
e fase. Implementação: [mod.rs](mod.rs), [downloads.rs](downloads.rs) e
[modelo das fases](../download/models.rs).

1. `Store::open` cria estado novo com permissões privadas, schema 2 e
   application_id próprio. Banco existente deve passar versão/application_id,
   `quick_check`, `foreign_key_check` e modo de journal esperado. Symlink, banco
   existente vazio, corrompido, incompatível ou ilegível causam `StateStorage`;
   não apagar, migrar ou reinicializar automaticamente.
2. Ativar foreign keys, synchronous FULL e journal DELETE. Chaves de escopo:
   cliente + conta + raiz remota + raiz local. ID remoto é chave de item dentro
   desse escopo, nunca identidade global entre contas.
3. `save` publica inventário/plano em transação IMMEDIATE por geração.
   Observações antigas podem permanecer, mas não se tornam trabalho atual ou B.
   Cancelamento antes do commit e falha SQL preservam a geração anterior.
4. `put_operation` valida a estrutura serializada e persiste intenção/fase antes
   de o executor avançar para o efeito seguinte. Colisão de ID de operação com
   outro arquivo é erro, não sucesso silencioso. Uma gravação SQLite não engloba
   atomicamente rede e filesystem.

O retorno é confirmação de gravação local ou erro; salvar plano nunca significa
instalação. Tokens não pertencem a esses registros.

## STATE-002 — leitura da base e recuperação

`operations_for` consulta apenas o escopo solicitado, ordena por `rowid`, valida
IDs da linha contra o registro e rejeita JSON/fases inválidos. Há limites de
quantidade e volume serializado. `confirmed_bases` percorre as operações nessa
ordem e substitui o candidato de cada ID remoto a cada registro:

| Última operação do item | B retornada |
| --- | --- |
| `Installed` ou `Updated`, binário, ID não vazio e revisão positiva | Revisão remota + SHA-256/device/inode da instalação |
| `Started`, `Ready`, `Updating`, `UpdateReady`, `UpdateAbandoned` ou evidência sem revisão válida | Nenhuma |
| Registro estruturalmente inválido | Erro de armazenamento; não retornar mapa vazio como recuperação |

Uma operação posterior incompleta mascara a candidata anterior daquele item,
sem apagar o histórico nem inventar convergência. O marcador da raiz, inventário,
plano e arquivos existentes não reconstroem B. Banco ausente pode ser criado como
estado novo, mas os arquivos locais permanecem não gerenciados.

B carregada é evidência histórica. O planner precisa da fingerprint atual; o
executor precisa revalidar identidade/conteúdo e reconciliar operações pendentes.
A confirmação após efeito no filesystem pertence a
[DOWNLOAD-001/002](../download/execution.md) e
[DOWNLOAD-003](../download/updates.md). `Store` não instala nem restaura arquivos.
Se a confirmação falhar, a fase recuperável persiste; o ciclo seguinte verifica
as evidências antes de concluir. Ambiguidade mantém dados e bloqueia o efeito.

## Evidência e limites

[tests.rs](tests.rs) verifica reabertura/isolamento, rollback de geração,
corrupção/symlink, permissões, schema incompatível e retenção de observações.
A [matriz por regra](preparation.md#rastreabilidade-das-regras-do-eixo-1)
vincula perda/incompletude de B e recuperação aos testes de download com
filesystem/SQLite reais e provedor falso. Reabrir conexões não demonstra queda
real do processo, energia interrompida ou durabilidade em todos os filesystems.

Reconstrução geral após perda de estado, quarentena automatizada, critérios de
saída de modo degradado e recuperação bidirecional permanecem decisões futuras.
Não reconstruir B por inventário enquanto essas políticas não forem definidas.
WAL é referência técnica alternativa, não o modo deste journal; não há migradores
entre formatos internos. A bateria A01–A12 com Google Drive real permanece pendente.

Referências de projeto: [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html),
[SQLite pragmas](https://www.sqlite.org/pragma.html) e
[SQLite WAL](https://www.sqlite.org/wal.html). Essas referências não demonstram
validação de quedas de energia ou integração Google.
