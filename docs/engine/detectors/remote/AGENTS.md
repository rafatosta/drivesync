# Orientações para agentes — Detector Remoto

## Objetivo

Implementar observação de alterações remotas, com eventos individuais e checkpoint de leitura.

## Regras de implementação

- Seguir as operações e os contratos documentados em [INTERFACES.md](INTERFACES.md).
- Utilizar apenas as funções públicas de [Providers](../../providers/README.md) para acesso às APIs externas.
- Processar paginação, cursores e resultados parciais conforme as capacidades de cada serviço.
- Preservar a identidade e o conteúdo observado dos eventos.
- Confirmar checkpoints somente após entrega durável das ocorrências correspondentes.
- Representar erros e perda de cobertura como estados observados, sem presumir exclusão.
- Manter a lógica de interpretação de cada serviço em seu especialista.

## Validação

Seguir [TESTS.md](TESTS.md), utilizando interfaces simuladas dos serviços, eventos e checkpoints isolados. Aplicar também as instruções gerais do [AGENTS.md da raiz](../../../../AGENTS.md).

## Estado e controle

- Expor atividade e contadores da rodada por `status()`, sem disparar rede.
- Não implementar pausa de entrega no Remoto: o pai pausa o consumo externo; polling e emissão durável continuam.
- `stop` interrompe polling preservando checkpoint e eventos duravelmente aceitos; não decidir verificação/reconciliação pós-parada.
- Testar REM-S-01 a REM-S-05, sem depender de Orquestrador real.
