# Accounts — TESTS

## Objetivo

Validar o cadastro e ciclo de vida de contas, a associação de credenciais armazenadas de forma segura e a fronteira com os provedores. **Não testar OAuth real dentro de Accounts**: simular as respostas e sessões do Provider especializado. Validar integração real com keyring/Secret Service no ambiente Flatpak quando disponível.

## Fixtures

- Repositório de cadastro isolado com contas sintéticas de provedores distintos.
- Provider falso com identidade estável, autorização válida, expirado/reautorização, falha de rede, usuário alterado e credenciais revogadas.
- Armazenamento seguro isolado/falso sem exportação de tokens; testes de integração com backend Secret Service quando disponível.
- Conta com sincronizações vinculadas via interface simulada do proprietário de conexões (contrato ainda pendente).
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

## Invariantes transversais

- Uma conta é identificada por `(provider, provider_user_id)`; e-mail e nome são mutáveis.
- `AccountId` nunca é trocado durante renovação de credenciais.
- Nenhum token/segredo bruto nos retornos públicos, banco de cadastro, logs, diagnósticos e mensagens de erro.
- Métodos de consulta não iniciam autenticação remota.
- Accounts nunca chama Scanner, Inventário, Detector, Analisador ou Operador para modificar arquivos.
- Remoção de cadastro não apaga conteúdo local ou remoto nem revoga credenciais remotamente de forma implícita.
- Não criar banco SQLite sem requisito explícito; qualquer persistência concreta continua privada.
- Não criar submódulo `accounts/auth/` nem duplicar OAuth/OIDC existente em Providers.

## Execução

Exercitar inicialmente contratos com Providers/keyring simulados e persistência isolada; executar testes reais de Rust **dentro do Flatpak**. Testes que dependem de contrato ainda não fechado, como proprietário das conexões, devem ser `BLOCKED`, não presumidos aprovados.

**Estado:** plano TDD; nenhum teste Rust foi executado durante esta documentação.
