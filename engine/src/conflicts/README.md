# Módulo conflicts

Classifica diferenças B/L/R e fornece encaminhamentos conservadores sem I/O,
GTK, persistência ou execução de operações. O [contrato](contract.md) define
entradas, precedência, garantias e limites. A implementação usa tipos normalizados
em [models.rs](models.rs) e decisão pura em [classification.rs](classification.rs).

`planner` e `changes` consomem os adaptadores de [planning.rs](planning.rs).
A coleta/prova de conteúdo e localização permanece em `changes`; o executor
revalida os efeitos suportados. Uma classificação conciliável não habilita
rename/move, atualização remota, exclusão nem resolução assistida.

A especificação vigente está em
[SOFTWARE_SPECIFICATION.md](../../../docs/SOFTWARE_SPECIFICATION.md#sync-006--classificação-centralizada).
Os oráculos de C01/C03/C04/C09/C12/C21/P01 estão ligados em
[algorithms.md](algorithms.md#classificação-sync-006).

O [catálogo](validation-checklist.md) contém 56 cenários/políticas, não 56
funcionalidades implementadas. A [viabilidade](technical-feasibility.md) e a
[pesquisa de UX](ux-research.md) são referências conceituais. A [preservação SYNC-010](preservation.md) adiciona cópia local verificada da
versão remota em divergências binárias comprovadas no mesmo caminho. Retenção,
limpeza e resolução assistida continuam propostas. A classificação permanece
pura: somente o relatório de execução confirma cópias adicionais; cópia não
significa resolução nem sucesso de sincronização.
