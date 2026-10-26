# Documentação global DriveSync

## Mockups e interface

- [Galeria dos mockups e auditoria GNOME HIG / compatibilidade com o motor](ui-mockups.md)

## Configurar Google Cloud

- [**Tutorial: criar projeto Google Cloud e testar o DriveSync com OAuth próprio**](google-cloud-setup.md)
- [Perfis de compilação: Development e Production](build-profiles.md)

## Publicação, autorização Google e site

- [**Plano de publicação, builds e verificação Google OAuth (checklist)**](google-publication-plan.md)
- [**Plano do site público, política de privacidade, termos e GitHub Pages**](website-plan.md)

## Comece aqui

**[MVP — escopo, roteiro progressivo, checklist e critérios de aceite](MVP.md)**. Esse é o plano de implementação em vigor. Seus gates dependem uns dos outros e podem ser reabertos após descobertas posteriores.


- [Arquitetura](architecture.md)
- [Requisitos](requirements.md)
- [Invariantes](invariants.md)
- [Modelos](data-model.md)
- [Contratos](contracts.md)
- [Máquinas de estado](state-machines.md)
- [Dependências dos módulos](module-dependencies.md)
- [Contexto de dados](shared-context.md)
- [API pública](public-engine-api.md)
- [Ciclo de vida e background](app-lifecycle.md)
- [Integração Nautilus](nautilus-integration.md)
- [**Método de desenvolvimento**](development-methodology.md)
- [**Template de algoritmo**](algorithm-spec-template.md)
- [**Diretrizes de IA**](ai-development-guidelines.md)
- [**Catálogo de referências técnicas**](reference-catalog.md)
- [**Estudo de viabilidade**](feasibility-validation.md)
- [Estratégia de testes](testing-strategy.md)

A documentação local dos 16 módulos fica junto de `mod.rs`, em `engine/src/<modulo>/`; não criar árvore `modules/` separada.
