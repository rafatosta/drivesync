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

1. `Store::open` cria estado novo com permissões privadas, schema 3 e
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

A reconstrução local explícita é definida em STATE-003 abaixo. Salvamento parcial
de linhas, limpeza automática de evidências e recuperação bidirecional geral
permanecem fora desse protocolo.
Não reconstruir B por inventário enquanto essas políticas não forem definidas.
WAL é referência técnica alternativa, não o modo deste journal; não há migradores
entre formatos internos. A bateria A01–A12 com Google Drive real permanece pendente.

Referências de projeto: [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html),
[SQLite pragmas](https://www.sqlite.org/pragma.html) e
[SQLite WAL](https://www.sqlite.org/wal.html). Essas referências não demonstram
validação de quedas de energia ou integração Google.

## STATE-003 — reconstrução explícita (SYNC-005)

Entrada: associação verificada, bloqueios lógico e físico retidos pelo chamador,
confirmação explícita do usuário e cancelamento. Saída: quantidade de bancos
substituídos e indicação de uploads bloqueados; não é confirmação de sincronização.
Implementação: [recovery.rs](recovery.rs), exposta por `recover_state` na fachada.

1. Verificar marcador e ler `.drivesync/recovery.json`, se existente. O seletor
   aceita somente formato vigente e nomes de bancos conhecidos sem componentes de
   caminho. Seletor inválido bloqueia leituras normais. Na recuperação explícita,
   preservar seus bytes e tratar todas as gerações como desconhecidas, criando
   substitutos vazios sem reutilizar bases antigas. Symlink, objeto não regular
   ou arquivo ilegível bloqueiam recuperação; nunca escolher banco por enumeração.
2. Para cada banco, copiar banco e sidecars presentes (`-journal`, `-wal`, `-shm`)
   para diretório privado de inspeção. Validar SQLite e registros serializados do
   escopo nas cópias. SQLite pode consumir sidecars durante abertura; por isso a
   inspeção nunca abre os originais. Cópias de inspeção são descartáveis, sem valor
   de baseline; sob interrupção podem permanecer e não são selecionadas.
3. Manter o caminho de cada banco válido. Para banco ausente, inválido ou de
   formato incompatível, criar um banco vigente vazio com nome exclusivo
   `recovered-<id>-<banco>`. Não copiar linhas de um banco parcialmente inválido
   para afirmar integridade. O original e todos os seus sidecars permanecem no
   lugar, sem política automática de expiração ou remoção.
4. Sincronizar candidatos e diretório; escrever seletor temporário privado,
   sincronizá-lo, criar e sincronizar `recovery-required` e publicar por rename
   atômico, seguido de fsync do diretório. O marcador impede voltar aos originais
   obsoletos se o seletor desaparecer; se houver interrupção antes do primeiro
   rename, a reconstrução explícita reinicia com todo o estado desconhecido.
   Antes do marcador, cancelamento/falha deixa a seleção anterior. Entre o marcador
   e o primeiro rename, leituras ficam bloqueadas até reconstrução explícita. Após rename,
   falha de fsync ainda é erro: repetir valida a seleção visível. Candidatos não
   publicados permanecem preservados e nunca são descobertos por enumeração.
5. Uma seleção publicada cujo banco desapareceu causa erro nas leituras normais;
   somente nova recuperação explícita pode substituí-lo. Repetição sem nova
   corrupção não publica outra seleção. Nenhuma conexão de escrita cooperante
   pode atravessar a troca: ciclos, recuperação, configuração de associação e
   alteração de consentimento de upload usam o lock de associação/raiz.
   Leituras públicas de planos, reconciliações e consentimento, além do logout,
   também retêm o lock de associação: abrir SQLite pode recuperar um hot journal
   e não equivale necessariamente a uma leitura sem efeitos no banco.
6. Perda de journal de instalações deixa B desconhecida. Inventários e planos
   íntegros continuam observações, nunca autorização para adotar preexistentes.
   A próxima sincronização refaz observações remotas quando necessário e aplica
   as regras conservadoras existentes; offline a reconstrução local pode concluir,
   mas não declara acesso remoto nem convergência.
7. Perda/invalidade do banco de uploads bloqueia uploads duravelmente, inclusive
   nova tentativa de habilitá-los. Não repetir creates de resultado incerto. O
   download pode continuar, com pendência explícita no relatório combinado.
   A resolução dos intents remotos perdidos não é inferida nem automatizada; exige
   reconciliação adicional das evidências preservadas. O botão de recuperação não
   remove esse bloqueio. Não há migração de formatos internos.

O cursor confirma aquisição durável de inventário/notices reaproveitáveis, não
instalação. Pode avançar antes da transferência somente porque o inventário
persistido continua sendo reprocessado mesmo sem novo evento. B só avança depois
de identidade, integridade, efeito e journal confirmados. Erro de checkpoint não
pode ser tratado como inventário vazio; reconstruí-lo exige coleta completa.

Base técnica: [atomic commit](https://www.sqlite.org/atomiccommit.html) e
[risco de alterar bancos/journals em uso](https://www.sqlite.org/howtocorrupt.html),
consultados em 2026-10-30. A publicação por seletor e a preservação conservadora
são políticas do DriveSync, não uma transação distribuída SQLite/filesystem/rede.
Os locks são advisory; ferramentas externas que os ignoram não são coordenadas.
