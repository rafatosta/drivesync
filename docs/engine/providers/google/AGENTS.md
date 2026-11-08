# Google Provider — AGENTS

> Escopo: organizar e implementar futuramente **a exposição/instanciação da API Google existente** dentro de `providers/google/`. Aplicar [README](README.md), [INTERFACES](INTERFACES.md), [TESTS](TESTS.md), [Providers — AGENTS](../AGENTS.md) e [Google API — AGENTS](api/AGENTS.md).

## Contrato de propriedade

- `providers/`: mantém catálogo de serviços, seleciona `ProviderId`, disponibiliza `get` e `open`.
- `providers/google/`: implementa o mecanismo específico de disponibilização/composição da API Google e de seu contexto autenticado.
- `providers/google/api/`: **única fonte de verdade dos métodos, tipos, protocolos, respostas e erros Google**. Preservar os 59 métodos REST + 9 OAuth/OIDC + 10 auxiliares e seus testes.
- `accounts/auth/`: coordena consentimento, keyring e ciclo de credenciais; usa métodos Google específicos obtidos via Providers.
- Especialistas de Detector/Upload/Download: conhecem métodos Google obtidos pela fábrica, mas só cumprem/exibem contratos dos seus módulos pais.

## Regras

1. **Não** copiar, redesenhar, renomear ou reduzir os métodos técnicos de `api/INTERFACES.md` nesta etapa.
2. **Não** criar segundo conjunto de funções OAuth/Files/Changes/Upload/Download em `providers/google/`.
3. `GoogleFile`, `GoogleChange`, `GoogleSession` podem ser usados internamente por especialistas Google, mas não entram no contrato genérico `RemoteDetector`/Inventário.
4. A fábrica obtém a API correta por `ProviderId` e valida autorização de `AccountId`; não lê bancos privados do Inventário nem armazena credenciais em texto.
5. Não fabricar conta autenticada com `get`; `open` exige contexto autorizado e não mistura credenciais.
6. Nenhum cliente OAuth/HTTP avulso fora da infraestrutura de Providers; um especialista não pode burlar a obtenção pela fábrica.
7. Estruturas de handles devem evitar dependências circulares, token em `Debug`, clones sem controle de sessão ou resultados positivos falsos.
8. Respeitar as decisões de cada módulo consumidor: Detector possui checkpoint e `FileBatch`; executores possuem operações; Inventário possui `SyncItem`/baseline; Accounts possui cadastro.
9. Não criar adaptadores de compatibilidade legada nem um serviço universal de arquivos por conveniência.

## TDD e entrega

Antes do código, fechar representação dos handles/sessões e validação de autorização. Implementar a composição em passos pequenos com fakes da API Google. Validar [TESTS.md](TESTS.md), preservar os testes independentes de [api/TESTS.md](api/TESTS.md) e relatar com precisão o que passou, falhou ou ficou bloqueado.

**Este documento é proposta de implementação, não comprovação de existência de arquivos Rust.**
