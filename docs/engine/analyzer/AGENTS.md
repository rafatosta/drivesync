# Orientações para agentes — Analisador

## Objetivo

Implementar a API pública do Analisador para processar **um `SyncItem` por chamada**, coordenando os algoritmos internos de reconciliação e conflitos, executando comandos pelo Operador e confirmando somente efeitos verificados no Inventário.

## Leitura obrigatória

1. [README.md](README.md): comportamento e invariantes.
2. [INTERFACES.md](INTERFACES.md): assinaturas, todos os tipos públicos, retornos, erros e semântica.
3. [TESTS.md](TESTS.md): matriz TDD, falhas e critérios de aceite.
4. [Inventário](../inventory/INTERFACES.md): `SyncItem`, baseline, revisão, fila, confirmação e atividades.
5. [Operador](../operations/INTERFACES.md): comandos, precondições, resultados e estados de verificação.
6. [AGENTS.md da raiz](../../../AGENTS.md): regras gerais, commits, segurança e ambiente Flatpak.

## Orientações de implementação

- Expor somente `new`, `analyze` e `status` como API pública desta primeira versão.
- Receber `AnalyzeRequest` com um único `SyncItem`; não introduzir entrada em lote nem esperar decisões do usuário dentro da chamada.
- Representar explicitamente `Completed`, `Pending` e `Failed`; distinguir erros prévios à execução de falhas com efeitos parciais.
- Reconciliador e Conflitos são componentes internos independentes; seus contratos serão definidos em etapa separada. Preparar pontos de integração testáveis sem presumir suas implementações.
- O Analisador **é o único coordenador da execução**: utiliza os comandos tipados e métodos do Operador; nenhum algoritmo interno ou Orquestrador executa comandos diretamente.
- Reutilizar tipos existentes do Inventário e Operador; não duplicar `SyncItem`, `SyncBaseline`, `OperationCommand` nem metadados.
- Verificar escopo, identidade, revisão e evidência suficiente, especialmente antes de exclusão, substituição e restauração.
- Executar comandos na ordem e com as dependências identificadas; preservar o resultado de cada operação e seu estado de verificação.
- Usar exclusivamente os métodos públicos do `InventoryManager` para registrar fila, atividades e confirmar baseline; jamais atualizar SQLite diretamente.
- Uma decisão externa contém identidade de pendência, opção e evidências; revalidar tudo na retomada e rejeitar escolhas obsoletas.
- Distinguir falha técnica, conflito, evidência insuficiente e resultado de operação indeterminado; nunca converter incerteza em sucesso.
- Preservar dados do usuário e impedir reexecução cega após perda de resposta ou erro de confirmação.
- Conservar o escopo da tarefa e documentar com rustdoc as garantias não triviais quando houver código.

## Observabilidade durante execução

- `status(item_id)` consulta apenas a **análise ativa daquele item**, rapidamente e sem I/O; nenhum histórico de progresso ou fila privada de trabalho deve ser criado.
- Atualizar fases reais: validação, reconciliação, avaliação de conflito, execução, verificação e confirmação; não inventar duração ou porcentagem.
- Quando o Operador disponibilizar `ProgressReporter`, transportar as informações efetivamente reportadas para a consulta de status. Para comandos sem progresso, fornecer somente fase e contagens confiáveis.
- Encerrar o registro transitório de atividade ao retornar `Completed`, `Pending`, `Failed` ou `Err`; a persistência de resultado/histórico é do Inventário.
- Concorrência: permitir itens distintos, mas rejeitar a segunda análise simultânea do mesmo `SyncItemId` com `AlreadyRunning`. Continuar validando revisões/precondições, pois a proteção local não impede outras instâncias.
- Não criar `pause`, `resume`, `stop`, notificação GTK ou ligação direta com o Orquestrador.
- Cobrir os testes AN-P01 a AN-P12 descritos em `TESTS.md`.

## Estratégia de validação

Começar pelos testes da API pública em [TESTS.md](TESTS.md) com dependências falsas. Complementar com Inventário real em SQLite temporário, Operator falso registrador e falhas injetadas para verificar efeitos parciais, decisões pendentes, concorrência, revisão e recuperação.

Se os contratos externos ainda não disponibilizarem persistência de `PendingAction` ou plano de comandos completo, registrar a lacuna como **pendência de integração**; não criar APIs de outros módulos por iniciativa própria e não declarar recuperação durável implementada.

Executar verificações Rust somente dentro do ambiente Flatpak especificado no repositório. Registrar os testes executados, os não executados e qualquer impedimento, sem afirmar validação funcional a partir apenas da documentação.

## Entrega e escopo

Modificar apenas arquivos autorizados da raiz `docs/engine/analyzer/` nesta fase. A documentação de `reconciliation/` e `conflicts/` será tratada separadamente. Não implementar código Rust nem alterar outros módulos como efeito colateral desta tarefa.

**Estado:** especificação para implementação futura, sujeita a revisão após documentação dos algoritmos internos.
