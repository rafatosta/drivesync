# Orientações para agentes — Detector Remoto Google

## Objetivo

Implementar a observação de alterações do Google Drive por meio das operações disponibilizadas em [Providers Google](../../../providers/google/INTERFACES.md).

## Regras de implementação

- Seguir as operações de observação e os tipos descritos em [INTERFACES.md](INTERFACES.md).
- Obter todas as informações externas exclusivamente pela interface `providers/google/`.
- Interpretar resultados, cursores, metadados e alterações do Google Drive.
- Converter respostas em eventos `File`, mantendo `RemoteFile` e identidade qualificada quando disponíveis.
- Preservar campos efetivamente recebidos; não fabricar valores ausentes ou concluir exclusão por perda de acesso.
- Emitir eventos individualmente e confirmar checkpoints somente após entrega durável.
- Preservar a separação entre mapeamento da API e interpretação das observações.

## Validação

Executar os cenários de [TESTS.md](TESTS.md) com interface de provedor simulada, sem rede externa ou credenciais reais. Seguir as instruções gerais do [AGENTS.md da raiz](../../../../../AGENTS.md).
