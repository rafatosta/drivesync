# Scanner Local — TESTS

## Objetivo

Validar que o Scanner reconstrói as observações locais no Inventário, preserva baseline e estado remoto e **prossegue apesar de falhas individuais**, devolvendo relatório fiel ao que conseguiu observar.

Os testes exercitam o Scanner concreto sobre um **filesystem temporário real** e um Inventário SQLite temporário conforme o contrato de `InventoryManager`. Detectores, Analisador, Operador e rede **não participam** dos testes.

## Fixtures

- Raiz temporária com arquivos, diretórios aninhados, nomes Linux válidos não UTF-8, symlinks e entradas opcionais especiais.
- Inventário com itens já conhecidos, baseline local/remoto e identidades preservadas.
- Entradas presentes, removidas, novas, inacessíveis e com atributos parcialmente indisponíveis.
- Falhas injetadas de leitura, `stat`, enumeração de diretório e persistência SQLite.
- Modificações concorrentes produzidas por tarefas de teste durante a varredura.
- `.drivesync/` com banco de inventário, excluída do escopo de sincronização.

## Casos por método público

| Caso | Método | Cenário | Resultado esperado |
| --- | --- | --- | --- |
| SC-01 | `new` | Configuração válida | Prepara Scanner sem ler arquivos nem executar operações |
| SC-02 | `new` | Escopo ou dependências incompatíveis | `ScannerError` tipado |
| SC-03 | `scan` | Raiz com arquivos e subpastas | Observações locais persistidas; contagens conferem com filesystem |
| SC-04 | `scan` | Arquivo novo | Novo vínculo no Inventário com `Present`, sem modificar remoto/baseline |
| SC-05 | `scan` | Arquivo existente com novo nome/caminho e identidade confiável | Atualiza observação correta sem inventar alteração remota |
| SC-06 | `scan` | Arquivo previamente presente não encontrado em escopo totalmente observado | `AbsentConfirmed` somente com prova de cobertura suficiente |
| SC-07 | `scan` | Diretório inacessível com descendentes conhecidos | `Partial`; descendentes não são marcados excluídos |
| SC-08 | `scan` | Arquivo inacessível ou metadado opcional indisponível | Continua nos outros itens; diagnóstico e presença conforme evidência |
| SC-09 | `scan` | Arquivo criado e excluído durante a varredura | Nenhuma suposição de estado final; registra observação/limitação possível |
| SC-10 | `scan` | Alteração concorrente durante gravação no Inventário | Revisão divergente impede substituição silenciosa; registra ocorrência |
| SC-11 | `scan` | Falha localizada de escrita no Inventário | Continua quando seguro, `Partial` e nenhum sucesso de gravação inventado |
| SC-12 | `scan` | Falha geral do Inventário, raiz inacessível ou storage ausente | `ScannerError` estrutural, sem confirmar ausências |
| SC-13 | `scan` | `.drivesync/` dentro da raiz | Não trata arquivos internos como itens de usuário |
| SC-14 | `scan` | Arquivos e diretórios vazios | Observações corretas, nenhuma transferência ou hash presumido |
| SC-15 | `scan` | Symlink apontando para fora da raiz | Nunca observa/modifica recursos fora do escopo |
| SC-16 | `scan` | Varredura concluída com erro de atributo opcional | `Completed` com `issues` registradas quando a cobertura for válida |
| SC-17 | `scan` | Varredura concluída com subárvore não lida | `Partial`, mantendo os itens conhecidos sem ausência inventada |
| SC-18 | `scan` | Cancelamento explícito | `ScannerError::Cancelled`, sem alegar conclusão |
| SC-19 | `scan` | Scan repetido sem mudanças | Sem perda de identidade, sem baseline modificado e efeitos idempotentes |
| SC-20 | `scan` | Registro de diagnóstico por item | Relatório e persistência verificáveis; marcar `BLOCKED` enquanto API do Inventário estiver indefinida |

## Casos transversais

| Caso | Garantia | Verificação |
| --- | --- | --- |
| SC-S01 | Fronteira arquitetural | Scanner não chama Detectores, Analisador, Operador nem Provider |
| SC-S02 | Sem mutação física | Conteúdo e nomes de arquivos permanecem intactos |
| SC-S03 | Preservação do baseline | Baseline, estados remotos e `last_synced_at` não são alterados |
| SC-S04 | Identidade | IDs de ocorrência, `SyncItemId` e IDs remotos não são confundidos |
| SC-S05 | Ausência segura | Falta de observação não implica exclusão sem cobertura suficiente |
| SC-S06 | Diagnóstico | Erro individual não encerra leitura de outros arquivos |
| SC-S07 | Persistência | Contagens de `items_recorded` correspondem a escritas efetivas |
| SC-S08 | Integridade de SQLite | Transações falhas não geram observações parcialmente confirmadas |
| SC-S09 | Segurança de caminhos | Não segue `..`, symlinks externos ou escopos não autorizados |
| SC-S10 | Alterações concorrentes | Revisões/limites de evidência impedem sobregravar estado com suposição |
| SC-S11 | Campos de Models | `NotCollected`, `Unavailable` e similares não são transformados em dados inventados |
| SC-S12 | Estado final | `Completed` com `issues>0` é permitido; `Partial` indica cobertura/gravação insuficiente |

## Critérios de aceite

1. Todos os métodos públicos possuem testes positivos e negativos, com asserções independentes sobre filesystem e Inventário.
2. A varredura percorre o que for acessível, mesmo que alguns arquivos tenham erro de leitura ou metadados.
3. Não existe exclusão física local ou remota por iniciativa do Scanner.
4. `AbsentConfirmed` exige cobertura suficiente, comprovada por teste que inclua subpastas inacessíveis.
5. Preservar os atributos de baseline e remoto antes e depois de todas as varreduras.
6. A estrutura de diagnóstico de cada item deve estar recuperável pelo Analisador através do Inventário antes de declarar integração completa; até lá, testes correspondentes permanecem `BLOCKED`.
7. Testes reais de Rust usam o Flatpak do projeto; reportar `PASS`, `FAIL` e `BLOCKED` sem declarar testes não executados.

**Estado:** plano de testes TDD documental; nenhuma implementação ou execução de testes Rust ocorreu nesta alteração.
