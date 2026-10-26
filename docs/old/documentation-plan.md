# Plano de documentação técnica pós-MVP

Estado: planejamento para execução após o primeiro MVP.

## Objetivo

Consolidar a documentação do DriveSync como produto de engenharia de software. Após o MVP, realizar engenharia reversa e recuperação da arquitetura implementada, confrontando código, testes e requisitos planejados. Produzir um documento técnico principal que descreva o software real no marco avaliado.

## Documento técnico consolidado

Incluir concepção, objetivos, requisitos funcionais e não funcionais, rastreabilidade, casos de uso, arquitetura, componentes, módulos, interfaces, modelos de dados, segurança, recuperação de falhas, testes, implantação e limitações. Distinguir requisitos planejados, implementados e validados.

## Diagramas

Elaborar diagramas UML de casos de uso, classes, sequência, atividades, estados, componentes e implantação conforme sua utilidade. Considerar C4 para visão arquitetural. Manter fontes editáveis (PlantUML ou Mermaid) e imagens exportadas SVG ou PNG. Derivar os diagramas do código e do comportamento verificado, não apenas do planejamento original.

## Documentação Rust

Adotar rustdoc, ferramenta equivalente ao Javadoc: comentários /// em itens e //! em módulos. Documentar contratos, parâmetros, retornos, erros, invariantes, concorrência e exemplos quando relevantes. Gerar HTML e executar doctests:

    cargo doc --workspace --no-deps
    cargo doc --workspace --no-deps --document-private-items
    cargo test --workspace --doc

Durante o MVP, comentários rustdoc acompanham a implementação e a
refatoração de cada módulo, conforme `docs/development-methodology.md`.
Após o MVP, auditar a cobertura e a coerência da documentação do código
como parte da documentação técnica consolidada.

## Execução após o MVP

1. Delimitar o escopo do marco e inventariar código, testes e documentação existente.
2. Recuperar a arquitetura real por análise de código e execução.
3. Confrontar requisitos e arquitetura planejados com a implementação.
4. Mapear requisitos para componentes e testes e identificar divergências.
5. Elaborar diagramas e revisar contratos rustdoc.
6. Consolidar o documento técnico e revisar sua consistência.
7. Avaliar a retirada do planejamento temporário MVP.md e a consolidação de documentos redundantes, preservando informações relevantes.

## Política

Documentar o produto por marcos relevantes, não a cada commit. O histórico de alterações permanece no Git. Não iniciar essa reorganização durante o MVP, salvo correções de informações operacionais ou de segurança que se tornem incorretas.
