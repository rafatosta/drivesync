# Providers — AGENTS: fronteira única de integrações externas

## Objetivo e escopo

Implementar `providers/` como **único ponto da engine para APIs remotas**, conforme [README](README.md), [INTERFACES](INTERFACES.md) e [TESTS](TESTS.md). O estado é documentação de arquitetura-alvo; **não afirmar implementação Rust existente**.

## Dependências e limites obrigatórios

1. **Apenas `providers/` executa protocolos HTTP/REST/OAuth contra provedores**. Outros módulos recebem `ProviderAuthentication`, `ProviderRemoteObservation`, `ProviderRemoteFiles` ou `ProviderTransfers` via fachada de Providers.
2. `providers/google/api/` continua sendo contrato técnico especializado, **privado à implementação Google para os consumidores da engine**, mesmo que seus tipos Rust internos sejam públicos dentro da crate por razões técnicas. Não importar `GoogleFile`, `GoogleSession` ou clientes REST do Google em Accounts, Detector, Operações, Inventário ou Models.
3. O catálogo de provedores inclui **somente implementações reais**. `example/` é placeholder, não backend funcional, não deve aparecer como opção de login.
4. `accounts/` possui o cadastro; `accounts/auth/` coordena consentimento, keyring, renovação e revogação; Providers executa protocolo externo e traduz resposta. Credenciais protegidas jamais entram em logs/SQLite de inventário, `File` ou `SyncItem`.
5. O Detector Remoto decide quando ler alterações e gerencia checkpoint + `EventStore`; Providers **não** produz `FileBatch`, não reconhece eventos e não confirma checkpoint.
6. Operações autorizadas de upload, download, renomeação, movimentação e exclusão usam Providers, mas são decididas/executadas funcionalmente por seus operadores. Providers não inspeciona baseline nem resolve conflito.
7. `RemoteFile` genérico e `RemoteFileRef` pertencem a Models. Conversão de `GoogleFile` e retenção de `ProviderFileData` ficam em `providers/google/`, mantendo estados de coleta.
8. Erros de acesso, token, rede, paginação, limite de cota e recurso indisponível não provam exclusão remota.
9. Adição de recurso específico do Google deve primeiro avaliar capacidade genérica reutilizável; quando não houver equivalência, não forçar campos falsos ou acrescentar 59 métodos ao contrato genérico.
10. Não modificar código Rust, criar adaptadores legados, remover APIs externas existentes ou alterar assinaturas de módulos consumidores sem solicitação de implementação e testes específicos.

## Processo de implementação

- Começar por catálogo, validação de conta/sessão e portas mínimas necessárias ao primeiro provedor (Google); não implementar um conector universal hipotético.
- Fazer TDD por capacidade, com fake da API especializada e keyring de teste; separar testes de autenticação e chamadas de arquivos.
- Preservar contratos específicos Google existentes como implementação de protocolo, sem promovê-los à API da engine.
- Só adicionar capacidades efetivamente implementadas ao catálogo e à UI; operações não suportadas são erro explícito.
- Provar isolamento: dois `AccountId`, reautorização, retries, cancelamento, limitação de cota e identidades remotas iguais em contas diferentes.
- Documentar quais métodos Rust foram realmente implementados, testados e quais permanecem conceituais.

## Decisões em aberto — não improvisar

Tipos concretos e segredos de autenticação, lifetime/async, sessão por conta, codecs de metadados, transferências retomáveis, remoção vs perda de acesso, paginação e cursores são pontos que precisam ser fechados antes do código. Não chamar implementação incompleta de sucesso.

## Referências de consumidores

[Accounts](../accounts/README.md) · [Detector Remoto](../detectors/remote/INTERFACES.md) · [Upload](../operations/upload/README.md) · [Download](../operations/download/README.md) · [Models](../models/INTERFACES.md).
