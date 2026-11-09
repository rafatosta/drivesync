# Analisador — TESTS

## Objetivo e abordagem

Validar a interface pública `new` e `analyze` com o **Analisador real** e dependências simuladas/injetadas. O processamento é por `SyncItem`, com invariantes de identidade, estados, execução, confirmação e recuperação.

Fonte de verdade: [README.md](README.md) e [INTERFACES.md](INTERFACES.md). Os casos são especificações **TDD**, não testes já executados.

## Ambiente e fixtures

- Inventário SQLite **real e temporário** quando testar persistência/confirmação; interface do Inventário simulada apenas nos testes unitários de coordenação e injeção de falha.
- Operador falso registrador de chamadas, capaz de devolver resultados verificados, não verificados, erros, efeitos indeterminados e cancelamento. Não executar rede real.
- `SyncItem` com baseline confirmado, versões e presenças locais/remotas distintas, IDs de conta e revisão conhecidos; produzir expectativas sem reutilizar o algoritmo sob teste.
- Pendência identificada por `PendingId`, opções válidas e fingerprint conhecido; versões mutáveis para simular decisão obsoleta.
- Uma sequência de duas operações para o mesmo item, com falha injetável em cada etapa e na confirmação.

## Índice de casos por método público

| Caso | Método | Sucesso | Falhas e invariantes |
| --- | --- | --- | --- |
| AN-01 | `new` | Instância com inventário e operador compatíveis | Dependência ausente, falha de inicialização ou contexto incompatível; nenhuma execução |
| AN-02 | `analyze` | Item consistente → `Completed(Unchanged)` e zero chamadas ao Operador | Escopo divergente, item inválido, revisão obsoleta |
| AN-03 | `analyze` | Alteração local exclusiva → operação remota adequada, execução e confirmação | Precondição divergente ou comando incompatível bloqueia operação |
| AN-04 | `analyze` | Alteração remota exclusiva → operação local adequada, execução e confirmação | Sem dados suficientes → `Pending`, não exclusão |
| AN-05 | `analyze` | Duas transformações para um item executadas na ordem prevista | Falha na segunda preserva relatório verificável da primeira |
| AN-06 | `analyze` | Conflito resolvível automaticamente executa plano autorizado | Conflito ambíguo → `Pending` com evidências e opções |
| AN-07 | `analyze` | Resolvido após nova chamada com `Resolution` válida | Pendência/versão/opção obsoletas são rejeitadas |
| AN-08 | `analyze` | Confirma baseline somente após verificação | `Unverified` não promove baseline nem marca `Completed` |
| AN-09 | `analyze` | Falha do Operador gera `Failed` com etapa e comando | Efeito remoto desconhecido → `ReobserveBeforeRetry`, nunca replay automático |
| AN-10 | `analyze` | Falha na persistência conserva informações do efeito aplicado | Retry não repete automaticamente operação já aplicada |
| AN-11 | `analyze` | Itens independentes podem avançar sem aguardar decisão externa | Mesmo item em concorrência não executa duas alterações incompatíveis |
| AN-12 | `analyze` | `SyncItem` com baseline `None` e presença comprovada é avaliado sem inventar histórico | `Unknown` e observação incompleta não se tornam `AbsentConfirmed` |

## Testes de tipos e contratos de retorno

| Caso | Verificação |
| --- | --- |
| AN-T01 | `Completed(Unchanged)` inclui ID, revisão final válida e `operations=[]` |
| AN-T02 | `Completed(OperationsApplied)` inclui comandos executados, resultados tipados e IDs persistentes quando presentes |
| AN-T03 | `PendingAction` inclui identidade, revisão, fingerprint, motivo, contexto e opções não ambíguas |
| AN-T04 | `PendingAction.operations` relaciona os efeitos anteriores já verificados |
| AN-T05 | `AnalysisFailure` identifica etapa, comando interrompido e conselho de retry sem inventar efeito |
| AN-T06 | `AnalyzerError` distingue configuração/solicitação inválida, escopo errado e resolução obsoleta |
| AN-T07 | `ResolutionMeaning::KeepBoth` nunca apaga uma das versões antes de preservar ambas |
| AN-T08 | Campos `Option` preservam ausência/desconhecimento sem sintetizar dados |
| AN-T09 | Nenhum retorno divulga segredo, token, conteúdo ou informação sensível desnecessária |

## Casos transversais

| Caso | Situação | Resultado esperado |
| --- | --- | --- |
| AN-S01 | Aviso de exclusão sem escopo completo | Reobservação pendente; nenhuma exclusão no outro lado |
| AN-S02 | Duas contas remotas com identificadores semelhantes | Isolamento por conta/provedor mantido |
| AN-S03 | Estado local e remoto modificado simultaneamente de maneiras compatíveis | Decisão reproduzível e execução somente das transformações seguras |
| AN-S04 | Falha após executar o primeiro de dois comandos | Relatório parcial correto; baseline não é avançado indevidamente |
| AN-S05 | Reinício após sucesso remoto e falha de registro | Reobservação e reconciliação antes de emitir novamente o comando |
| AN-S06 | Duas chamadas com revisão idêntica competindo pelo mesmo item | No máximo um conjunto incompatível de efeitos aplicado |
| AN-S07 | Dependência entre comandos (ex.: criar pai antes de transferir filho) | Ordem/dependências respeitadas; falha anterior impede execução dependente |
| AN-S08 | Escolha do usuário chega após nova alteração remota | `StaleResolution` ou nova pendência; nenhuma sobrescrita silenciosa |
| AN-S09 | Resolver conflito exige recurso não suportado pelo provedor | `Pending` seguro ou falha tipada; não simular sucesso |
| AN-S10 | Operação concluída com resposta não verificada | Reobservação e confirmação por evidência; baseline permanece estável até então |

## Isolamento e conformidade

1. Testar ambos os métodos públicos com cenários de sucesso, falha e ausência de efeitos proibidos.
2. Usar o Analisador concreto com fakes do Reconciliador, Conflitos e Operador para verificar coordenação; algoritmos internos terão suas suítes independentes quando documentados.
3. Validar confirmação com Inventário SQLite real temporário e `InventoryManager`, incluindo rollback e reabertura quando aplicável.
4. Validar separadamente que as funções do Operador foram chamadas **somente** pelo Analisador e com requisições tipadas completas.
5. Não reutilizar as funções de decisão sob teste para gerar expectativas; inspecionar estados e rastros independentes.
6. Marcar como `BLOCKED` os casos que dependam de persistência de pendências, recuperação de plano completo ou semânticas ainda não definidas nos contratos externos.
7. Executar compilação/testes Rust exclusivamente no Flatpak configurado pelo repositório e informar exatamente o que foi efetivamente executado.

**Estado:** plano de testes documental; nenhuma execução de teste Rust foi realizada por esta alteração.
