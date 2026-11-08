# Google Provider — TESTS

> Casos de contrato para `providers/google/` (composição e exposição da API). **Não executados.** Os testes dos 78 métodos REST/OAuth/transferência pertencem a [`api/TESTS.md`](api/TESTS.md), não são duplicados aqui.

## Objetivo

Comprovar que a fábrica Providers seleciona a implementação Google, fornece a interface técnica **existente** e associa corretamente uma conta autenticada à sessão, sem duplicar protocolo ou criar cliente HTTP fora da fábrica.

## Matriz de testes de composição

| ID | Cenário | Resultado obrigatório |
| --- | --- | --- |
| GPR-01 | `Providers.list()` com Google registrado | Exibe Google exatamente uma vez |
| GPR-02 | `Providers.get(google)` | Retorna `ProviderHandle::Google`, sem login implícito |
| GPR-03 | `get` para configuração OAuth | Disponibiliza API específica para `load_google_config` e `authorization_request` |
| GPR-04 | `Providers.open(account_id)` de conta Google válida | Retorna `ProviderSession::Google` vinculado à mesma `AccountId` |
| GPR-05 | Conta inexistente, revogada ou de outro serviço | Erro de seleção/autorização; nenhum fallback |
| GPR-06 | Duas contas Google simultâneas | Sessões isoladas, nenhum token cruzado |
| GPR-07 | Especialista Detector Google recebe a API | Acessa `changes_list` pela instância obtida, sem criar cliente paralelo |
| GPR-08 | Especialista converte `GoogleChange`/`GoogleFile` | Emite saída exigida por `RemoteDetector`, não expõe tipos Google no contrato pai |
| GPR-09 | Especialista Upload/Download recebe API Google | Usa métodos técnicos correspondentes sem redefinir interface Providers |
| GPR-10 | Renovação/revogação por Accounts/Auth | Handle obsoleto não permite usar sessão revogada |
| GPR-11 | Erro Google REST ou OAuth | Mantém erro técnico no cliente Google; módulo consumidor mapeia conforme seu contrato |
| GPR-12 | Análise estática de dependências | Apenas a fábrica compõe o cliente; especialistas não constroem HTTP externo independente |
| GPR-13 | `GoogleApiHandle` e `GoogleAccountApiHandle` | Não expõem credenciais em `Debug`, logs ou dados de domínio |
| GPR-14 | Revisão dos contratos `providers/google/api/` | Nenhum dos 78 métodos/tipos foi redefinido ou reduzido pela composição |

## Cobertura e isolamento

1. Usar fake da interface Google API já definida. Não exigir conta real.
2. Usar `AccountId` fictício e uma porta fake de autorização de `accounts/auth/`.
3. Testar `get` sem sessão e `open` com sessão vinculada separadamente.
4. Injetar erros de rede/permissão no fake para demonstrar que a fábrica não decide se um arquivo foi excluído.
5. Evitar testes de sincronização dentro desta pasta: Detector, Upload e Download possuem testes próprios dos especialistas.

## Condições de aceite

- API específica Google fornecida sem duplicação de seus métodos.
- A fábrica não introduz traits genéricas obrigatórias que reescrevam o protocolo.
- Não há credenciais vazadas, clientes externos paralelos, seleção de conta ambígua ou retorno de API de outro provedor.
- Casos dependentes de representação Rust do handle e do contexto autorizado são `BLOCKED` até essa decisão; nenhuma simulação equivale a teste executado.

[README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md) · [Google API — TESTS](api/TESTS.md).
