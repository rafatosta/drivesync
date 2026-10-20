# Algoritmos UI / ciclo de vida

## UI-001 inicialização
1. Detectar ativação normal versus autostart autorizado.
2. Construir/obter instância do motor uma única vez.
3. Em ativação normal, apresentar a janela única; no autostart, não apresentá-la.
4. Manter processo vivo com retenção apropriada enquanto motor ativo.

## UI-002 ocultação
1. Interceptar solicitação de fechar.
2. Ocultar janela; não emitir shutdown para motor.
3. Continuar processando eventos e atualizações sem bloquear GTK.

## UI-003 reabertura
1. Ao ativar o aplicativo já executando, localizar janela existente.
2. Apresentar janela, não criar segundo motor.

## UI-004 saída verdadeira
1. Receber comando explícito Sair.
2. Solicitar ao motor parada recuperável e confirmar journal.
3. Liberar retenção de aplicação e terminar.

## Testes
Fechar/reabrir; executar sem janela; comando Sair; autostart sem janela; duas ativações em sequência; falha do background portal; desempenho da UI sob sincronização.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### UI-001

- **Técnica conhecida:** Inicialização única e apresentação da janela.
- **Referências primárias:** [GIO Application hold](https://docs.gtk.org/gio/method.Application.hold.html).
- **Política do DriveSync:** Um único processo e motor; ativar abre/reexibe janela existente, sem duplicação.
- **Testes/oráculos a detalhar:** Duplo clique lançador, aplicação em background.
- **Limitações / questões abertas:** Inicialização de app e Flatpak em Fedora precisam de validação.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### UI-002

- **Técnica conhecida:** Ocultação da janela sem parar o motor.
- **Referências primárias:** [GIO Application hold](https://docs.gtk.org/gio/method.Application.hold.html).
- **Política do DriveSync:** Fechar X esconde interface sem pausar/encerrar; manter processo vivo com hold balanceado.
- **Testes/oráculos a detalhar:** Fechar durante transferência, app sem janelas, reabrir.
- **Limitações / questões abertas:** Confirmar consumo de recursos em background, permissões e ciclo de vida.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### UI-003

- **Técnica conhecida:** Reabertura de instância existente.
- **Referências primárias:** [GIO Application hold](https://docs.gtk.org/gio/method.Application.hold.html).
- **Política do DriveSync:** Reexibir janela no processo atual; não criar segundo coordenador sobre a mesma raiz.
- **Testes/oráculos a detalhar:** Ativações concorrentes, janela minimizada/oculta.
- **Limitações / questões abertas:** Separar single-instance do aplicativo de exclusão de operações por root.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### UI-004

- **Técnica conhecida:** Saída explícita e encerramento recuperável.
- **Referências primárias:** [GIO Application release](https://docs.gtk.org/gio/method.Application.release.html).
- **Política do DriveSync:** Sair comunica shutdown ao motor, persiste journal e libera hold; fechar X não é Sair.
- **Testes/oráculos a detalhar:** Sair durante download, timeout de fechamento, cancelamento.
- **Limitações / questões abertas:** Definir política de encerramento forçado e retomada após kill.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
