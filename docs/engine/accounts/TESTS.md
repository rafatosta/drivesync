# Accounts — TESTS

## Objetivo

Validar o cadastro e ciclo de vida de contas, a associação de credenciais armazenadas de forma segura e a fronteira com os provedores. **Não testar OAuth real dentro de Accounts**: simular as respostas e sessões do Provider especializado. Validar integração real com keyring/Secret Service no ambiente Flatpak quando disponível.

## Fixtures

- Repositório de cadastro isolado com contas sintéticas de provedores distintos.
- Provider falso com identidade estável, autorização válida, expirado/reautorização, falha de rede, usuário alterado e credenciais revogadas.
- Armazenamento seguro isolado/falso sem exportação de tokens; testes de integração com backend Secret Service quando disponível.
- Conta com sincronizações vinculadas via interface simulada do proprietário de conexões (contrato `AccountAssociations`/`RemovalGuard`).
- Falhas injetadas entre gravação em keyring e cadastro local, com reinício.

## Matriz TDD

| ID | Método/cenário | Resultado esperado |
| --- | --- | --- |
| ACC-01 | `new`, configuração válida | Instância preparada sem disparar OAuth |
| ACC-02 | `new`, armazenamento indisponível | Falha tipada; nenhum dado sobrescrito |
| ACC-03 | `add`, identidade verificada e autorização válida | Conta criada; referência de credenciais segura |
| ACC-04 | `add`, mesmo provedor + ID externo repetido | Sem conta duplicada; comportamento idempotente |
| ACC-05 | `add`, mesmo e-mail com identidades externas diferentes | Não juntar contas por e-mail |
| ACC-06 | `add`, credencial ausente/inválida ou provider falha | Não registrar conta conectada fictícia |
| ACC-07 | `get`, ID válido/inexistente | Retorno íntegro ou `NotFound` |
| ACC-08 | `list`, múltiplos provedores/filtro | Contas corretas, sem segredos nem rede |
| ACC-09 | `update`, nome de apresentação | Altera somente campo local permitido |
| ACC-10 | `remove`, conta sem vínculos | Cadastro e segredos removidos de modo seguro |
| ACC-11 | `remove`, conta vinculada a sincronização | `AccountInUse`, sem excluir arquivos |
| ACC-12 | `acquire_session`, credencial válida | Retorna handle opaco, não token |
| ACC-13 | `acquire_session`, sessão expirada | Provider determina renovação ou reautorização; Accounts não implementa OAuth |
| ACC-14 | `update_authorization`, mesma identidade externa | Atualiza credencial mantendo `AccountId` |
| ACC-15 | `update_authorization`, outra identidade externa | `IdentityMismatch`, sem substituir credenciais |
| ACC-16 | Falha entre keyring e cadastro | Compensação/recuperação consistente sem conta "Connected" inválida |
| ACC-17 | Falha de Secret Service/keyring | Erro tipado; sem fallback inseguro para SQLite/log/arquivo |
| ACC-18 | Reabertura do cadastro | IDs e dados persistidos; segredos ausentes dos metadados |
| ACC-19 | Remoção repetida/crash intermediário | Não deixa referência órfã sem sinalização e recuperação |
| ACC-20 | Provider não tem OAuth | Accounts não impõe endpoints ou fluxo Google universal |

## Estado transitório e concorrência — testes adicionais

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| ACC-S01 | `status()` sem tarefas ativas | `active_operations=[]`, sem implicar conectividade ou ausência de contas |
| ACC-S02 | `add` antes de criar `AccountId` | Atividade `Adding` observável, `account_id=None` permitido e `request_id` preservado quando fornecido |
| ACC-S03 | `add` grava keyring e cadastro | Fases reais de persistência observáveis, sem expor token/handle |
| ACC-S04 | `remove` verifica vínculos | `Removing/CheckingAssociations`; `AccountInUse` encerra operação sem remover dados |
| ACC-S05 | `acquire_session` acessa Secret Service | `AcquiringSession/AccessingSecureStorage`; falha tipada sem segredos |
| ACC-S06 | `update_authorization` | `UpdatingAuthorization` e etapas efetivas; identidade divergente não substitui credenciais |
| ACC-S07 | Operação `Ok`, `Err` ou cancelada | Retirada do snapshot em todos os desfechos; resultado permanece no método |
| ACC-S08 | Várias contas com operações simultâneas | Estados independentes; sem mistura de IDs e erros |
| ACC-S09 | Consulta de `status()` durante I/O lento | Snapshot coerente e rápido, sem SQL, keyring ou HTTP |
| ACC-S10 | `get/list/update` local | Não gera tarefa monitorada fictícia nem executa autenticação remota |
| ACC-S11 | Falha do keyring em uma operação | Falha reportada; nenhuma marcação global fictícia de desconexão |
| ACC-S12 | Autorização no navegador ainda não entregue ao Accounts | `status()` não inventa fase de consentimento externo ao módulo |

Usar fakes controlados que mantenham operações em fases definidas, sem depender de `sleep` arbitrário. Os resultados de execução estão registrados abaixo.

## Invariantes transversais

- Uma conta é identificada por `(provider, provider_user_id)`; e-mail e nome são mutáveis.
- `AccountId` nunca é trocado durante renovação de credenciais.
- Nenhum token/segredo bruto nos retornos públicos, banco de cadastro, logs, diagnósticos e mensagens de erro.
- Métodos de consulta não iniciam autenticação remota.
- Accounts nunca chama Scanner, Inventário, Detector, Analisador ou Operador para modificar arquivos.
- Remoção de cadastro não apaga conteúdo local ou remoto nem revoga credenciais remotamente de forma implícita.
- Não criar banco SQLite sem requisito explícito; qualquer persistência concreta continua privada.
- Não criar submódulo `accounts/auth/` nem duplicar OAuth/OIDC existente em Providers.

## Evidências de execução — 2026-11-10

Ambiente: `org.gnome.Sdk//50`, extensão `org.freedesktop.Sdk.Extension.rust-stable` do manifesto, ambos do Flathub. Todas as ferramentas Rust foram executadas dentro do SDK, com o caminho da extensão em `PATH` e `CARGO_HOME` explícito. Não houve acesso a Google real nem ao Secret Service pessoal.

- `cargo test -p drivesync-engine`: **486 aprovados**, **1 ignorado por isolamento** (173 unitários aprovados, 1 teste público de Accounts, 312 testes de integração Providers; 0 doctests).
- A parcela Accounts da execução padrão contém **22 testes internos aprovados e 1 teste público aprovado**. Os testes agregam os cenários documentais; 32 IDs não significam 32 funções Rust.
- `python3 scripts/check-vault-isolated.py --binary <binário-de-testes-da-engine> --test isolated_accounts_vault_roundtrip`: **1 aprovado**, executando explicitamente o caso ignorado contra oo7 e D-Bus descartáveis. Exercita inclusão, sessão, troca de autorização, remoção e preservação de credencial com codificação inválida. Nenhum teste ignorado permanece sem execução específica nesta entrega.
- `cargo fmt --all --check` e `cargo clippy -p drivesync-engine --all-targets -- -D warnings`: aprovados.
- `cargo doc --workspace --no-deps --document-private-items`, `cargo test --workspace --doc`, documentação e doctests específicos da engine: aprovados; a suíte atual não contém exemplos executáveis de rustdoc.
- `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace`: **bloqueados na compilação do teste preexistente `tests/local_activity_diagnostic.rs`**, linhas 41 e 154, que usa `CARGO_BIN_EXE_drivesync-local-diagnostic` sem existir esse executável na branch. O problema também está presente na árvore original `origin/feat/accounts`; não foi alterado para fazer a suíte passar.

### Rastreabilidade dos contratos

| Cenários | Evidência principal |
| --- | --- |
| ACC-01, 02, 07, 08; S01, S10, S12 | Teste público `acc_01_02_07_08_s01_s10_s12_local_contracts`: abertura/reabertura exclusiva, vazio/filtro, NotFound e preservação de arquivo inválido |
| ACC-03, 04, 05, 09, 12, 14, 15 | `acc_03_04_05_09_12_14_15_18_lifecycle`, `acc_09_local_updates_do_not_call_dependencies`: identidade estável, mesmo e-mail sem fusão, alteração local e divergência sem substituição |
| ACC-06, 17 | `acc_06_17_20_reject_invalid_and_unsupported_without_effects`, testes de falhas do adaptador Google e keyring isolado |
| ACC-08 | `acc_08_multiple_provider_metadata_is_readable_without_authentication`: lista real com serviços distintos sem consultar credenciais ou Provider |
| ACC-10, 11, 19; S04 | `acc_10_11_19_remove_in_use_repeat_and_recover`, `acc_s04_s06_remove_and_reauthorization_phases`: conta em uso, repetição, falha de exclusão e retomada após reabertura; arquivo sentinela preservado |
| ACC-12, 13 | `acc_12_missing_and_mismatched_credentials_fail_closed`, `acc_13_refresh_preserves_refresh_token_and_identity`, `optional_refresh_token_allows_access_and_preserves_existing_renewal`: sessão opaca, credencial ausente/trocada e opcionalidade de refresh |
| ACC-16, 18; S03 | `acc_16_write_failure_leaves_recoverable_intent`, `acc_16_18_restart_reconciles_both_persistence_boundaries`, `acc_s03_metadata_failure_after_secure_write_recovers_after_restart`: falhas antes/depois de persistir secrets, limpeza antiga falha, nova instância e durabilidade |
| ACC-20 | Limite arquitetural: serviço desconhecido sem OAuth retorna UnsupportedProvider, metadados de outro serviço permanecem legíveis, nenhuma fábrica/trait universal foi adicionada a Providers. Não representa implementação de um segundo Provider não OAuth |
| S02, S05, S07, S08, S09, S11 | `acc_s02_s03_s07_s08_s09_s11_controlled_activity_and_cancellation`: barreiras determinísticas, correlação, tarefas simultâneas, cancelamento e keyring indisponível sem desconexão global |
| S06 | `acc_s04_s06_remove_and_reauthorization_phases`: operação de atualização identificada durante validação, retorno e limpeza do snapshot |
| Segurança e regressões adicionais | Cancelamento durante gravação segura, recuperação sem misturar IDs, credencial antiga ausente, intenção inconsistente e arquivo de staging com hard link |

O cliente Google real é exercitado por `google_adapter_uses_only_existing_provider_operations` e `google_adapter_maps_provider_failures_without_secret_diagnostics`, usando o servidor HTTP local já existente nos testes de Providers, sem modificar esse módulo.

### Limites

O contrato de Accounts é exercitado de forma independente. O proprietário definitivo de conexões deve integrar a guarda de remoção; seus vínculos reais e a integração GTK não foram exercitados. Não foi medida porcentagem de cobertura. Google real e aplicativo instalado não foram testados. A suíte global permanece bloqueada pelo diagnóstico descrito acima, portanto não há declaração de regressão global aprovada.


## Atualização após a integração da interface — 2026-11-10

O bloqueio global registrado na entrega original acima foi resolvido pelo novo recorte do front-end: o teste de diagnóstico local obsoleto foi removido com os componentes de observação/sincronização, sem alterar os testes de Accounts ou Providers. `cargo test --workspace --locked` e Clippy de todos os targets agora passaram. O caso de Secret Service isolado foi executado novamente e aprovado. Os números atuais e os limites da integração estão em [Connection — TESTS](../connection/TESTS.md#execução--2026-11-10).
