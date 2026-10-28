# Módulo planner

Classifica B/L/R no fluxo de download de binários e constrói planos por IDs/pais.
A classificação é pura; a construção recebe observador injetado e não escreve.
O plano não autoriza efeitos sem revalidação no executor.

O recorte PLAN-001 está implementado; comparação geral bidirecional, exclusões,
renomeações e resolução permanecem fora dele. SYNC-002 para futuras escritas
remotas continua pendente. Entradas, decisões, erros e limites estão em
[algorithms.md](algorithms.md); [contrato](contract.md),
[rastreabilidade e evidências](../state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
e [requisitos vigentes](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização)
completam a referência. Testes locais não equivalem ao aceite A01–A12 com Google
Drive real, que permanece pendente.
