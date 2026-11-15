# Orientações para agentes — Conflitos

## Objetivo

Implementar um avaliador puro de conflitos para **um item lógico, dois lados e um baseline opcional**, retornando somente classificação, motivos e requisitos de evidência. O usuário limitou esta etapa a identificar; ações e resoluções serão implementadas posteriormente.

## Leitura obrigatória

- [README.md](README.md): objetivo, fluxo e invariantes.
- [INTERFACES.md](INTERFACES.md): contrato `evaluate`, parâmetros, tipos, retornos e erros.
- [TESTS.md](TESTS.md): matriz unitária e critérios de aceite.
- [Analisador](../INTERFACES.md): tipos de pendência, resoluções, comandos e coordenação.
- [Inventário](../../inventory/INTERFACES.md) e [Models](../../models/INTERFACES.md): identidade, baseline, presença e metadados observados.
- [AGENTS.md da raiz](../../../../AGENTS.md): regras gerais.

## Implementação

- Implementar somente o contrato de `evaluate` com entrada única `ConflictRequest` e saída `ConflictAssessment`.
- Reutilizar os modelos e enums proprietários dos módulos existentes. Não criar cópias de `SyncItemId`, `SyncBaseline`, `LocalFile`, `RemoteFile`, `OperationCommand` ou `ResolutionMeaning`.
- Manter a avaliação determinística, sem I/O e sem estado externo mutável.
- Comparar presença, conteúdo, nome, localização e metadados de forma independente, usando o baseline confirmado quando disponível.
- Tratar metadados desconhecidos, observações parciais e ausência de snapshot como evidência insuficiente, quando a informação for necessária à decisão.
- Não interpretar timestamps/tamanho idênticos como prova de conteúdo idêntico, nem nomes iguais como identidade comprovada.
- Retornar `NoConflict`, `Conflict` ou `Undetermined` com `findings` coerentes. Intervenção do usuário somente para conflito comprovado; incerteza requer evidência.
- Não propor resolução automática, opções, comandos nem ações. Descrever apenas os requisitos de evidência que faltam.
- Não usar o Operador, acessar Providers, alterar Inventário, criar filas ou interagir com o usuário.
- Manter os contratos do Analisador intactos nesta fase; registrar divergências de integração sem editar outros módulos.

## Testes e validação

Implementar os cenários `CF-01` a `CF-09` e transversais `CF-S01` a `CF-S10` antes de declarar conformidade. Priorizar testes table-driven de matrizes de estado e casos extremos de presença/exclusão e metadados não coletados. Usar expectativas independentes do algoritmo testado.

A execução de compilação e testes Rust deve ocorrer exclusivamente no ambiente Flatpak descrito no repositório. Registrar testes realmente executados, falhas e limites. Não afirmar existência ou comportamento do código a partir apenas da especificação.

## Escopo

A implementação autorizada está em `engine/src/analyzer/conflicts/`, privada ao Analyzer, acompanhada por estes documentos. Preservar o Reconciliador e os contratos da fachada Analyzer; a integração será tratada posteriormente.

**Estado:** identificação implementada. A fachada persiste a identificação no Inventory; ações e opções de resolução permanecem fora do escopo.
