# Módulo conflicts

## Objetivo
Classificar conflitos e propor preservação de versões.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
planner, coordinator, ui.

## Saídas e compromissos
Decisão explícita com versões preservadas.

## Limites: o que NÃO faz
Nunca sobrescrever silenciosamente versões divergentes.

## Viabilidade técnica
A [análise Rust/Linux × Google Drive API v3](technical-feasibility.md) mapeia os 44 casos do [checklist](validation-checklist.md) para evidências, módulos e limites; viabilidade não indica implementação ou validação.

## Pesquisa conceitual e experiência do usuário
O [estudo comparativo de resolução de conflitos](ux-research.md) reúne referências a Git, Syncthing, Nextcloud, MEGAsync e Google Drive e propõe linguagem, comparação de versões, preferências e fluxo de interação. Trata-se de investigação de produto, **não de implementação aprovada**.

## Revisão integrada de requisitos
O [checklist revisado](validation-checklist.md) preserva 44 códigos originais e adiciona N01–N12 (56 itens rastreáveis), separando conflitos reais, diferenças conciliáveis, incompatibilidades e erros. A [viabilidade técnica](technical-feasibility.md) e a [pesquisa de UX](ux-research.md) foram atualizadas em conjunto. Nenhum desses itens está marcado como validado por esta revisão.

## Algoritmos previstos
CONFLICT-001 preservar ambas; CONFLICT-002 resolver após escolha.

## Erros e cenários a verificar
Consultar o [checklist de validação de conflitos e falhas](validation-checklist.md), com matriz B/L/R, cenários C01–C25, erros E01–E10 e políticas pendentes. Os itens não indicam implementação ou teste concluído.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
