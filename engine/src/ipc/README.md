# Módulo ipc — integração externa

## Objetivo

Expor **estado e eventos de sincronização para clientes externos opcionais**, principalmente a futura extensão nativa do Nautilus. O motor e a interface GTK executam no **mesmo processo** e não dependem de IPC.

## Papel

Adaptador de infraestrutura para D-Bus de sessão. Traduz consultas externas em dados sanitizados, previamente disponíveis no motor. Não é um daemon, não executa planejamento, OAuth ou transferências.

## Quem usa

Extensão do Nautilus; futuramente ferramentas de diagnóstico e CLI, se explicitamente habilitadas.

## Dependências

Contratos de snapshots de `state`/`notifications`, modelos de status e raízes gerenciadas. A extensão não deve aceder diretamente aos módulos nem ao SQLite.

## Referências

[`docs/nautilus-integration.md`](../../../docs/nautilus-integration.md) e `docs/architecture.md` (a partir da raiz do repositório).

**Estado:** documentado, não implementado.
