# Arquitetura da engine

A engine separa observação, persistência, análise e execução. Os contratos em cada módulo distinguem o que está compilado do que permanece previsto; não interpretar diagramas de fluxo futuro como sincronização já implementada.

| Módulo | Responsabilidade |
| --- | --- |
| [Accounts](accounts/README.md) / [Connection](connection/README.md) | Identidade, autenticação, sessões e configuração das conexões |
| [Providers](providers/README.md) | Operações técnicas específicas de cada serviço externo |
| [Detector](detectors/README.md) | Captura Local/Remota, fila própria, leases, confirmação e diagnósticos |
| [Inventário](inventory/README.md) | Estados observados, identidades, revisões e baseline |
| [Analisador](analyzer/README.md) | Interpretação de um SyncItem, reconciliação/conflitos e coordenação de operações |
| [Operador](operations/README.md) | Execução dos comandos autorizados |
| [Orquestrador](orchestrator/README.md) | Coordenação do ciclo de sincronização pelas interfaces dos módulos |

## Arquitetura operacional

Accounts/Connection, Providers Google, Models, Detector, Inventory, Analyzer, Operator, e Orchestrator têm implementação Rust. O [fechamento do Detector](detectors/CLOSURE.md) descreve a fachada unificada. O Orchestrator inicia Local/Remote, consome as observações duráveis, persiste alterações no Inventory e entrega os recursos afetados ao Analyzer.

The current detection strategies are Local and Remote, and both publish the shared observation contract through the Detector's durable queue. Inventory persists synchronization state, Analyzer reconciles resources and decides operations, and Operator executes and verifies requested changes. Acknowledging a processed event does not by itself confirm synchronization.

Changes made locally while the application is closed are outside the current scope. Future planning may add a third retrospective local-observation producer inside Detector, implemented from scratch. The Orchestrator will not start or coordinate it directly.

## Persistência por pasta local

Cada par de sincronização reserva uma pasta oculta dentro da raiz local:

```text
<pasta-local>/.drivesync/
├── detector.sqlite3    # Eventos locais/remotos, leases e checkpoint remoto
└── inventory.sqlite3   # Identidades, metadados, baseline, operações e histórico
```

`detector.sqlite3` and `inventory.sqlite3` are separate active stores. The Detector owns its event queue and remote checkpoints; Inventory owns synchronization state and history. `.drivesync/` is excluded from observation and synchronized content. Accounts and credentials remain in their application stores. Development database formats are not migrated automatically.
