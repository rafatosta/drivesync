# Instruções para agentes — Detectores (DriveSync)

## Escopo e referências obrigatórias

Implementar e testar o componente independente `detectors/`, composto pelos detectores **Local** e **Remoto**. Os detectores produzem observações e notificações para o Orquestrador, utilizando as APIs do Inventário e da Google API, sem executar ações de sincronização.

**Ler nesta ordem:** [INTERFACES.md](INTERFACES.md) (contratos), [README.md](README.md) (responsabilidades e invariantes), [TESTS.md](TESTS.md) (matriz TDD), [AGENTS.md global](../../../AGENTS.md) (ambiente, commits e segurança), [MODULE_STANDARD.md](../MODULE_STANDARD.md) (padrão de módulos). Consultar também os contratos públicos de [Inventário](../inventory/INTERFACES.md) e [Google API](../google_api/INTERFACES.md) antes de integrá-los.

Não usar detalhes privados nem comportamento legado como fonte normativa. Contradições ou lacunas devem ser explicitadas e resolvidas na documentação **antes** de implementar o comportamento afetado.

## Fronteiras de responsabilidade

- **Detector Local:** filesystem, watcher, varredura, metadados observados e notificação.
- **Detector Remoto:** sessões recebidas, inventário remoto, changes paginados, recuperação por cursor e notificação.
- **Inventário:** única autoridade para persistência de SyncItem, metadados observados, cursores e eventos. Acessá-lo **somente pela API pública**, nunca via SQLite ou tabelas.
- **Google API:** única autoridade para detalhes de acesso ao serviço, HTTP, JSON, tokens e endpoints. Não criar um segundo cliente HTTP dentro dos detectores.
- **Orquestrador:** recebe notificações; decide agendamento global, políticas e encaminhamento ao Analisador.
- **Analisador e operadores:** interpretam divergências e executam operações autorizadas; nenhum detector escolhe conflito ou transfere bytes.

Não alterar GUI, contas, configuração, implementação interna de Inventário/Google API ou lógica dos operadores para facilitar testes. Dependências incompletas devem ser simuladas por interfaces públicas tipadas, sem criar contratos novos silenciosamente.

## Regras de segurança inegociáveis

1. Um erro de rede, permissão, paginação, watcher, mount ou diretório ausente **não** prova exclusão.
2. Somente varredura completa e validada de um escopo autoriza registrar ausência confirmada para esse escopo; varreduras parciais não autorizam exclusão em massa.
3. `.drivesync/` é reservado, não sincronizável e não pode gerar itens de usuário.
4. Um arquivo pode mudar enquanto se registra o watcher ou se percorre uma árvore. Capturar e reconciliar a janela de concorrência.
5. Ao reiniciar após aplicação fechada, efetuar nova observação local e continuar mudanças remotas pelo cursor persistido, sem depender de eventos efêmeros.
6. `removed=true` no Google Changes é um sinal de reavaliação; pode representar perda de acesso e não só exclusão física.
7. Campos ausentes e metadados parciais não devem ser substituídos por zero, vazio, `false` ou exclusão inventada.
8. Não avançar baseline, concluir fila, registrar atividade de transferência ou executar upload/download.
9. Notificações repetidas, perdidas ou agregadas devem ser toleradas; não afirmar entrega exatamente uma vez.
10. Nunca vazar OAuth, URLs sensíveis, SQL, conteúdo privado, nomes e caminhos pessoais desnecessários em logs e erros.

## Contratos públicos e implementação

- Os **10 métodos** documentados em [INTERFACES.md](INTERFACES.md) definem o escopo inicial: `new`, `start`, `scan_once`/`poll_once`, `stop` e `status` por detector.
- Preservar objetos coesos `Request`/`Result`, significados dos opcionais, enums `DetectorState`, `ScanCoverage`, motivos e erros.
- Notificação é via `DetectorNotificationSink`, uma fronteira injetada do Orquestrador; a publicação não é confirmação de sincronização.
- Contratos externos (incluindo `GoogleSession`, `SyncItem`, `LocalFile`) continuam propriedade dos módulos fornecedores. A forma concreta dos handles e runtime pode ser definida na implementação, **sem mudar a semântica pública**.
- Não impor um novo banco ou estado durável duplicado; o Inventário persiste observações e cursor.
- Os detalhes do scheduler/watch/poll são internos, mas devem comprovar reavaliação após interrupção e cobertura válida.
- Não inventar valores de debounce, polling, limites de página ou política de symlink antes de decisão expressa.

## Fluxo Contract-First + TDD

| Etapa | Obrigação |
| --- | --- |
| Contrato | Identificar método, tipos, campos e efeitos no `INTERFACES.md` |
| RED | Escrever teste público e cenário com filesystem temporário, SQLite real e Google fake |
| GREEN | Implementar o mínimo que satisfaz os casos existentes |
| REFACTOR | Simplificar sem alterar contratos nem perder comportamento |
| Regressão | Executar testes anteriores, especialmente ausência/rollback/reinício |
| Commit | Registrar unidade funcional local com validação documentada |

Implementar por grupos lógicos: tipos/estado; detector local e monitoramento; detector remoto e mudanças paginadas; sink/notificações; recuperação e concorrência. Testar antes de avançar para outro grupo; não implementar tudo em lote e testar apenas ao final.

### Autonomia e definição de conclusão

**“Implemente o módulo Detectores”**, sem restrição adicional, significa concluir **os dois detectores e todos os 10 métodos públicos**, suas integrações públicas e casos aplicáveis de `TESTS.md`. Prosseguir autonomamente entre unidades validadas, sem pedir autorização por método/commit local. Se a solicitação disser apenas “Detector Local” ou “Detector Remoto”, respeitar a restrição, informar a outra metade como pendente e **não anunciar o módulo integral como concluído**.

Só declarar conclusão integral com métodos implementados, testes pertinentes realmente executados em ambiente Flatpak, retomada e falhas demonstradas, invariantes mantidas e nenhum bloqueio conhecido relevante. Se houver dependência/contrato em aberto, registrar bloqueio específico, impacto e escopo restante. Não usar stubs de sucesso para interfaces pendentes.

## Testes e ambiente

- Executar ferramentas Rust/Cargo **somente no Flatpak** definido por `flatpak/com.rtosta.DriveSync.Devel.yml`; não usar host Rust nem instalar SDK ausente automaticamente.
- Testar o detector local em filesystem real temporário, inclusive nomes bytes não UTF-8, root inacessível, symlink externo e alterações feitas com watcher parado.
- Testar o remoto com **provider fake** de Google API e Inventário real SQLite temporário. Não chamar Google, não usar conta real, credenciais OAuth ou dados pessoais.
- Testar janelas entre watcher e scan, polling paginado, cursor expirado, parcialidade, repetição e falha de notificação. Confirmar persistência por API do Inventário após reabertura.
- Distinguir fake de dependência e mock da unidade sob teste: o segundo não demonstra detecção real.
- Executar `cargo fmt`, `clippy`, `test`, `doc` e doctests quando disponíveis, **dentro do Flatpak** e reportar comandos realmente executados.
- Mensagens de commit e comentários de código/rustdoc **em inglês**; seguir commits atômicos e regredir testes anteriores antes de novo commit.
- Não executar `git push`, criar PR ou publicar resultados remotamente sem autorização explícita.

## Entrega

Relatar os **10 métodos por status**, IDs dos cenários DET-01…DET-10, notificações/segurança/recuperação verificadas, comandos/ambiente reais, commits locais, dependências, bloqueios e limitações. Diferenciar validação isolada de teste com Google Drive real e do funcionamento no Flatpak instalado.

**Estado deste documento:** instruções de implementação. Criá-lo não equivale a implementar nem testar o componente.
