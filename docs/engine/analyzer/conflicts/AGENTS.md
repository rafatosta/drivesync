# Orientações para agentes — Conflitos

## Objetivo

Implementar um avaliador puro de conflitos para **um item lógico, dois lados e um baseline opcional**, retornando presença de conflito, evidências, encaminhamento e opções seguras.

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
- Retornar `NoConflict`, `Conflict` ou `Undetermined` com `findings` coerentes; preencher `decision` conforme o contrato.
- Só propor resolução automática com evidências e precondições suficientes; caso contrário devolver opções válidas ou reobservação.
- Não usar o Operador, acessar Providers, alterar Inventário, criar filas ou interagir com o usuário.
- Manter os contratos do Analisador intactos nesta fase; registrar divergências de integração sem editar outros módulos.

## Testes e validação

Implementar os cenários `CF-01` a `CF-09` e transversais `CF-S01` a `CF-S10` antes de declarar conformidade. Priorizar testes table-driven de matrizes de estado e casos extremos de presença/exclusão e metadados não coletados. Usar expectativas independentes do algoritmo testado.

A execução de compilação e testes Rust deve ocorrer exclusivamente no ambiente Flatpak descrito no repositório. Registrar testes realmente executados, falhas e limites. Não afirmar existência ou comportamento do código a partir apenas da especificação.

## Escopo

Esta etapa abrange somente os quatro documentos da raiz de `docs/engine/analyzer/conflicts/`. A documentação do Reconciliador e os contratos do Analisador principal permanecem inalterados e serão revisados na etapa de integração.

**Estado:** especificação documental inicial; não representa código implementado.
