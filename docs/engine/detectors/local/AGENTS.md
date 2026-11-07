# Instruções para agentes — Detector Local (DriveSync)

## Escopo e fontes de verdade

Implementar **exclusivamente o Detector Local**. É independente do Detector Remoto; uma solicitação para implementar Local **não autoriza** implementar ou alterar Remoto.

Antes de modificar código, ler [INTERFACES.md](INTERFACES.md), [README.md](README.md), [TESTS.md](TESTS.md), [AGENTS.md global](../../../../AGENTS.md), [MODULE_STANDARD.md](../../MODULE_STANDARD.md) e o contrato público do [Inventário](../../inventory/INTERFACES.md). Os contratos são referência, não a implementação anterior.

## Responsabilidades e limites

- Inspecionar arquivos, diretórios e eventos dentro de uma única raiz local por sincronização.
- Registrar observações exclusivamente via API pública do Inventário; jamais executar SQL externo, modificar baseline, cursor remoto, fila ou histórico de operações.
- Produzir avisos tipados para o Orquestrador via sink injetado. Aviso **não é** comando de sincronização.
- Nunca acessar Google Drive, APIs de OAuth, credenciais ou endpoints remotos.
- Nunca realizar upload/download, renomear, mover ou excluir fisicamente arquivos por decisão própria.
- Não alterar módulos vizinhos para simplificar testes sem uma solicitação explícita; contratos de `LocalFile` e do sink devem ter proprietário definido.

## Garantias de segurança

1. Raiz desmontada, pasta inexistente, subárvore ilegível, watcher interrompido, overflow ou cancelamento não comprovam exclusão.
2. Uma varredura parcial não autoriza `AbsentConfirmed` para arquivos não observados; `Complete` precisa ser relativa ao escopo efetivamente coberto.
3. Não seguir symlinks para fora da raiz, não sincronizar `.drivesync/`, não modificar dados pessoais.
4. Capturar ou reconciliar mudanças ocorridas entre registrar watcher e concluir varredura, e recuperar mudanças feitas com aplicativo fechado por nova varredura.
5. Preservar bytes de nomes Unix não UTF-8; não depender exclusivamente de caminho/inode como identidade estável.
6. Registrar fatos observados sem avançar baseline nem criar operações de transferência.
7. Notificações podem ser perdidas, agregadas ou repetidas; não depender de entrega exatamente uma vez.
8. Mensagens de erro e logs devem ser sanitizados; nenhum conteúdo ou caminho pessoal desnecessário.

## Fluxo de implementação: Contract-First + TDD

| Fase | Obrigação |
| --- | --- |
| Contrato | Identificar método e atributos em INTERFACES; resolver ambiguidade relevante |
| RED | Testar assinatura e comportamento contra filesystem temporário/Inventário real |
| GREEN | Implementar mínimo para fazer os testes passarem |
| REFACTOR | Melhorar código preservando interface e comportamento |
| Regressão | Rodar casos anteriores, segurança, reinício e falhas |
| Commit | Commit **local** coeso, com comandos/resultados efetivos |

Implementar por unidades coesas: ciclo de vida; varredura e metadados; watcher e concorrência; notificação; recuperação após reinício. Não acumular implementação inteira e testar apenas no fim.

## Autonomia e definição de conclusão

Quando solicitado **“Implemente o Detector Local”**, concluir todos os **5 métodos públicos** e os casos aplicáveis de `TESTS.md`, avançando autonomamente após validar cada unidade. Não exigir implementar o Detector Remoto. Se a solicitação delimitar um grupo, entregar somente o grupo e registrar os demais como pendentes.

Não declarar conclusão quando houver testes exigidos não executados, contrato indefinido, implementação fictícia ou falhas bloqueadoras. Registrar claramente o que está aprovado e o que não foi possível testar.

## Ambiente e evidências

- Executar Rust/Cargo, `fmt`, `clippy`, `test`, documentação e doctests **somente dentro do Flatpak** previsto no manifesto `flatpak/com.rtosta.DriveSync.Devel.yml`, conforme AGENTS global.
- Usar filesystem **real e temporário**, SQLite real isolado acessado por APIs do Inventário, notificações simuladas e relógio/scheduler determinísticos. Não usar conta Google nem pasta pessoal.
- Verificar watchers e casos de filesystem no Linux/Flatpak; mock do próprio Detector Local **não** demonstra capacidade real.
- Testar cancelamento, mudanças concorrentes e indisponibilidade, sem afirmar completude quando houve lacuna.
- Comentários e mensagens de commit em inglês; commits pequenos por unidade. **Não executar push ou abrir PR sem autorização explícita**.

## Entrega

Relatar 5 contratos por status, cenários LOC-01…LOC-05 e transversais, comandos e ambiente, evidências de reinício, cobertura apurada se disponível, commits locais, limitações e decisões em aberto. Diferenciar teste isolado do funcionamento real de toda a engine.

**Este documento é somente instrução. Sua criação não executa testes nem implementa código.**
