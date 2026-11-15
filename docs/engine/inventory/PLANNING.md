# Inventory — vínculos explícitos e planos duráveis

O esquema é **5**. Bancos anteriores são preservados e recusados com
`UnsupportedSchema`, sem migração, limpeza ou reconstrução automática.

`associate_items(AssociateItemsRequest)` associa dois itens presentes sem vínculo
oposto nem baseline. Recebe snapshots completos e a identificação da escolha
explícita do usuário. O item local permanece canônico; o remoto anterior é
retirado da listagem atual, mas seu ID/histórico é preservado. Referências e recibos
remotos são vinculados ao canônico na mesma transação. Não compara nome/hash,
não altera arquivos e não confirma baseline. Escolhas antigas ou incompatíveis
falham; retry idêntico recupera a proveniência. `get_association` consulta o registro.
O console oferece seleção explícita dos dois IDs; ainda não existe confirmação
de vínculo por Operator executado. A raiz remota configurada não pode ser
associada a um filho local.

`NewAnalysisAssessment.plan` e `RecordedAnalysis.plan` guardam o plano opcional.
O Analyzer preencherá o plano; chamadas legadas de análise isolada usam `None`.
A gravação ocorre na mesma transação do recibo e das operações `Blocked`.

Cada passo tem método, comando tipado opcional de `operations::OperationCommand`,
bloqueios e dependências por identidade lógica. O comando não carrega callbacks,
credenciais ou opções de execução e não representa autorização para chamar Operator.
Não há variante de exclusão permanente. `DirectoryRef::LocalRoot` representa a raiz
configurada sem fabricar um caminho relativo vazio.

Passos, bloqueios, dependências, requisitos manuais, recursos e guardas são linhas
relacionais. O comando e referências individuais são serializações de enums
tipados, não um blob genérico da análise nem código executável. Todas as tabelas
novas aparecem na inspeção somente leitura do Inventory.

`ItemGuard` protege o estado observado e o baseline, sem invalidar plano por mera
mudança do diagnóstico/revisão durante outra análise. `get_namespace` fornece
uma guarda do conjunto de filhos de um destino por identidade: inserções novas
invalidam o plano mesmo sem alterar o item de origem. Guardas são revalidadas na
transação; consultas de histórico e filtro atual calculam obsolescência novamente.
Nenhuma guarda elimina a necessidade de revalidação antes da execução futura.
