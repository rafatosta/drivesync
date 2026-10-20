# Contrato de configuração

## Responsabilidades
Validar e persistir preferências por perfil de conta e por aplicativo, incluindo `start_with_session`, `auto_sync` e `close_to_background` (na v1, fechar normalmente oculta; saída explícita termina).

## Pré-condições
Autostart só pode ser marcado efetivo após autorização/integração com o ambiente; preferência desejada não equivale à permissão concedida.

## Saídas
`RequestedAutostart`, `AutostartEnabled`, `AutostartDenied`, `AutostartUnsupported`; configuração validada, persistida e auditável.

## Invariantes
Nenhuma preferência de ciclo de vida modifica implicitamente filtros de exclusão ou resultados de sincronização.
