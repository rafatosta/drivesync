# Template obrigatório de especificação por algoritmo

Copiar/adequar esta estrutura para cada algoritmo `MOD-XXX` (preferencialmente arquivo próprio em `engine/src/<module>/algorithms/`; catálogos atuais `algorithms.md` continuam válidos como índices).

## Identificação
- ID, título, módulo responsável, status, versão da especificação, última revisão e responsável por validação.
- Dependências funcionais e consumidores (com contrato e teste duplo correspondente).

## Problema e objetivo
- Situação concreta a resolver; exemplos e contraexemplos; o que não resolve.
- Critério observável de sucesso e riscos de integridade.

## Fundamentação técnica — obrigatória
- **Técnica estabelecida:** nome canônico do método e por que é adequado.
- **Fonte primária:** link para RFC, documentação oficial, publicação ou código-fonte relevante; versão/data verificada.
- **Limitações da técnica/fonte:** pressupostos e diferença para Google Drive/Flatpak/Linux.
- **Alternativas avaliadas:** motivo da escolha ou da recusa.
- **Regra própria do DriveSync:** decisão de produto explicitamente separada da técnica.
- **Questões em aberto:** hipóteses ainda não comprovadas e experimento necessário.
- **Licença:** se for reutilizado código de terceiros, identificar licenciamento e obrigações; não incorporar trechos sem revisão.

## Contrato
- Entradas tipadas, saídas tipadas, precondições, pós-condições.
- Estados antes e depois, efeitos colaterais, operação destrutiva? (sim/não).
- Idempotência, paralelismo, cancelamento, timeout, recursos consumidos.
- Falhas tipadas e semântica (erro transitório, conflito, resultado ambíguo, fatal).

## Algoritmo
Pseudocódigo sem elementos específicos de Rust; ramificações devem ser explícitas. Separar side effects de cálculos puros. Explicar fronteiras atômicas e revalidação necessária. Prever repetição após crash.

## Matriz de testes
| Teste | Estado inicial e entrada | Falha injetada? | Resultado/efeito esperado | Evidência |
|---|---|---|---|---|
| MOD-XXX-T01 | especificar | não | especificar | pendente |

Obrigatórios: nominal, inválido, repetição, concorrência, resultado remoto ambíguo se aplicável, interrupção em pontos críticos e propriedade de não perder dados.

## Implementação e validação
- Local do código, bibliotecas selecionadas + versão/motivo.
- Testes e revisão humana efetuados; comandos reproduzíveis; cobertura relevante.
- Mudanças de contrato e decisões arquiteturais correlatas.
- Estado final e limitações remanescentes.

**Importante:** o template é checklist de engenharia, não uma certificação de segurança por preenchimento. Não preencher fatos não verificados.
