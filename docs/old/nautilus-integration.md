# Integração opcional com o Nautilus

**Estado:** especificação, não implementado.

## Componentes

- **DriveSync Flatpak:** único aplicativo/processo, incluindo UI, motor e adaptador D-Bus opcional.
- **Extensão Nautilus nativa:** componente instalado por pacote do sistema ou mecanismo suportado pelo Nautilus; implementa o provedor de informação/emblemas e consulta exclusivamente o contrato D-Bus.
- **Serviço de estado:** implementado em `engine/src/ipc/`, consumindo snapshots/eventos sanitizados de `notifications` e `state`.

## Contrato D-Bus conceitual

Identificador proposto: `com.rtosta.DriveSync`; objeto proposto: `/com/rtosta/DriveSync/Status`; interface versionada a validar na implementação. Não confundir identificador do aplicativo com a interface de estado.

- `GetFileStatus(uri) -> (status, revision_or_generation)`
- `GetSyncRoots() -> array of root URIs + metadata não sensível`
- Sinal `FileStatusChanged(uri, status, generation)`
- Estados: `SYNCED`, `SYNCING`, `PENDING`, `CONFLICT`, `ERROR`, `UNKNOWN`, `UNAVAILABLE`, `NOT_MANAGED`.

O contrato público definitivo deverá definir limites de lote, tipos de parâmetros, serialização e versionamento. É aconselhável oferecer `GetFileStatuses(uris)` para evitar muitas chamadas por pasta.

## Regras

1. O adaptador responde apenas com estado **já conhecido e em cache**; não faz rede síncrona.
2. Nunca expor refresh tokens, conteúdo dos arquivos, banco de dados ou caminho interno do Flatpak.
3. Restringir consultas ao mínimo necessário e tratar clientes não confiáveis; presença no barramento não equivale a autorização completa.
4. Não informar `SYNCED` quando o motor estiver indisponível ou snapshot for obsoleto.
5. Eventos podem se perder; a extensão sempre consegue recuperar um snapshot.
6. Pasta gerenciada agrega estados filhos por regra documentada, distinguindo desconhecido de sincronizado.
7. A extensão não deve impedir o Nautilus de navegar caso o Flatpak esteja parado.

## Flatpak e ciclo de vida

Acesso ao D-Bus de sessão e propriedade do nome precisam ser validados no manifest e testados contra um cliente nativo. Permissões de background/autostart dependem do ambiente, do portal e de consentimento do usuário; não presumir que ocultar a janela garante manutenção do processo no ambiente Flatpak.

## Testes de integração futuros

- Abrir DriveSync visível/oculto; fechar janela; consultas permanecem operacionais.
- Reabrir sem duplicar processo/motor.
- Encerrar DriveSync: extensão apresenta `UNAVAILABLE`.
- Perder evento: consulta em lote recompõe emblemas.
- Sem extensão instalada: DriveSync opera normalmente.
- Sem permissão/sem D-Bus: motor opera e informa integração indisponível sem falhar na sincronização.
