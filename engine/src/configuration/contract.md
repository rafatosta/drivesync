# Contrato de configuração

## Responsabilidades
Validar e persistir preferências por perfil de conta e por aplicativo, incluindo `start_with_session`, `auto_sync` e `close_to_background` (na v1, fechar normalmente oculta; saída explícita termina).

## Pré-condições
Autostart só pode ser marcado efetivo após autorização/integração com o ambiente; preferência desejada não equivale à permissão concedida.

## Saídas
`RequestedAutostart`, `AutostartEnabled`, `AutostartDenied`, `AutostartUnsupported`; configuração validada, persistida e auditável.

## Invariantes
Nenhuma preferência de ciclo de vida modifica implicitamente filtros de exclusão ou resultados de sincronização.

## Preferências multi-conta

Aplicativo tem preferências globais (iniciar na sessão, fechar para segundo plano) e preferências **por conta/raiz** (ativar sincronização, pausa, pasta local, política). Cada root referencia uma `AccountId`; não permitir que roots locais de duas contas coincidam ou se sobreponham. Validar caminhos normalizados e symlinks com segurança, não somente prefixos textuais. Preferência global jamais modifica silenciosamente o estado individual das contas.

## Configuração inicial de pasta (CONFIG-003)

Uma pasta remota e um destino local por conta, separados por cliente OAuth.
`configure_folder` revalida metadados do Drive, acesso local e sobreposição antes
de salvar; erro/cancelamento não conclui a configuração. `saved_folder` recupera
a escolha. `RestoredAccount.folder` orienta o roteamento para seleção ou dashboard,
independentemente de falhas temporárias de autenticação. Regras e armazenamento:
[CONFIG-003](algorithms.md#config-003--configuração-inicial-e-roteamento).
