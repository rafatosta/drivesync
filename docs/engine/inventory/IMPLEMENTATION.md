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

## Regra de metadados aguardando definição

O contrato metadata/INTERFACES.md deixa em aberto como manter o último valor
conhecido e a condição da coleta recente. A pergunta foi encaminhada ao usuário.
O esquema dos metadados e record_local/record_remote dependem dessa decisão;
nenhum codec ou atualização parcial deve ser escolhido silenciosamente.

Os tipos de requisição seguem o contrato documental. Métodos de itens, baseline,
operações e atividades serão acrescentados por unidades funcionais, com SQLite
real e transações, sem stubs nem implementação do Analisador/Operations.

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

API compilada nesta unidade: open, status, get_sync_info e
mark_initial_download_complete. Os tipos dos demais contratos foram declarados,
mas seus métodos ainda não existem e não são stubs. Esta fundação não representa
o Inventory completo nem integração com o Analisador.
