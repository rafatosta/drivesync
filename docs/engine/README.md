# Arquitetura da engine

A engine separa observação, persistência, análise e execução. Os contratos em cada módulo distinguem o que está compilado do que permanece previsto; não interpretar diagramas de fluxo futuro como sincronização já implementada.

| Módulo | Responsabilidade |
| --- | --- |
| [Accounts](accounts/README.md) / [Connection](connection/README.md) | Identidade, autenticação, sessões e configuração das conexões |
| [Providers](providers/README.md) | Operações técnicas específicas de cada serviço externo |
| [Detector](detectors/README.md) | Captura Local/Remota, fila própria, leases, confirmação e diagnósticos |
| [Scanner Local](scanner/README.md) | Levantamento local e registro de observações no Inventário |
| [Inventário](inventory/README.md) | Estados observados, identidades, revisões e baseline |
| [Analisador](analyzer/README.md) | Interpretação de um SyncItem, reconciliação/conflitos e coordenação de operações |
| [Operador](operations/README.md) | Execução dos comandos autorizados |
| [Orquestrador](orchestrator/README.md) | Coordenação do ciclo de sincronização pelas interfaces dos módulos |

## Estado compilado e próximos limites

Accounts/Connection, Providers Google, Models e Detector têm implementação Rust. O [fechamento do Detector](detectors/CLOSURE.md) descreve a fachada unificada e sua validação. O Orquestrador atual é uma composição diagnóstica mínima que instancia essa fachada e disponibiliza o conector; não executa ainda o fluxo completo de sincronização.

O Scanner é exclusivamente local. Não recebe eventos dos Detectores, não acessa APIs remotas e não entrega itens diretamente ao Analisador. Seu contrato foi especificado posteriormente para manter o levantamento local separado da análise por item. Os contratos de Scanner/Inventário/Analisador/Operador e do Orquestrador completo devem ser conferidos antes das respectivas implementações.

O fluxo futuro é `Detector → Orquestrador → Inventário → Analisador`, com `Scanner Local → Inventário` no levantamento local e `Analisador → Operador` na execução. A leitura inicial remota pertence ao especialista do Detector. Consumo terminal no Developer confirma somente a observação diagnóstica, nunca sincronização ou baseline.
