# Instruções para agentes — Detector (fachada pública)

## Escopo e referências

Implementar o módulo **`detectors/` como coordenador/fachada pública**, independente da implementação real dos filhos. Ler [INTERFACES.md](INTERFACES.md), [README.md](README.md), [TESTS.md](TESTS.md), [AGENTS.md da raiz](../../../AGENTS.md), [MODULE_STANDARD.md](../MODULE_STANDARD.md) e [Inventário](../inventory/INTERFACES.md).

**Três unidades independentes:** `detectors/` (API pública, coordenação), `detectors/local/` (detecção local interna) e `detectors/remote/` (detecção remota interna). Implementar a fachada **não** autoriza reconstruir os filhos ou alterar o Inventário/Google API. Use fakes internos dos filhos nos testes do coordenador.

## Regras arquiteturais obrigatórias

1. **Única API pública externa:** `Detector`, seus objetos de requisição/resultado, status e `DetectorEvent`. O Orquestrador não depende de nenhum tipo ou método dos detectores internos.
2. **Interface interna mínima de coordenação:** start/pause/resume/stop/request_observation/status, apenas quando cabíveis aos filhos. Essa interface não dita formatos de watcher, paginação, cursores, políticas internas, classes ou arquivos Rust.
3. **Implementações distintas:** Local escolhe mecanismos de filesystem, Remoto escolhe integração Google; podem ter tipos, métodos auxiliares, estados e testes próprios. Eles não precisam conhecer o Orquestrador nem um ao outro.
4. **Notificações normalizadas:** converter sinais internos em `DetectorEvent` com identidade, motivo, itens e cobertura; o Orquestrador **não precisa distinguir origem local e remota** para encaminhar reavaliação.
5. **Coordenação e persistência:** os fatos devem chegar ao Inventário por sua API, nunca SQL; avisos que afirmam persistência exigem commit confirmado. Falhas precisam permanecer diagnosticáveis e recuperáveis.
6. **Sem execução física:** Detector não decide conflitos, não envia/baixa arquivos, não altera baseline, fila ou histórico de transferência e não interpreta erro como exclusão.
7. **Controle público mínimo:** apenas os **sete métodos** do contrato. Não criar APIs públicas especiais como `local_start`, `remote_poll` ou `on_inotify`.
8. **Isolamento por sincronização:** eventos, status e efeitos de uma pasta/conta não vazam para outra.

## Contract-First + TDD

- RED: primeiro testes de assinatura, visibilidade e comportamentos descritos em `TESTS.md`.
- GREEN: implementar os contratos usando **fontes internas fake independentes**, sink do Orquestrador simulado e Inventário isolado real quando necessário.
- REFACTOR: preservar API e regressão, sem impor arquitetura dos filhos.
- Desenvolver em unidades coesas: composição; controle de ciclo de vida; observações; eventos; estados; falhas/recuperação. Fazer commit local da unidade validada.
- Se um filho não existe ainda, não bloquear o teste unitário do coordenador: testar contra uma abstração interna falsa, distinguindo isso de integração real.

## Autonomia e conclusão

**“Implemente o Detector”** significa executar todos os **sete métodos públicos e o contrato de eventos** do pai, com os testes aplicáveis, **não** implementar automaticamente o Detector Local e o Remoto. Quando o usuário pedir explicitamente a integração completa dos três, planejar entregas separadas.

Pode avançar autonomamente pelos grupos e criar commits locais após validação, sem solicitar autorização por método. Não declarar conclusão com contratos pendentes, testes obrigatórios ausentes ou bloqueios omitidos. A fachada isolada validada não significa que o acompanhamento real do filesystem/Google já funciona.

## Ambiente e entrega

Toda execução Rust/Cargo/`clippy`/`test`/`doc` deve ocorrer **exclusivamente no Flatpak** definido em `flatpak/com.rtosta.DriveSync.Devel.yml`, conforme AGENTS global. Não recorrer a Rust do host nem instalar SDK ausente automaticamente.

Usar dados sintéticos, fakes das fontes internas, sink fake e armazenamento isolado. Não tocar arquivos pessoais, contas Google, credenciais, GUI ou operadores reais.

Comentários/rustdoc e mensagens de commit em inglês. Commits locais por unidade lógica; **sem git push/PR sem autorização explícita**.

Reportar os sete métodos, notificações, teste de visibilidade, resultados reais, comandos/ambiente, limitações, commits e pendências de integração. Nenhum teste foi executado apenas por criar este documento.
