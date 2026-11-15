# Analyzer — mapa de ações antes do Operator

Este documento separa decisões já implementadas, limitações do fluxo real e
contratos ainda a definir. Não implementa comandos, resolução de conflitos ou
novas interfaces. O Operator permanece fora desta etapa.

## 1. Identidade: regra obrigatória

**Nomes nunca associam arquivos locais e remotos.** Mesmo nome, caminho relativo,
tamanho, datas ou conteúdo idêntico não estabelecem identidade lógica entre dois
itens independentes. Hash comprova uma comparação de conteúdo; não autoriza fundir
itens, associá-los ou confirmar que foram sincronizados.

Vínculos válidos têm origem explícita:

- Replicação pelo futuro Operator: a operação conhece o item de origem e registra
  a identidade do recurso efetivamente criado no destino após verificação.
- Associação de itens preexistentes definida explicitamente pelo usuário: deve
  identificar os dois recursos por identidade e escopo, validar suas revisões e
  registrar a escolha. Esta ação ainda não foi implementada.

Arquivos independentes com o mesmo nome permanecem independentes, inclusive
quando têm o mesmo conteúdo. Diretórios homônimos não são mesclados automaticamente.
Comparar nomes e conteúdo **dentro de um vínculo já estabelecido** é permitido
para detectar mudanças; essa comparação não cria o vínculo.

Uma associação definida pelo usuário não autoriza sobrescrita/exclusão nem
confirma baseline automaticamente. A definição e persistência de sua proveniência
fazem parte do contrato futuro; o modelo atual não registra uma escolha de
associação feita pela UI.

Exclusão permanente é proibida, inclusive esvaziamento de lixeira e remoção de
revisões pela API. Toda exclusão deve ir para a lixeira. A falta de suporte à
lixeira exige bloqueio, nunca fallback para exclusão permanente.

## 2. Ação, direção e disposição são coisas diferentes

- **Ação:** o que precisa mudar, por exemplo criar, atualizar conteúdo ou renomear.
- **Direção:** local→remoto (`Upload`) ou remoto→local (`Download`). Não significa
  que toda mudança transfira conteúdo: rename/move preservam o recurso existente.
- **Disposição:** se há intenção, evidência faltante, conflito ou nenhuma mudança.
- **Execução:** indisponível. Todas as intenções persistidas permanecem `Blocked`.

Uma análise pode identificar várias ações do mesmo item e requisitos simultâneos.
Conflito tem prioridade na apresentação, sem apagar os demais fatos. Uma intenção
com direção identificada pode continuar pendente de identidade, destino ou evidência.

## 3. Catálogo de transformações

| Ação | Local→remoto | Remoto→local | Condição para decidir | Estado atual |
| --- | --- | --- | --- | --- |
| Criar arquivo | Criar réplica no Drive; futuro upload | Criar réplica local; futuro download | Origem suportada e observada; item novo sem baseline ou histórico compatível de ausência | Intenção `Create` implementada nos dois sentidos; destino/identidade ainda podem bloquear |
| Criar pasta | Criar diretório remoto | Criar diretório local | Mesmas condições de criação; sem transferência de conteúdo de diretório | Implementada como `Create` com `ItemKind::Directory` |
| Atualizar conteúdo | Atualizar recurso remoto vinculado | Atualizar arquivo local vinculado | Vínculo existente, baseline confirmado e mudança unilateral comprovada; conteúdo atual não convergiu | Núcleo e persistência implementados; fluxo real ainda não fornece comparações de conteúdo |
| Renomear | Renomear o recurso remoto vinculado | Renomear o recurso local vinculado | Mudança de nome unilateral em relação ao baseline, preservando identidade | Núcleo e persistência implementados; dependem do vínculo correto |
| Mover | Alterar pasta do recurso remoto vinculado | Alterar pasta do recurso local vinculado | Mudança unilateral de pai lógico, com vínculo e evidências de localização | Núcleo e persistência implementados; mapeamento dos pais pode continuar pendente |
| Enviar à lixeira | Destino remoto | Destino local, se suportado | Identidade, ausência/exclusão comprovada, política e autorização específicas | Vocabulário geral do Inventory/Operator; Analyzer não gera esta intenção |
| Excluir definitivamente | Proibido | Proibido | Nenhuma autorização deve liberar exclusão permanente; toda exclusão deve ir para a lixeira | Analyzer não deve gerar esta intenção; métodos destrutivos da API permanecem chamáveis, mas bloqueados sem execução |
| Restaurar | Restaurar recurso remoto | Restaurar recurso local, se suportado | Recurso recuperável identificado e política/escolha explícita | Analyzer não gera esta intenção |

Na primeira observação sem vínculo oposto, `Create` indica **replicação desejada**,
não prova que o destino está livre. `Unknown` permanece desconhecido, com
`ObserveLocal`/`ObserveRemote`, `VerifyIdentity` e `ResolveDestination`.
Encontrar um item homônimo no destino não autoriza reutilizá-lo nem sobrescrevê-lo.

Permissões, propriedade e atributos específicos do provedor não são ações de
propagação implementadas. Documentos nativos que exijam exportação/conversão e
capacidades de escrita/lixeira também precisam de contrato específico; tratá-los
como arquivo observado não comprova que uma transferência seja executável.
Esses limites devem gerar diagnóstico e bloqueio apropriados, sem converter
silenciosamente formatos ou alterar metadados. O vocabulário atual de requisitos
não representa todas essas capacidades; sua eventual ampliação será uma etapa
explícita, sem mudanças de interface nesta documentação.

Ações independentes podem coexistir: renomear e atualizar conteúdo, mover e
renomear, ou mudanças compatíveis em sentidos distintos. A ordem atual das
avaliações é presença, conteúdo, nome e localização. Isso não constitui ordem de
execução: dependências e precondições do futuro plano ainda precisam ser definidas.

## 4. Intervenção manual

| Situação | Resultado atual | Limite / ação futura |
| --- | --- | --- |
| Conteúdos alterados nos dois lados, comprovadamente diferentes | Conflito `ConcurrentChanges` quando as comparações são fornecidas | Identificar as versões; escolha e execução posteriores |
| Nomes alterados nos dois lados para valores distintos, no mesmo item vinculado | Conflito `ConcurrentChanges` | Não escolher nome vencedor |
| Pais alterados nos dois lados para destinos distintos, com prova do mapeamento | Conflito `ConcurrentChanges` | Não escolher pasta vencedora |
| Exclusão comprovada contra alteração do sobrevivente | Conflito `DeleteVsChange` | Não recriar, excluir ou descartar alteração automaticamente |
| Destino efetivamente ocupado por outro item independente | Detecção transversal ainda não implementada | Bloquear a criação/movimentação; encaminhar colisão para decisão do usuário, sem associar os itens |
| Usuário deseja associar dois itens preexistentes | Fluxo de associação explícita ainda não implementado | Selecionar as identidades, validar escopo/revisões e registrar a escolha; sem baseline presumido |

Nome igual sozinho não é conflito de sincronização. Colisão exige comprovar que
uma ação tenta ocupar um destino incompatível; nomes repetidos permitidos pelo
provedor não devem ser tratados automaticamente como colisão local.

Opções como escolher uma versão, manter ambas ou definir outro destino ainda
precisam de contrato. São possibilidades de resolução, não funcionalidades
implementadas nem políticas de sobrescrita aprovadas.

## 5. Requisitos: obter dados antes de decidir/executar

Estes resultados não são operações físicas e não equivalem automaticamente a ação
manual. Cada requisito já existe em `AnalysisRequirement`; seu atendimento
completo no fluxo real deve ser implementado antes de liberar a ação dependente.

| Requisito | O que falta |
| --- | --- |
| `ObserveLocal` | Observação local confiável e suficiente para a conclusão pretendida |
| `ObserveRemote` | Observação remota confiável; scan local completo não prova ausência remota |
| `VerifyIdentity` | Confirmar vínculo por origem explícita ou decisão do usuário; nunca pelo nome/hash |
| `VerifyBaseline` | Histórico confirmado suficiente, sem inventar sincronização anterior |
| `VerifyContent` | Comparações confiáveis entre os lados e com o baseline, ligadas às revisões |
| `VerifyName` | Nome observado e histórico necessários para decidir uma renomeação |
| `VerifyParent` | Identidades/mapeamento dos pais para avaliar localização |
| `VerifySurvivingState` | Reobservar o lado sobrevivente antes de concluir sobre exclusão |
| `AuthorizeDeletion` | Política/autorização de exclusão; não é uma decisão de excluir |
| `ResolveDestination` | Destino e dependências identificados, incluindo pai ainda não criado e colisões |
| `VerifyKind` | Tipo suportado; links, tipos especiais e capacidades/formatos sem tratamento não autorizam operação |

O coordenador atual mantém as três comparações de conteúdo como `Unknown`.
Implementar medição de conteúdo não autorizará associação automática entre itens.
A evidência de ausência precisa distinguir remoção de perda de acesso, falha de
coleta e cobertura parcial.

## 6. Resultados sem transformação e falhas

| Resultado | Interpretação |
| --- | --- |
| `NoChanges` | Nenhuma ação/requisito identificado nesta avaliação; não confirma baseline nem marca `Synced` |
| `PendingOperations` | Intenções persistidas; comandos/execução ainda indisponíveis |
| `PendingEvidence` | Evidência, autorização ou destino pendente; pode haver direção já identificada |
| `PendingUserDecision` | Conflito comprovado; nenhuma escolha automática |
| Análise obsoleta | Observação/revisão posterior invalidou a atualidade do recibo; reanalisar |
| Falha técnica ou entrada inválida | Erro de validação/persistência/escopo; não converter em conflito ou sucesso |

Repetição idêntica reutiliza o recibo e não duplica a fila. Nova evidência exige
reanálise da revisão atual. Falhas e requisitos não autorizam confirmar baseline.
Ambos os lados comprovadamente ausentes podem não exigir efeito físico; isso não
apaga automaticamente o histórico do item.

## 7. Matriz de validação antes do Operator

Os cenários abaixo são o checklist de fechamento do mapa. Há cobertura parcial
nas suítes atuais; esta lista não declara funcionalidades futuras implementadas.

| Grupo | Cenários obrigatórios | Situação |
| --- | --- | --- |
| Identidade | Mesmo nome/conteúdo sem vínculo permanece como itens distintos; homônimos em contas/pastas diferentes; associação explícita não implica baseline | Regra obrigatória; fluxo da escolha e cobertura transversal ainda pendentes |
| Criação | Arquivo e pasta, nos dois sentidos; origem parcial; lado oposto desconhecido; destino ocupado | Intenções básicas cobertas; colisões/dependências pendentes |
| Conteúdo | Mudança unilateral em ambos os sentidos; convergência; mudanças concorrentes distintas; arquivo mudando durante medição | Algoritmos com evidência sintética cobertos; aquisição real pendente |
| Nome/localização | Rename, move, combinações compatíveis e conflito; pai ausente; identidade preservada após mudança | Núcleo coberto; identidade local e dependências precisam de validação integrada completa |
| Exclusão | Ausência comprovada, perda de acesso, scan parcial, exclusão×edição, descendentes | Proteções/requisitos existentes; política de envio à lixeira pendente; exclusão permanente proibida |
| Persistência | Retry, reentrega, reinício, obsolescência, isolamento, falha transacional | Suítes atuais cobrem análise e ingestão; efeitos do Operator ainda inexistentes |
| Manual | Identificação, escolhas com revisões antigas, revalidação e preservação das duas versões | Identificação implementada; escolhas e resolução pendentes |
| Interface | Arquivo, múltiplas ações, direção, requisitos, detalhes e análise obsoleta | Painel implementado; ações do usuário ainda não implementadas |

## 8. Ordem proposta para fechar o Analyzer

1. Validar este catálogo e fechar as políticas pendentes de colisão, exclusão e
   resolução, sem ampliar silenciosamente as ações autorizadas.
2. Definir o vínculo explícito e sua proveniência; cobrir homônimos sem associação
   automática e a distinção entre identidade, conteúdo e baseline.
3. Fornecer evidências reais de conteúdo/presença e resolver destinos/dependências.
4. Completar as decisões e a matriz de testes, mantendo tudo sem efeitos físicos.
5. Só então definir os comandos do Operator: identidades, recursos de origem e
   destino, revisões, capacidades, dependências, autorização e resultado verificável.

O futuro Operator executará comandos autorizados; não decidirá identidade,
resolução de conflito ou qual versão deve prevalecer por conta própria.
