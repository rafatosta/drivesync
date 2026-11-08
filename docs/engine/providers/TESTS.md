# Providers — TESTS

> Plano de testes do **catálogo e entrega de APIs específicas**. Nenhum teste foi executado nesta etapa documental.

## Escopo

A cobertura de Providers limita-se a listagem, seleção, inicialização e fornecimento da **interface correta**. Testes dos métodos de cada serviço pertencem à pasta da API correspondente: [Google API — TESTS](google/api/TESTS.md).

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| PRV-01 | Catálogo com Google implementado | `list` retorna Google uma vez |
| PRV-02 | Selecionar `ProviderId` Google | `get` fornece a API Google tipada |
| PRV-03 | Selecionar provedor desconhecido | `UnknownProvider` explícito |
| PRV-04 | Provedor documentado mas não implementado | Não consta de `list` nem é anunciado como funcional |
| PRV-05 | Inicialização do cliente específico falha | Erro de inicialização sem interface fictícia |
| PRV-06 | Selecionar repetidamente o mesmo provedor | A interface continua corretamente tipada e utilizável |
| PRV-07 | Dois provedores com APIs distintas | Nenhuma imposição de métodos comuns entre os clientes |
| PRV-08 | Métodos técnicos do Google | A API fornecida referencia o contrato existente, sem implementação duplicada |
| PRV-09 | API exige parâmetros de autenticação | A fábrica não realiza login nem persistência automática |
| PRV-10 | Falha de configuração de um provedor | Erro isolado sem corromper catálogo de outros provedores |
| PRV-11 | Revisão de dependências | Não existe segundo cliente de protocolo externo concorrendo com a implementação do provedor |
| PRV-12 | Cadastro de novo provedor | Nova interface específica pode ser registrada sem alterar métodos das APIs existentes |

## Fora do escopo

Não testar aqui armazenamento de contas, políticas de sessão, decisões funcionais, tratamento de eventos de sincronização ou interpretação dos resultados por outras partes do aplicativo. Protocolos OAuth, Drive REST e transferências Google devem ser testados no contrato especializado [`google/api/TESTS.md`](google/api/TESTS.md).

## Critérios

- Sem endpoints universais inventados.
- Sem exposição de provedores inexistentes.
- Sem funções Google duplicadas na fábrica.
- Nenhum teste recebe status `PASS` sem execução real; cenários dependentes de representação Rust definitiva permanecem `BLOCKED`.
