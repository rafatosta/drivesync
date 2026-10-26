# ADR-0004 — Um motor com sessões isoladas para múltiplas contas

**Status:** decidido para MVP; implementação pendente.

## Contexto
O DriveSync deve sincronizar **várias contas Google simultaneamente** sem exigir processos separados, instâncias duplicadas do motor ou tratamento premium por conta. A primeira prova funcional usará uma conta; a aceitação do MVP exige pelo menos duas.

## Decisão
Manter `DriveSyncEngine` único, com coleção de `AccountSession` identificadas por `AccountId`. Um único processo Flatpak e uma GUI que mostra conta individual e visão geral. Os 16 módulos já existentes são suficientes; **nenhuma reorganização de diretórios é necessária**.

Cada sessão usa autorização, estado e fila escopados. `SyncRoot` referencia `AccountId`; operações têm `account_id`, `root_id` e identidade remota contextualizada. Limites globais de I/O/requisições são compartilhados com política justa. Estado global e SQLite podem ser compartilhados com isolamento por chaves e regras de ownership. Não compartilhar token nem cursor entre contas.

Rejeitar raízes locais sobrepostas entre contas. Falhas independentes de rede/credencial atingem somente a conta afetada; corrupção do banco global pode degradar todo o motor para preservar integridade.

## Consequências
Não duplicar algoritmos nem serviços por conta. Exigir desde os primeiros schemas e contratos suporte a múltiplas contas, mesmo que os testes iniciais utilizem uma. A fase F13 do [MVP](../MVP.md) é gate obrigatório de aceitação. D-Bus/Nautilus permanecem opcionais.

## Riscos a validar
Quotas compartilhadas do Google Cloud/projeto; política de seleção de OAuth account; renovação concorrente; granularidade do feed `changes`; correspondência de nomes/IDs entre contas; fairness e limite de recursos; guarda contra sobreposição de raízes.

## Referências
[Modelo de dados](../data-model.md) · [MVP](../MVP.md) · [API pública](../public-engine-api.md) · [Arquitetura](../architecture.md).
