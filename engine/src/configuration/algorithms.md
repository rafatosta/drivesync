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
