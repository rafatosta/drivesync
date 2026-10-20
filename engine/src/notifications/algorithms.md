# Algoritmos notifications

**Estado:** especificação proposta v0.2, independente de linguagem.

## IDs
NOTIFY-001 eventos, NOTIFY-002 progresso.

## Pré-condições
Entradas validadas, estado conhecido, pré-condições registradas e recursos disponíveis.

## Algoritmo
1. Receber evento de motor.
2. filtrar segredos.
3. agrupar atualizações de progresso.
4. publicar para assinantes.
5. disponibilizar snapshot.

## Pós-condições
Resultado confirmado e durável ou erro tipado com etapas parciais recuperáveis.

## Falhas
Rede, autorização, disco, versão concorrente, estado parcial, cancelamento e interrupção entre efeito e commit.

## Verificar
Identidade, versões, integridade e invariantes antes de ações destrutivas, e efeitos confirmados depois.

## Não inferir
Timeout não prova ausência; nome não prova identidade; erro parcial não confirma sucesso; estado UNKNOWN bloqueia exclusão.

## Invariante
Nenhum token em evento ou log.

## Complexidade
Metadados por item aproximadamente O(1); n operações O(n); B bytes transferidos O(B), excluídas latências externas.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### NOTIFY-001

- **Técnica conhecida:** Notificação assíncrona orientada a eventos.
- **Referências primárias:** [GIO Notification](https://docs.gtk.org/gio/class.Notification.html), [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html).
- **Política do DriveSync:** Emitir apenas eventos já conhecidos; estado persistente é fonte de verdade; payload não deve revelar credenciais.
- **Testes/oráculos a detalhar:** Evento fora de ordem, cliente fechou janela, segredo presente em texto de erro.
- **Limitações / questões abertas:** Definir classes de eventos e campos publicáveis.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### NOTIFY-002

- **Técnica conhecida:** Coalescência e reconexão a snapshot.
- **Referências primárias:** [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html).
- **Política do DriveSync:** Agrupar progresso ruidoso, mas preservar transições semânticas (SYNCED/ERROR/CONFLICT); clientes recuperam estado por snapshot.
- **Testes/oráculos a detalhar:** Evento perdido, 500 mudanças por segundo, Nautilus reiniciado.
- **Limitações / questões abertas:** Medir limites e frequência; sinais não garantem entrega de histórico.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
