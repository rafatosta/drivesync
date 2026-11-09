# Orientações para agentes — Accounts

## Escopo

`accounts/` administra contas cadastradas, identidade estável, metadados e vínculo seguro com as credenciais locais. **Não existe `accounts/auth/`.** A API especializada em autenticação pertence a [Providers](../providers/INTERFACES.md); o Secret Service/keyring é armazenamento privado sob controle de Accounts.

## Leituras obrigatórias

- [README.md](README.md) — responsabilidades.
- [INTERFACES.md](INTERFACES.md) — contratos públicos e tipos.
- [TESTS.md](TESTS.md) — matriz TDD e invariantes.
- [Providers](../providers/INTERFACES.md) e [Google](../providers/google/INTERFACES.md) — autenticação técnica específica.
- [Models](../models/INTERFACES.md) — tipos compartilhados.
- [MODULE_STANDARD](../MODULE_STANDARD.md) — convenções documentais.
- [Configuration](../configuration/README.md) — propriedade e política de dados persistidos.

## Regras de implementação

1. Não implementar OAuth, OIDC, PKCE, refresh ou chamadas HTTP próprias em Accounts; usar operações específicas do provedor.
2. Não impor um `trait ProviderAuth` universal ou catálogo obrigatório em `providers/`; seus contratos atuais são especializados.
3. Não criar ou restaurar `accounts/auth/`, nem mover a guarda segura de credenciais para o Provider.
4. Não expor tokens, refresh tokens, segredos, conexões SQL ou keyring handles brutos pela API pública.
5. Guardar credenciais no armazenamento seguro associado à conta e provedor, nunca em campos de metadados, texto claro ou logs.
6. Validar o identificador externo estável; não identificar contas por e-mail ou nome.
7. Projetar idempotência e recuperação entre cadastro e keyring, sem supor transação atômica entre sistemas distintos.
8. Não atribuir ao Accounts a propriedade do cadastro de conexões antes de decisão contratual. Não permitir remoção que torne sincronizações órfãs.
9. Não executar ações de sincronização nem exclusões locais/remotas por remoção de conta.
10. Quando houver falhas de armazenamento persistente, distinguir indisponibilidade do keyring de corrupção de banco; não fazer reset destrutivo nem backup inseguro de segredos.
11. Sem compatibilidade com formatos legados não solicitados, sem mudanças destrutivas ou migrações retroativas.
12. Implementação Rust/testes reais somente no ambiente Flatpak; não alegar que testes foram executados sem evidência.

## Estado operacional e observabilidade

- Expor `Accounts::status()` como consulta síncrona, rápida, somente às operações **ativas nesta instância**; não consultar keyring, banco nem rede nessa consulta.
- Não confundir `Account.state` com atividade de execução ou indicador de conectividade em tempo real.
- Acompanhar `add/remove/acquire_session/update_authorization` por operações/fases transitórias; `get/list/update` não precisam de monitoramento artificial.
- Preservar `AddAccountRequest.request_id` opcional para correlação antes do `AccountId`, sem carregar dados de autorização no status.
- Remover o registro transitório em todos os desfechos, inclusive erro/cancelamento, sem registrar uma fila permanente ou um novo gerenciador de tarefas.
- Não bloquear GTK com Secret Service ou I/O externo; definir runtime e threading concretos na implementação.
- Não adicionar `pause/resume/stop` ou outra abstração universal de Provider. O fluxo externo de consentimento continua no Provider; Accounts recebe o resultado já autorizado.
- Se houver armazenamento persistente de metadados, a política de recuperação é do seu Storage, sem dependência obrigatória de Configuration e sem backup automático de credenciais.
- Cobrir ACC-S01 a ACC-S12 com testes determinísticos.

## Critério de aceite

Quatro arquivos de contrato coerentes, API de contas independente da tecnologia de autenticação, keyring privado a Accounts, ausência de referências funcionais a `accounts/auth/` e cobertura TDD para falhas, isolamento e exclusão segura.

**Estado:** documento orientativo, não implementação.
