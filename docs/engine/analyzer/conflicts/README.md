# Conflitos

## Objetivo

Avaliar **uma divergência entre duas representações de um mesmo item lógico** e determinar, a partir de evidências disponíveis, se existe conflito e qual encaminhamento é seguro.

O módulo é um algoritmo independente e fechado, utilizado internamente pelo [Analisador](../README.md). Cada chamada recebe os estados local e remoto, o baseline confirmado quando existir e a informação pertinente à situação; devolve **uma avaliação única e estruturada**. Não executa comandos nem aguarda interação humana.

## Funcionamento

1. Recebe o mesmo item lógico representado nos estados local e remoto, com as presenças observadas, um possível baseline e evidências associadas.
2. Valida se há informações suficientes para comparar identidade, conteúdo, nome, localização, presença e mudanças relevantes.
3. Identifica se as alterações são compatíveis, incompatíveis ou não comprováveis.
4. Retorna a classificação do conflito, as evidências, uma solução segura quando determinável ou as alternativas que exigem decisão externa.

**Conflito não é sinônimo de qualquer alteração nos dois lados.** Alterações simultâneas podem ser compatíveis (por exemplo, renomeação de um lado e edição de conteúdo do outro). Ausência de um snapshot não comprova exclusão. Igualdade de tamanho ou timestamp, isoladamente, não comprova igualdade de conteúdo.

## Resultados possíveis

| Situação | Resultado |
| --- | --- |
| Comparação segura sem incompatibilidade | `NoConflict`, com justificativa |
| Divergência resolvível por algoritmo comprovado | `Conflict` com `Automatic` e operações propostas |
| Divergência que exige escolha externa | `Conflict` com `RequiresDecision` e opções fundamentadas |
| Observações insuficientes ou inconclusivas | `Undetermined` com informações necessárias para nova avaliação |

Cada operação proposta é **apenas uma recomendação**. O [Analisador](../INTERFACES.md) decide como coordená-la e, quando autorizada e segura, executá-la pelo Operador. Um resultado `RequiresDecision` é convertido pelo Analisador no objeto público `PendingAction`.

## Contrato e propriedade dos dados

O módulo reutiliza `SyncItemId`, `SyncBaseline`, `PresenceState`, `LocalFile` e `RemoteFile` dos respectivos contratos proprietários. A entrada é **um par de estados referente ao mesmo item**, não um lote.

A interface de [INTERFACES.md](INTERFACES.md) define os objetos de entrada e saída, classificação, evidências, opções, erros e invariantes. Os algoritmos podem evoluir sem alterar a interface pública do Analisador.

## Invariantes

- Cada chamada produz avaliação de um item lógico, confrontando os dois lados; nunca agrupa itens distintos.
- A conclusão sempre indica a presença ou indeterminação de conflito, com evidências que sustentem o resultado.
- `Unknown`, `NotCollected`, `Unavailable` e ausência de baseline não equivalem a `AbsentConfirmed`.
- Nunca propor descarte de conteúdo sem decisão explícita e precondições compatíveis.
- Uma solução automática precisa ser demonstravelmente segura; diante de dúvida, solicitar decisão ou reobservação.
- Não escrever no Inventário, no filesystem, em serviços remotos ou em filas.
- Não incorporar política de escolha do usuário como se fosse observação factual.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** especificação de contrato para a primeira versão documental; não corresponde a código Rust implementado.
