# Implementação incremental do Inventory

Branch de implementação: feat/inventory, a partir do merge do PR #96.

## Fundação

Abertura de inventory.sqlite3 na pasta reservada da raiz, identidade por conta,
sincronização, raiz remota e bytes da raiz local; esquema inicial versionado e
integridade verificada. Sem migração, conversores, exclusão ou recriação automática.
A raiz deve existir. Criar exige create_if_missing=true e ausência de arquivos
de usuário; .drivesync existente é permitido para coexistência com o Detector.

Abertura usa descritores sem seguir symlinks, verifica dono, permissões e hardlinks,
serializa a inicialização e mantém o diretório reservado aberto. SQLite usa
foreign_keys=ON, journal DELETE, synchronous EXTRA e busy timeout de 1 segundo.
Transações de escrita são IMMEDIATE; leituras compostas usam transação DEFERRED.
status lê snapshot separado e não aguarda uma transação SQL.

A marca de materialização inicial é persistida sob precondições de geração e
valor anterior. Essa marca não transfere arquivos nem confirma baseline.

Não existe ainda mecanismo comum de backup/restauração em Configuration. A base
atual falha explicitamente e preserva o banco, sem simular Restored/Rebuilt.
A implementação não abre bancos de contas reais nem se integra à GUI nesta etapa.

## Itens e metadados

A decisão do usuário mantém último valor conhecido e condição recente em campos
separados. Identidades, caminhos Unix, relações de pais, presença, status e
revisão são relacionais/indexados. metadata_fields tem uma linha por atributo,
com latest e known separados; não há SyncItem/LocalFile/RemoteFile inteiro em
um blob. O JSON de um atributo individual preserva u64, estados de coleta e
valores compostos sem inventar um tipo SQL incapaz de representá-los.

SyncItem.local/remote oferecem a visão conhecida; local_latest/remote_latest
expõem a coleta mais recente. NotApplicable é uma condição atual efetiva, não
apaga o valor conhecido histórico. Se a combinação de tipo/modo/alvo local de
coletas diferentes for incoerente, esse grupo usa a observação recente em vez
de devolver um LocalFile inválido; os valores conhecidos continuam no banco.
Referências lógicas ficam disponíveis mesmo quando não há metadados associados.

record_local/record_remote validam origem, conta, escopo, presença e revisão.
Uma ocorrência repetida com os mesmos dados/prova é idempotente antes do teste
de revisão; payload ou associação divergentes falham. Ausência confirmada exige
prova de varredura/listagem completa e identificada, fornecida pelo chamador;
Inventory não gera essa prova nem infere exclusão. Observações não avançam baseline.

Paginação usa chave crescente e limite de inserção da primeira página, com
cursor vinculado a identidade/filtro/geração; alterações de status continuam
sendo lidas como estado atual, sem prometer snapshot histórico entre chamadas.
link_item é explícito e transacional; trocar referência remota limpa somente a
projeção observada daquele lado para evitar atribuir metadados a outro recurso.

ProviderFileData aceito: perfil Google já selecionado, formato
google.drive.detector-file v1, allowlist e 64 KiB. Nenhum catálogo extra, ACL,
xattr, segredo ou dado bruto do Provider é acrescentado ao Inventory.

## Validação da fundação em 14/11/2026

Executados no org.gnome.Sdk//50 com rust-stable: fmt --all --check, clippy
--workspace --all-targets --offline -- -D warnings, test --workspace --offline,
doc --workspace --no-deps --document-private-items --offline e test --workspace
--doc --offline. Resultado integrado: 678 execuções aprovadas (incluindo 6
doctests), 8 testes de ambiente ignorados. Os testes novos cobrem abertura,
reabertura, identidade, geração/precondição da marca inicial, preservação de
formato incompatível, recusa de pasta ocupada, symlinks/hardlinks e status
consultável enquanto uma transação mantém a conexão ocupada.

Inicialização usa flock não bloqueante; as entradas novas são sincronizadas em
disco. A verificação final repetiu clippy e os três testes públicos de Inventory
após acrescentar os traits de identidade previstos no contrato. Não houve teste
com contas reais, GUI, pacote instalado ou mecanismo de backup/restauração.

## Baseline, operações e atividades

As 17 operações estão compiladas. Baselines têm uma linha por lado, pais tipados,
nomes locais em bytes e tamanho textual exato para u64. Apenas confirm_sync os
substitui. A confirmação requer estado conhecido nos dois lados e fatos
verificados pelo chamador; não coleta nem compara conteúdo físico.

Operations registra intenção e precondições em colunas relacionais. Claim usa
transação IMMEDIATE, revisão-base, versão remota recente e unicidade de Running
por item. Não há lease por tempo nem recuperação automática de Running. Requeue
é explícito após verificação do efeito pelo chamador. Tokens de tentativa evitam
atualização por worker antigo. A fila não executa efeitos, não decide ações,
não reescreve revisão-base e não transforma ausência de versão em autorização.

A confirmação grava baseline, estado/revisão, conclusão de operação, atividade
e recibo idempotente na mesma transação. O replay exato não duplica histórico.
Histórico é append-only, com FK para item/operação do mesmo item e consulta
cronológica por (occurred_at, id), com limite de inserção e cursor por filtro.
Não há política de expurgo destrutivo nem eliminação de tombstones.

As políticas exatas de confirmação, presença, deduplicação, retry/requeue e
limites de texto estão em INTERFACES.md. Os testes públicos usam SQLite real,
duas conexões concorrentes, reinício com Running, tentativa antiga, metadados
parciais, ausência comprovada, replay e falha SQL forçada no histórico para
verificar rollback integral. Não usam conta real nem executores físicos.

## Validação integrada final em 14/11/2026

Executados exclusivamente em org.gnome.Sdk//50 com a extensão rust-stable:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```

Todos passaram, sem warnings. `cargo test --workspace`: 693 testes aprovados
(incluindo 6 doctests), 8 testes de ambiente ignorados. A execução explícita de
`--doc` repetiu os 6 doctests, todos aprovados. O Inventory tem 18 testes públicos
e um teste interno de consulta de status durante transação.

A revisão final corrigiu o erro de conta que indevidamente desabilitava uma
instância saudável, a perda de presença ao repetir o mesmo vínculo remoto e a
reclassificação por tipo local desconhecido. Procurados stubs, código morto,
caminhos legados e dependências adicionais: nenhum remanescente identificado.
Não foram introduzidas dependências. Branch feat/inventory; commits locais,
sem publicação remota. Integração com Analisador/GUI, testes com contas reais,
benchmark de 80 mil itens e backup/restauração continuam fora desta entrega.

## Inspeção de tabelas para desenvolvimento

`inventory::inspection::inspect` é uma API de diagnóstico separada da fachada
transacional. Consulta somente as 11 tabelas permitidas, com conexão SQLite
read-only, query_only, identidade por conta/raízes e validação do esquema. Não
cria arquivos, não inicializa inventário e não disponibiliza SQL arbitrário.
O SyncId real é lido do inventário, sem supor o identificador de um detector.

Páginas têm até 100 registros por rowid e prévias limitadas a 4096 bytes por
célula. NULL, texto, número, BLOB UTF-8 e BLOB hexadecimal permanecem distintos;
truncamento e tamanho original são explícitos. Cada página é uma leitura
consistente, mas páginas seguintes consultam o estado atual. A inspeção não
substitui API de domínio nem restauração de banco incompatível.

Validação no SDK: fmt, clippy do workspace e 21 testes públicos de inventário,
incluindo três testes novos para consulta sem escrita (banco com permissão 0400),
ausência sem criação, identidade/esquema, paginação e prévias binárias/UTF-8.

## Developer Console — consulta conectada

F12 → Inventory agora usa a inspeção somente leitura. Contas vêm das configurações
concluídas, sem iniciar Detector; trocar conta/tabela descarta respostas antigas.
Há seletor das 11 tabelas, colunas redimensionáveis, paginação de 100 registros,
detalhe da linha, tipos SQL, prévias longas/binárias e estados de vazio, banco
inexistente e acesso negado. Consultas executam fora da thread GTK. Não existe
alimentação automática pelo Analisador ou criação do banco ao abrir essa página.

Validação final no org.gnome.Sdk//50 com rust-stable: fmt --all --check, clippy
--workspace --all-targets -- -D warnings, test --workspace, doc --workspace
--no-deps --document-private-items, test --workspace --doc e build --workspace.
Todos passaram. O workspace aprovou 697 testes (incluindo 6 doctests), com 9
testes de ambiente ignorados; os 6 doctests também passaram na execução separada.

O novo teste gráfico ignorado na execução padrão foi executado explicitamente
e passou em Mutter/Wayland isolado, com perfil XDG temporário e duas contas
sintéticas. Conferidas capturas de tabela, metadados com detalhe, banco ausente
e janela de 520 pixels. Foram validados Next/Previous, troca rápida de escopo,
truncamento, seleção e liberação da página. Não foram usadas contas reais.
A inspeção tem quatro testes públicos; os 22 testes públicos de Inventory
foram reexecutados após corrigir a distinção entre permissão e inexistência.
