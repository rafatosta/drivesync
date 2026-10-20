# ADR-0003 — Implementação fundamentada e incremental

**Estado:** adotada como regra de processo, ainda sem validação técnica dos módulos.

## Contexto
O produto exige sincronização confiável e o desenvolvedor quer aprender Rust revisando unidades pequenas. Geração ampla e opaca de código dificultaria a explicação de decisões e a depuração.

## Decisão
Cada módulo agrupa função, contrato, algoritmos, referências e testes. Para **cada algoritmo**, distinguir técnica documentada, política DriveSync e hipótese aberta. Especificar oráculos antes de gerar código. Utilizar IA em diffs pequenos e revisáveis, com revisão humana e execução comprovada dos testes. Toda mudança de comportamento requer atualização da especificação.

## Consequências
Maior esforço documental inicial e velocidade menor por funcionalidade, em troca de rastreabilidade, segurança e aprendizado. Cumprir o processo não substitui testes de integração e a comprovação de confiabilidade.

## Referências
`docs/development-methodology.md`, `docs/ai-development-guidelines.md`, `docs/algorithm-spec-template.md`, `docs/reference-catalog.md`.
