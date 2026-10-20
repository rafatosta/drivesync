# Algoritmos IPC — futura extensão Nautilus

**Status:** proposta; não implementado.

## IPC-001: consultar status de arquivo

1. Receber URI de arquivo ou lote e validar esquema, limites e formato.
2. Localizar raiz gerenciada com correspondência segura (não confiar em prefixo textual ambíguo).
3. Consultar snapshot de status já existente no motor.
4. Identificar snapshot expirado ou motor degradado e classificar como `UNKNOWN`, nunca `SYNCED`.
5. Retornar o estado tipado, geração e erro controlado quando aplicável, sem revelar segredos.

## IPC-002: publicar mudança

1. Consumir mudança tipada do módulo de notificações.
2. Projetar em estado público (somente URI, status e geração, sem dados internos).
3. Agrupar mudanças frequentes e emitir sinal D-Bus.
4. Permitir ao cliente reconectar e consultar snapshot completo se perder sinais.

## IPC-003: integração resiliente

1. Tentar adquirir nome D-Bus permitido no sandbox.
2. Se negado, registrar erro sanitizado e continuar motor/GUI.
3. Se cliente desconectar, descartar apenas assinatura; jamais encerrar a sincronização.

## Testes

Sem cliente; cliente lento; reinício do Nautilus; perda de sinal; URI inválida; permissão D-Bus negada; DriveSync oculto; DriveSync encerrado; status obsoleto; tentativa de consultar arquivo não gerenciado.

## Invariantes

Nenhuma transferência ou Google Drive API síncrona dentro da operação de status. Nenhum estado obsoleto declarado sincronizado. Nada de gerenciamento do ciclo de vida do motor por causa da extensão.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### IPC-001

- **Técnica conhecida:** Consulta tipada ao estado via D-Bus de sessão.
- **Referências primárias:** [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html), [Flatpak Background Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html).
- **Política do DriveSync:** A GUI usa API Rust diretamente; D-Bus só para cliente externo opcional, não inicia transferências nem exige daemon.
- **Testes/oráculos a detalhar:** Flatpak aberto/oculto, extensão Nautilus consulta arquivo fora da raiz, permissão negada.
- **Limitações / questões abertas:** Provar integração D-Bus Flatpak↔cliente nativo.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### IPC-002

- **Técnica conhecida:** Publicação e recuperação de eventos/snapshot.
- **Referências primárias:** [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html).
- **Política do DriveSync:** Emissor informa mudanças; consumidor que perdeu sinais volta a consultar snapshot; indisponibilidade ≠ SYNCED.
- **Testes/oráculos a detalhar:** Nautilus reiniciado, múltiplos arquivos, sessão sem nome no barramento.
- **Limitações / questões abertas:** Definir versão do protocolo, capacidade de batch e timeouts curtos.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### IPC-003

- **Técnica conhecida:** Falha graciosa de integração externa.
- **Referências primárias:** [Flatpak Background Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html), [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html).
- **Política do DriveSync:** Perda de permissão/nome IPC não encerra motor; extensão nativa não consulta SQLite privado.
- **Testes/oráculos a detalhar:** App iniciado sem D-Bus acessível, cliente malformado, falha de sandbox.
- **Limitações / questões abertas:** Identificador final do bus e permissões do manifest ainda não validados.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
