# configuration — algoritmos

**Status:** rascunho v0.2.

## Identificadores
CONFIG-001 validar, CONFIG-002 migrar.

## Objetivo
Implementar a responsabilidade do módulo sem acoplar a interface ao motor.

## Pré-condições
Dados de entrada validados, dependências acessíveis ou erro tipado.

## Algoritmo
1. Validar pasta, conta e ID remoto.
2. recusar raízes sobrepostas.
3. validar seleção e política de exclusão.
4. persistir atomicamente.
5. migrar versão com backup.

## Pós-condições
Resposta ou erro explícito sem perda silenciosa de estado.

## Erros
Configuração inválida, falha temporária de comunicação, interrupção, cancelamento e incompatibilidade de versão.

## Verificações
Identidade e versão de contexto, efeitos confirmados e invariantes globais.

## Proibido
Mudar configuração não deve produzir deleção automática. Timeout não significa sucesso ou ausência remota.

## Complexidade
Por comando O(1) sem contar eventuais itens de lista; para n registros O(n).


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### CONFIG-001

- **Técnica conhecida:** Validação e persistência atômica de configuração.
- **Referências primárias:** [GLib GSettings](https://docs.gtk.org/gio/class.Settings.html), [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html).
- **Política do DriveSync:** Validar root, política, paths e duplicidade antes de ativar; jamais interpretar mudança de pasta como autorização de exclusões.
- **Testes/oráculos a detalhar:** Root não existe, raiz dentro da raiz, caminho relativo, migração falhou.
- **Limitações / questões abertas:** Definir storage para preferências e perfil antes de codificar; GSettings não é obrigação.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### CONFIG-002

- **Técnica conhecida:** Migração versionada e consentimento de autostart.
- **Referências primárias:** [GLib GSettings](https://docs.gtk.org/gio/class.Settings.html), [Flatpak Background Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html).
- **Política do DriveSync:** Separar intenção de iniciar com sessão da autorização efetiva; migrar dados com rollback/backup.
- **Testes/oráculos a detalhar:** Autostart negado, configuração anterior incompleta, versão futura.
- **Limitações / questões abertas:** Comportamento portal em GNOME/Flatpak alvo precisa de experimento real.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

## CONFIG-003 — Configuração inicial e roteamento

Fluxo: conectar → selecionar pasta remota/local → salvar → dashboard. Na abertura,
qualquer conta com pasta salva leva ao dashboard; sem configuração, uma conta
verificada vai à seleção. Falhas de autenticação permanecem visíveis no dashboard
sem apagar a configuração. Transferências e autostart não são ativados aqui.

- Selecionar pasta remota por ID com navegação paginada no My Drive; consultar
  metadados reais pelo auth do motor, sem expor tokens à UI. Revalidar pasta e
  permissão ao salvar. Não aceitar arquivos, atalhos ou itens na lixeira.
- Selecionar diretório local existente pelo portal GTK. Canonicalizar aliases e
  symlinks; no Flatpak, resolver caminhos do Documents portal pelo GetHostPaths
  (versão 5), preservando o caminho exportado para acesso e o original para
  exibição/comparação. Falha na resolução impede salvar; confirmar escrita e rejeitar destinos iguais/ancestrais/descendentes
  de outra configuração. Uma pasta por conta neste incremento.
- Persistir conta, pasta remota e caminho local atomicamente no índice de contas
  versão 2. Ler versão 1 sem perda; criar backup antes da primeira escrita v2.
  Cancelar não grava; falha não conclui configuração. Verificar sobreposição sob
  o mesmo lock da gravação. Não transferir nem apagar arquivos do usuário.

Referências: [Drive files.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list),
[busca de pastas](https://developers.google.com/workspace/drive/api/guides/search-files),
[GTK FileDialog](https://docs.gtk.org/gtk4/method.FileDialog.select_folder.html).

Portal de diretórios: [Documents.GetHostPaths](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Documents.html#gethostpaths).
