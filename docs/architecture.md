# Arquitetura DriveSync

## Decisão: aplicativo único, motor independente

O DriveSync será **um único processo Flatpak** com interface GTK4/Libadwaita e biblioteca Rust `drivesync-engine` carregada no próprio processo. **Não haverá daemon separado na primeira versão.** A separação é arquitetural no código, não entre processos.

- `src/main.rs`: aplicação GNOME e seu ciclo de vida; consome somente a API pública de `engine/src/lib.rs`.
- `engine/src/<módulo>/`: implementação e documentação juntas; módulos privados e funcionalidades encapsuladas.
- `docs/`: requisitos, contratos e decisões globais.
- `engine/src/ipc/`: **integração externa opcional** por API D-Bus de sessão, destinada à futura extensão do Nautilus e outros consumidores; não é necessário para a comunicação entre GUI e motor.

## Ciclo de vida

1. Abertura normal apresenta a janela existente.
2. Fechar pelo X **oculta a janela, sem encerrar a aplicação ou parar a sincronização**.
3. Reabrir pelo lançador traz a janela existente à frente; não inicia um segundo motor.
4. Inicialização automática opcional inicia o aplicativo **com a janela oculta**, mediante integração de desktop autorizada.
5. **Pausar** suspende ciclos de sincronização, não o processo.
6. **Sair do DriveSync** encerra o motor de maneira controlada e libera a retenção da aplicação.
7. O processo preserva seu ciclo de vida sem janelas visíveis mediante retenção explícita do `GApplication`, sem depender da existência da janela.

O `GtkApplication` deve manter uma instância lógica por sessão para o mesmo application ID; a prevenção de trabalhos duplicados por raiz também é responsabilidade do Coordinator.

## Integração com Nautilus

A extensão do Nautilus é um componente **opcional, instalado nativamente**, fora do Flatpak. Ela acessa uma API de estado no D-Bus de sessão, exposta pelo **mesmo processo** DriveSync quando ativo. Não acessa SQLite, tokens ou arquivos privados do sandbox.

- Consulta status de arquivo e raízes gerenciadas.
- Recebe sinais de mudanças e atualiza emblemas.
- Ausência do aplicativo ou falha de consulta equivale a `UNAVAILABLE`, não a `SYNCED`.
- Consultas locais rápidas: a extensão não executa chamadas à Google Drive API nem bloqueia o Nautilus com rede.

Validar cedo a posse de nome no barramento D-Bus, permissões Flatpak, ciclo de vida em segundo plano e a API do Nautilus antes da implementação da extensão.

## Funcionamento interno

A API pública encapsula módulos privados; o Coordinator verifica as dependências funcionais e orquestra mudanças. Módulos colaboram por dados de domínio e contratos, não por variáveis globais mutáveis. Autenticação é pré-condição de sincronização remota real; testes unitários podem usar provedores simulados.

## Estado atual

A biblioteca ainda não implementa sincronização; a janela inicial apresenta somente “DriveSync”. Esta decisão não cria um daemon ou extensão de Nautilus nesta etapa.

## Referências internas

[Dependências entre módulos](module-dependencies.md) · [Dados compartilhados](shared-context.md) · [API pública](public-engine-api.md) · [Invariantes](invariants.md) · [Integração Nautilus](nautilus-integration.md).

## Metodologia de construção

A modularização é também a estratégia de reduzir complexidade: cada módulo encapsula **problemas cuja técnica já está documentada**, enquanto o DriveSync explicita suas próprias políticas de segurança e integração. Para cada algoritmo deve haver **fundamentação técnica com referência primária, limitações, política própria, testes e questões em aberto**. A implementação só começa após aprovação do comportamento e de seus oráculos; a IA recebe tarefas pequenas.

[Metodologia](development-methodology.md) · [Catálogo de problemas](problem-catalog.md) · [Modelo de algoritmo](algorithm-spec-template.md) · [Regras para IA](ai-development-guidelines.md).


## Múltiplas contas (requisito obrigatório do MVP)

**Não duplicar `DriveSyncEngine` nem abrir um processo/daemon por conta.** A instância única mantém um gerenciador de sessões de conta com `AccountId` estável e operações escopadas por `AccountId + SyncRootId`. O mesmo código em `auth`, `remote`, `changes`, `planner`, `download`, `queue` e `state` atende todas as sessões.

O único motor compartilha recursos (executores, conexão ao banco, limites globais) com **isolamento lógico forte**: credenciais próprias, cursores no escopo correto da Drive API, filas e estados identificados por conta, pausa individual, erros limitados à conta afetada. Ver [modelo de domínio](data-model.md) e [ADR multi-conta](decisions/0004-single-engine-multi-account.md).

A interface mostra uma visão geral e cartões/estados por conta. Contas diferentes nunca compartilham ou sobrepõem a mesma raiz local. O MVP deve comprovar sincronização real de **duas contas distintas**; isso não representa limite fixo do produto.
