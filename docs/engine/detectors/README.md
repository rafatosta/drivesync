# Detectores — organização dos componentes

Os detectores do DriveSync são **dois módulos independentes**, cada um responsável por observar um ambiente diferente e produzir fatos e avisos para o Orquestrador. Não existe contrato público único que obrigue implementá-los ou testá-los conjuntamente.

| Módulo | Origem observada | Estado da documentação |
| --- | --- | --- |
| [Detector Local](local/README.md) | Filesystem da pasta sincronizada | [README](local/README.md), [INTERFACES](local/INTERFACES.md), [TESTS](local/TESTS.md), [AGENTS](local/AGENTS.md) definidos |
| [Detector Remoto](remote/README.md) | Google Drive via Google API | **Documentação independente ainda pendente**; `remote/README.md` é apenas um placeholder vazio |

## Responsabilidades comuns, sem acoplamento de implementação

- Identificar mudanças e necessidades de reavaliação, sem decidir conflitos, transferir conteúdo ou determinar exclusões por suposição.
- Comunicar alterações ao Orquestrador por contratos públicos, e persistir observações apenas pelo Inventário quando aplicável.
- Não assumir que evento isolado, falha de acesso ou resposta incompleta comprovam exclusão.
- Manter possibilidade de recuperação após reinício, sem depender somente de notificações em tempo real.

**Dependências não são simétricas:** o Local usa filesystem e Inventário; o Remoto usa Google API e Inventário. Nenhum requer que o outro esteja implementado para que seus próprios contratos e testes sejam validados.

Cada submódulo segue o [padrão documental](../MODULE_STANDARD.md) e mantém seus próprios `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md`. O diretório `detectors/` contém apenas esta orientação arquitetural, não interfaces ou testes conjuntos.

Referência: [SYSTEM_ARCHITECTURE.md](../../SYSTEM_ARCHITECTURE.md), seção Detectores.

**Estado:** documentação do Local preparada; Remoto permanece para etapa separada. Nenhum detector foi implementado nesta alteração.
