# Detector Remoto Google — implementação especializada

## Objetivo

Este submódulo **especializado** implementará o contrato interno definido em [Detector Remoto](../INTERFACES.md) utilizando as operações da [Google API](../../../providers/google/api/INTERFACES.md).

Ele decide **como observar o Google Drive** (consulta incremental, paginação, cursor, recuperação de mudanças e intervalos). O pai `detectors/remote/` define **o que a implementação precisa oferecer** e o tipo de evento individual que deve emitir. A fachada `detectors/` agrega esses eventos em fila própria e fornece lotes ao Orquestrador.

## Limites

- Pode acessar Google API por contrato; não implementar diretamente protocolo HTTP nem autenticação da conta.
- Não acessar implementação de `detectors/storage/`; emitir um sinal de cada vez para sink injetado, com sucesso somente após aceitação durável.
- Não importar Detector Local, Inventário, Orquestrador ou operadores.
- Controlar checkpoint da leitura Google sem avançar depois de evento rejeitado.
- Não decidir exclusões com base em `removed` ou resposta incompleta.

## Independência

Testar e implementar com Google API simulada, sink de eventos fake e checkpoint fake/temporário. Nenhum Detector pai real, Local real ou Storage real é exigido. Não criar versões futuras do OneDrive como pré-requisito.

[INTERFACES](INTERFACES.md) · [TESTS](TESTS.md) · [AGENTS](AGENTS.md). **Esta é uma especificação de implementação, não código existente.**
