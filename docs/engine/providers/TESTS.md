# Providers — TESTS

> Testes do **catálogo/fábrica** de APIs específicas. Documentação de testes propostos, não executados. Referência: [INTERFACES.md](INTERFACES.md).

## Escopo

Verificar seleção, retorno do tipo correto, isolamento por conta e controle de acesso. **Não** testar todos os métodos Google REST aqui: eles pertencem aos testes da [Google API](google/api/TESTS.md), enquanto os resultados exigidos de cada especialista pertencem aos testes do seu módulo pai.

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| PRV-01 | Listar provedores registrados | `list()` retorna apenas implementações disponíveis |
| PRV-02 | Selecionar Google | `get(google)` fornece `GoogleApiHandle`, sem autenticação implícita |
| PRV-03 | ID de provedor inexistente | `UnknownProvider`, sem acesso externo |
| PRV-04 | Placeholder `example/` não implementado | Não aparece em `list()` e não pode ser aberto |
| PRV-05 | Abrir `AccountId` Google autorizado | `ProviderSession::Google` com sessão dessa conta |
| PRV-06 | `AccountId` desconhecido ou sem autorização | Erro de seleção/autorização sem token alternativo |
| PRV-07 | Provedor da conta difere do handle solicitado | `AccountProviderMismatch`, sem fallback |
| PRV-08 | Duas contas Google diferentes | Sessões e credenciais isoladas |
| PRV-09 | Sessão revogada ou expirada | Falha tipada/reautorização via Accounts; sem sucesso fictício |
| PRV-10 | Especialista Google usa `changes_list` | Usa cliente obtido por Providers, sem HTTP avulso |
| PRV-11 | Especialista Google produz saída do módulo pai | `File`/resultado normalizado no limite do Detector, não `GoogleChange` público |
| PRV-12 | Outro serviço com API incompatível | Nova variante/handle sem exigir métodos Google ao outro serviço |
| PRV-13 | Erro técnico Google | Mantém significado na API específica; especialista converte para erro funcional do seu módulo |
| PRV-14 | Inspeção de dependências | Clientes remotos são criados apenas pela fábrica Providers |
| PRV-15 | Testar `get` durante fluxo OAuth | Entrega API específica; `accounts/auth` controla estado/credenciais |
| PRV-16 | Logout/reautorização com API obtida previamente | Contexto inválido não reutiliza autorização expirada nem atravessa contas |

## Limites de responsabilidade dos testes

- **Providers:** descoberta, fábrica, seleção tipada, isolamento e autorização.
- **`providers/google/api/`:** 59 métodos REST, OAuth/OIDC, semântica e erros de protocolo Google.
- **`accounts/auth/`:** ciclo de login, keyring, estado da conta e credenciais.
- **`detectors/remote/google/`:** estratégia de polling, conversão `GoogleChange/GoogleFile` para `File` e checkpoint seguro.
- **Upload/Download Google:** transferências e retorno conforme interface dos módulos correspondentes.

Não usar Google Drive real ou credenciais pessoais nos testes unitários. Usar fakes e, na integração, contexto de conta isolado. A ausência de contrato concreto dos operadores permanece `BLOCKED`, não é resultado `PASS`.

## Critérios de aceite

O catálogo retorna API correta e não promete capacidades genéricas; os especialistas usam o cliente recebido, mas seus métodos continuam específicos ao provedor; módulos pais preservam contrato único. Nenhum consumidor cria HTTP externo por fora da fábrica, acessa segredo de outra conta ou incorpora métodos de protocolo na interface do módulo pai.
