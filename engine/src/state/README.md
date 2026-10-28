# Módulo state

Persiste observações e journal por escopo de cliente, conta e raízes. O recorte
STATE-001/002 de downloads está implementado: separa planejamento de instalação,
fornece bases confirmadas e rejeita estado inválido sem reset automático.
A recuperação geral bidirecional e a reconstrução de base perdida permanecem
pendentes; arquivos existentes nunca reconstroem B por inferência.

Entradas, transações, seleção de B, erros e limites estão em
[algorithms.md](algorithms.md). A organização da raiz e a matriz de regras,
implementação e testes ficam em [preparation.md](preparation.md).
Consultar também o [contrato](contract.md) e os
[requisitos vigentes](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização).
A bateria A01–A12 com Google Drive real permanece pendente.
