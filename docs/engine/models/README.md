# Models — domínio compartilhado da engine

## Objetivo

`models/` define **tipos de dados genéricos e identidades compartilhadas** para os módulos da engine. Seu produto é a representação de informações; **não** é um serviço, repositório, DAO, fábrica de provedores, operador, serializador de API ou controlador de sincronização.

**Decisão de arquitetura:** este módulo **não exige trait de serviço** nem uma API de métodos `get_file`, `save`, `scan`, `upload` ou `download`. A interface pública consiste em **structs, enums, identificadores tipados e invariantes de validação** descritos em [INTERFACES.md](INTERFACES.md). Construtores de validação e acessores puros podem ser adicionados para proteger invariantes.

## Estrutura documental

```text
docs/engine/models/
├── README.md          # responsabilidades e organização
├── INTERFACES.md      # contratos de dados e limites de visibilidade
├── TESTS.md           # testes puros e de invariantes
└── AGENTS.md          # regras para implementação isolada
```

### Organização conceitual futura em Rust

```text
engine/src/models/
├── mod.rs
├── identity.rs        # AccountId, ProviderId, SyncId e referências tipadas
├── remote_file.rs     # RemoteFile e metadados remotos normalizados
├── local_file.rs      # LocalFile e metadados locais normalizados
├── metadata.rs        # tipos compartilhados de precisão/presença/revisão
└── provider_data.rs   # envelope opaco de dados próprios do provedor
```

A árvore é **proposta**, não descrição dos arquivos Rust existentes. Atualmente `engine/src/models.rs` contém modelos públicos de conta; `engine/src/local_state/models.rs` possui tipos de estado próprios. **Não mover, duplicar ou redefinir esses tipos automaticamente.** `models/` pode ser implementado inicialmente como um único arquivo e dividido somente conforme a necessidade de manutenção.

## Propriedade dos dados

| Tipo ou conceito | Proprietário | Papel |
| --- | --- | --- |
| `RemoteFile` | `models/` | Arquivo/diretório remoto com identidade, atributos normalizados e dados privados do provedor |
| `RemoteFileRef`, `ProviderId` e identidades compartilhadas | `models/` ou módulo proprietário já existente, por reexportação explícita | Evitar confusão entre serviços, contas e itens |
| `LocalFile` | `models/` | Fotografia/observação normalizada do filesystem, sem executar I/O |
| `ProviderFileData` | `models/` | Envelope opaco que **carrega dados específicos** sem interpretá-los |
| `GoogleFile` | [`providers/google/api/`](../providers/google/api/README.md) | Representação concreta da resposta Google; não é importada pelo domínio comum |
| `SyncItem`, baseline, vínculos e estados de presença | [`inventory/`](../inventory/README.md) | Estado gerenciado da sincronização; **não** pertence a `models/` nesta etapa |
| `DetectorEvent`, batches, leases | [`detectors/`](../detectors/README.md) | Mensageria de detecção, não modelos universais |
| Sessões, tokens, contas cadastradas | `accounts/` e `auth/` | Credenciais e ciclo de vida de autenticação não são dados de arquivo |

## RemoteFile com conteúdo específico

`RemoteFile` expõe dados comuns suficientes para catálogo, Inventário e Analisador (identidade, nome, tipo, informação observada e referências de versão). **Também possui um atributo de dados específicos do provedor**, semelhante à ideia do pacote com conteúdo especializado.

Esse atributo é **opaco para consumidores genéricos** e identificado por provedor/formato/versão. Um adaptador Google conhece `GoogleFile` e pode criar/interpretar o payload Google; um adaptador OneDrive faz o mesmo com seus tipos, sem acrescentar variantes específicas em `models/` a cada novo serviço. Não manter `GoogleFile` como dependência direta do módulo genérico.

Não obrigar todos os provedores a preencher os mesmos atributos. Metadados não solicitados, desconhecidos e ausentes com evidência são situações diferentes; uma conversão incompleta **não** pode inventar atributos ou concluir exclusões.

## Dependências e direção

```text
providers/google/api/   --> (modelos específicos Google)
detectors/remote/google --> models/ + Google API (adapta observações)
inventory/              --> models/ (usa RemoteFile/LocalFile)
analyzer/               --> models/ e API do Inventário
operations/...          --> models/ e especialistas necessários
models/                 --> biblioteca padrão + tipos/utilitários puros
```

`models/` **não importa** `inventory/`, `detectors/`, `providers/`, `accounts/auth/` nem `operations/`. A seleção do especialista cabe à conta e ao módulo funcional, nunca ao próprio `RemoteFile`.

## Critérios de segurança

1. Identidade remota deve ser interpretada no contexto de conta/provedor; IDs brutos não são necessariamente globais.
2. Nenhum `RemoteFile` contém credenciais, token, conexão, cliente HTTP ou handle de banco.
3. Metadados parciais não equivalem a ausência comprovada.
4. `LocalFile` não lê o filesystem: é um valor produzido por um observador.
5. `RemoteFile` não executa API do provedor nem transfere bytes.
6. Dados opacos específicos de provedor possuem contrato de identificação e limites; consumidores genéricos não fazem `match` em Google/OneDrive.
7. Nenhuma struct compartilhada executa decisões de conflito, confirmação de sincronização ou operações destrutivas.

## Escopo e estado

Esta documentação estabelece **contratos-alvo de dados** para implementação futura. O [Inventário](../inventory/INTERFACES.md) ainda utiliza `GoogleFile` diretamente em seu contrato anterior e **não foi migrado**; essa divergência está registrada e deve ser resolvida em alteração específica do Inventário, após a estabilização dos modelos. O documento não declara que os tipos abaixo já existam em código Rust.

[INTERFACES.md](INTERFACES.md) · [TESTS.md](TESTS.md) · [AGENTS.md](AGENTS.md)
