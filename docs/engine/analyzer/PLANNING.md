# Analyzer — fechamento de decisões sem execução

`analyze` valida o snapshot, chama Reconciler e Conflicts independentes e passa
pela fase `Planning` antes de gravar o recibo. A etapa consulta somente APIs
públicas do Inventory e usa tipos de `operations`; não possui nem chama Operator.

## Planos e contexto

Cada intenção ganha `PlannedStep`: método, `OperationCommand` com origem/alvo,
destino, nome e precondições quando conhecidos, bloqueios e dependências por ID.
Criação de arquivo mapeia para upload/download; diretório para create_directory;
conteúdo, nome e localização para update_content, rename e move.

Pais são resolvidos pelo vínculo de identidade existente ou pela raiz configurada.
Não se procura uma pasta remota pelo nome da pasta local, nem vice-versa. Sem
réplica do pai, o passo depende de seu SyncItemId e fica bloqueado; não fabrica ID
do recurso que ainda precisará ser criado. A raiz remota configurada é vínculo de
escopo, não uma pasta adicional a baixar.

Colisões locais são verificadas contra caminhos efetivamente ocupados e contra
réplicas remotas independentes que competiriam pelo mesmo caminho. Recursos
homônimos nunca são associados. Duplicatas de nome permitidas no Drive não são
tratadas como identidade nem automaticamente como colisão remota.

`ItemGuard` e guardas do conjunto de filhos do destino são validadas na gravação e
nas consultas. Nova observação que afete origem, pai ou destino torna o plano
obsoleto. O consumidor da sessão reanalisa o Inventory por lote, inclusive planos
de outros itens afetados. Revisões apenas diagnósticas não invalidam as guardas.

Uma segunda mutação sobre o mesmo item depende do passo anterior e fica bloqueada
por `PriorStep`: após seu resultado verificado será necessário reanalisar e renovar
referências/versões. Não executar dois comandos reutilizando cegamente a mesma
precondição. Isso vale mesmo quando os dois comandos já estão descritos no plano.

## Lixeira e restauração

Ausência comprovada de um lado contra sobrevivente comprovadamente inalterado
gera intenção `Trash`, conservando `AuthorizeDeletion` e bloqueio
`UserAuthorization`. Exclusão versus alteração continua conflito. Não há política
implícita de autorização nem variante Delete em OperationCommand.

`AnalyzeRequest.directive: Option<RestoreSelection>` permite um pedido explícito
com ID da decisão e alvo. Alvo estranho ao item ou escolha vazia é recusado;
snapshot antigo segue as regras de StaleItem. Para remoto comprovadamente em
lixeira, monta Restore pelo mesmo ID; para recuperação não comprovada mantém
pendência de capacidade/destino, sem inventar localização de lixeira local.
A escolha é persistida e participa do retry; não restaura arquivo nem confirma
baseline. Não implementa escolhas de vencedor ou resolução de conflitos.

## Evidência e capacidade

[EVIDENCE.md](EVIDENCE.md) descreve SHA-256 observado e identidade local. Nome e
hash nunca associam itens. A associação de preexistentes é explícita e registrada
pelo [Inventory](../inventory/PLANNING.md), sem confirmar baseline.

Versão de destino desconhecida exige `VerifyPreconditions`. Provedor sem suporte
conhecido e documentos nativos que exijam exportação/conversão permanecem bloqueados
por `VerifyCapabilities`, sem conversão implícita. Toda execução futura deve
revalidar as guardas, capacidade, autorização e o recurso físico.

Conflitos e colisões têm recursos envolvidos e motivo em `AnalysisPlan.manual`;
falta de prova continua separada de conflito. Todas as operações persistidas estão
Blocked, inclusive planos sem pendências de dados: o Operator não tem implementação.
