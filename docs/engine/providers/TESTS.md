# Providers — TESTS: fronteira única de APIs externas

**Status:** plano de testes para arquitetura-alvo, não evidência de execução. Fonte do contrato: [INTERFACES.md](INTERFACES.md). Não usar credenciais pessoais ou Google Drive real nos testes unitários.

## Estratégia

Testar (1) descoberta e composição por `ProviderId`, (2) autenticação e sessão por conta, (3) aquisição de dados e sua normalização, (4) comandos e transferências com erros/precondições, (5) **limites de dependência**. Usar fake do cliente de protocolo e keyring isolado nas integrações. Testes de contrato não precisam de uma conta Google real.

## Matriz de testes

| ID | Cenário | Garantia |
| --- | --- | --- |
| PRV-01 | Listagem de provedores | Apenas provedores realmente registrados; nenhum placeholder aparece como funcional |
| PRV-02 | Selecionar `ProviderId` desconhecido | `UnknownProvider`, nenhum HTTP |
| PRV-03 | Provedor não oferece capacidade solicitada | `UnsupportedCapability` explícito, sem sucesso fictício |
| PRV-04 | Abrir conta com `AccountId` de outro provedor | Rejeição sem sessão cruzada |
| PRV-05 | Duas contas do mesmo provedor | Identidade e credenciais rigorosamente isoladas |
| PRV-06 | Iniciar login e concluir callback válido | `ProviderIdentity` e material protegido retornados ao AuthManager |
| PRV-07 | Callback incorreto, state/PKCE inválido, token expirado | Falha sanitizada sem aceitar autenticação |
| PRV-08 | Renovação e revogação | Apenas protocolos realmente suportados; falhas não removem conta silenciosamente |
| PRV-09 | Requisição após revogação | Reautorização necessária, sem fallback a outra conta |
| PRV-10 | Leitura de metadados `GoogleFile → RemoteFile` | Identidade qualificada e `MetadataValue<T>` preservados |
| PRV-11 | Resposta parcial de API | Cobertura parcial explícita, sem `RemoteFileRef` inventado |
| PRV-12 | Remoção Google Changes / HTTP 403 ou 404 | `RemovedOrInaccessible` ou erro ambíguo, **não** exclusão comprovada |
| PRV-13 | Paginação e cursor opaco | Todas as páginas respeitadas; Providers **não** faz commit de checkpoint |
| PRV-14 | Nome/caminho repetido em dois recursos | IDs remotos não confundidos |
| PRV-15 | Renomear/mover com precondição obsoleta | `PreconditionFailed`, sem sobrescrever nem reupload implícito |
| PRV-16 | Lixeira/exclusão com recurso inacessível | Erro; operação destrutiva nunca disparada por observação |
| PRV-17 | Upload em chunks, erro de rede e retomada | Offset/bytes informados corretamente ou capacidade rejeitada |
| PRV-18 | Download parcial, cancelamento e documento nativo | Dados e erro de capacidade corretos, sem falso sucesso |
| PRV-19 | Rate limit, quota e offline | Erros tipados e `retry_after_seconds` quando conhecido |
| PRV-20 | Erro da API inclui headers/tokens/corpo privado | `ProviderFailure` expõe somente mensagem sanitizada |
| PRV-21 | Inspeção de dependências da engine | Nenhum consumidor importa `providers/google/api/`, HTTP ou `GoogleSession` |
| PRV-22 | `accounts/auth` com sessão e segredo | Keyring pertence a Accounts; token não chega ao Detector/Inventário |
| PRV-23 | Detector recebe dados e checkpoint não persistido | Sem dependência de commit dentro de Providers; Detector é proprietário |
| PRV-24 | Novo provedor com capacidades parciais | Mesmas portas genéricas, sem alterar `models/` nem `InventoryManager` |

## Regras de aceitação

- Testar falhas de rede, autenticação, paginação, retries e precondições com provedor simulado.
- Verificar que todas as chamadas remotas passam pela implementação de Providers e que consumidores só usam a API genérica.
- Validar que a autorização pode ser recusada/cancelada e que a conta não é cadastrada como autenticada.
- Validações de `RemoteFile` não fingem que campos não observados são `Known`.
- Não confundir teste unitário de protocolo com teste real de sincronização; integração real demanda autorização e escopo explícitos.

## Pendências antes de executar

As assinaturas async, formatos de token protegido, streams, `ProviderAccountPort`, limites de chunks, listas de capacidades e detalhes de mutações dependem de revisão. Casos correspondentes devem ser `BLOCKED`, nunca aprovados ficticiamente.

Referências: [README](README.md) · [INTERFACES](INTERFACES.md) · [AGENTS](AGENTS.md).
