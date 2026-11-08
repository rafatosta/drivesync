# Agentes — Detector Storage

## Escopo

Implementar e testar **somente `detectors/storage/`**, independentemente de Detector, Local e Remoto concretos. Ler [INTERFACES.md](INTERFACES.md), [README.md](README.md), [TESTS.md](TESTS.md), [AGENTS da raiz](../../../../AGENTS.md) e [padrão de módulos](../../MODULE_STANDARD.md).

## Regras

- Storage implementa API interna **`EventStore`** e abstrai todo banco de dados; nenhum consumidor precisa conhecer localização, SQL ou tecnologia de persistência.
- Usar SQLite **real e temporário** nos testes de persistência, concorrência, corrupção e rollback; não testar apenas um mock do Storage.
- Separar eventos de detecção da fila de operações e do baseline do Inventário; não acessar nem alterar `inventory.sqlite3`.
- **Não** importar `LocalDetector`, `RemoteDetector`, Orquestrador nem implementações internas do Detector. Seu domínio é guardar e entregar `File`.
- `enqueue` confirma somente gravação durável; `claim_batch` não apaga; `acknowledge` exige decisão explícita; `release_batch` devolve; leases vencidos recuperam eventos.
- Suportar duplicidade controlada e erro de capacidade sem descartar eventos silenciosamente. Preservar isolamento entre sincronizações.
- API Storage **não** é reexportada publicamente pela engine: visibilidade de crate/módulo conforme necessário à composição interna.
- Não mexer nos demais módulos para facilitar testes. Usar contratos tipados e fakes de dependências.

## TDD e entrega

Escrever RED, implementar GREEN, REFACTOR e regredir por unidade funcional; criar commits locais atômicos. “Implemente o Storage” significa concluir todas as **seis operações** e testes aplicáveis, **sem exigir que Detector/Local/Remoto estejam implementados**. Se faltar contrato, registrar BLOCKED, não fingir sucesso.

Executar ferramentas Rust exclusivamente no Flatpak do manifesto do projeto; nunca no host. Comentários e commits em inglês. Não executar push/PR sem autorização explícita. Reportar contratos implementados, testes reais, comandos, resultados, commits e limitações. Este documento não comprova testes executados.

## Fidelidade ao objeto File

A unidade armazenada é `models::File` **inteiro**, nunca um evento reduzido. Persistir e devolver `File.id`, `File.event.kind/previous/current`, `File.scope`, `File.coverage`, `File.data` incluindo LocalFile/RemoteFile e seus estados de coleta. O Storage não deve conhecer campos individuais de LocalFile/RemoteFile e não deve reconstruir dados por caminhos/IDs.

Codec versionado, limite, retenção e segurança são pré-requisitos de implementação para dados sensíveis e payloads especializados. Identidade repetida com dados divergentes falha; mesmo ID/payload idêntico é idempotente. Claim/ack/lease não dependem do schema. `FileBatch` é propriedade do Detector pai. Não tocar Inventário nem `SyncItem`.
