# Conflitos

Identifica situações que exigem intervenção do usuário para **um item lógico**,
comparando baseline confirmado, estado local e remoto. É um algoritmo puro,
determinístico, privado ao Analyzer e independente do Reconciler.

A implementação atual **apenas identifica**: não define ações, opções de
resolução ou escolhas automáticas, não executa comandos e não solicita interação.

| Resultado | Significado |
| --- | --- |
| `NoConflict` | Nenhuma incompatibilidade comprovada nas dimensões avaliadas; o item ainda pode exigir sincronização |
| `Conflict` | Incompatibilidade comprovada; exige intervenção do usuário |
| `Undetermined` | Falta evidência para concluir; exige reavaliação após obter os dados faltantes |

O retorno conserva identidade, revisão, findings por dimensão e requisitos de
evidência. São conflitos comprovados as alterações concorrentes de conteúdo,
nome ou pasta com resultados distintos e a exclusão de um lado contra alteração
do outro. Mudanças unilaterais, alterações equivalentes e mudanças compatíveis
em dimensões diferentes não exigem decisão por si só.

`Unknown`, metadados não coletados e ausência de snapshots não provam exclusão.
Tamanho, timestamp e versão remota não provam igualdade de conteúdo. A avaliação
usa os tipos reais do Inventário/Models e comparações explícitas fornecidas pelo
coordenador, sem acessar banco, filesystem ou rede.

**Estado:** identificação implementada em `engine/src/analyzer/conflicts/`.
A fachada Analyzer persiste esta identificação no Inventory. Resolução de
conflitos, ações e metadados específicos permanecem fora desta etapa.

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)
