# Agentes — Detector Local integrado aos modelos

## Escopo

Implementar **somente `detectors/local/`** conforme [README](README.md), [INTERFACES](INTERFACES.md), [TESTS](TESTS.md), os [contratos de Models](../../models/INTERFACES.md), o [padrão de módulos](../../MODULE_STANDARD.md) e [AGENTS raiz](../../../../AGENTS.md).

## Regras de implementação

1. **`LocalFile` pertence a Models.** Importar o tipo compartilhado; não criar outro `LocalFile`, `LocalItem`, `LocalFileKind` ou caminho genérico incompatível só para o Detector.
2. O Local é o **produtor de observações**: solicita `LocalFile` a um coletor especializado, sem construir seus campos, e o entrega com os dados realmente verificados. Não é o proprietário do modelo nem de sua persistência.
3. Emitir um `LocalNotice` por chamada ao `LocalNotificationSink`. Quando não houver arquivo observável, `local_file=None` com motivo e escopo; nunca interpretar isso como remoção confirmada.
4. Nenhum método do Local recebe, constrói ou depende de **`SyncItem` ou `SyncItemId`**. A introdução de `SyncItem` no núcleo será tratada somente durante revisão do Inventário.
5. `AccountId`, `SyncId`, `Timestamp` e identidades locais têm um **único proprietário efetivo**. Reutilizar/reexportar, não duplicar ou inventar aliases incompatíveis.
6. Não acessar InventoryManager, banco SQLite/Storage, Detector Remoto, Google API, Analisador, Orquestrador ou Operações para implementar o Local. Usar sink interno fake nos testes.
7. Sucesso no sink significa **aceite durável pelo pai**. Falha exige replay/revarredura, sem afirmar entrega ou limpar eventos.
8. `LocalRelativePath` deve preservar nomes não UTF-8 e impedir escapada da raiz. Não seguir symlinks externos ou tratar `.drivesync/` como conteúdo.
9. O Local não compara baseline, não decide conflitos, não cria upload/download e não declara ausência comprovada com watcher isolado.
10. Implementação própria e testes isolados; não refatorar módulos irmãos para facilitar testes.

## Regra de não regressão por alteração de LocalFile

**O Detector Local NÃO deve mudar quando forem incluídos/removidos campos de `LocalFile`.** Proibido construir `LocalFile` campo a campo no watcher/scanner/notificador ou usar acesso direto a campos para escolher se emite uma observação. Em vez disso:

1. Injetar/usar a porta estável `LocalMetadataCollector::collect` documentada em [INTERFACES.md](INTERFACES.md).
2. Delegar coleta, construção e rotinas adicionais ao componente especializado de metadados locais.
3. Tratar `LocalFile` como **valor opaco** no pipeline de detecção e emitir o objeto integral recebido.
4. Ao evoluir os campos, alterar **somente o contrato de dados, o coletor e os testes correspondentes**, além de eventuais consumidores reais — **não** watcher, scanner ou emissão.
5. Garantir que o coletor continue testável isoladamente com filesystem temporário e que `models/` não passe a executar I/O.

A revisão de utilidade dos metadados permanece obrigatória antes de implementar a proposta integral.

## Desenvolvimento Contract-First + TDD

Desenvolver em unidades RED → GREEN → REFACTOR: criação, ciclo de vida, observação/solicitação de coleta de `LocalFile`, entrega de `LocalNotice`, falhas e recuperação. Testar filesystem real temporário e sink fake; os testes de dados puros ficam em `models/TESTS.md`.

Se o contrato de `LocalFile`, caminhos ou IDs ainda estiver indefinido para algum cenário, relatar **BLOCKED** e registrar dependência, não criar modelo paralelo nem modificar o Inventário implicitamente.

Executar Rust/Cargo apenas no Flatpak do projeto; comentários e commits em inglês; não criar push/PR sem autorização. Relatar testes executados, resultados, evidências, commits e limitações.

**Estado:** instruções para código futuro; alteração atual restrita à documentação.
