# Orientações para agentes — Inventário

## Objetivo

Implementar e manter o armazenamento persistente do estado de uma pasta sincronizada. O inventário registra identidades `SyncItem`, vínculos locais/remotos, metadados observados, baseline confirmado, fila de operações e histórico de atividades.

## Contrato e estrutura

- [README.md](README.md): objetivo, funcionamento e componentes.
- [INTERFACES.md](INTERFACES.md): operações públicas, tipos, parâmetros, retornos, erros e invariantes.
- [TESTS.md](TESTS.md): cenários de conformidade e persistência.
- Os submódulos `items`, `metadata`, `baseline`, `operations`, `activity` e `storage` possuem seus próprios contratos.

A interface pública é `InventoryManager`, com as 17 operações documentadas. Cada método recebe uma requisição tipada e retorna `InventoryResult<T>`. A organização interna do código será definida conforme a implementação.

## Disponibilidade e recuperação

- `status()` consulta somente snapshot interno rápido, sem SQL nem esperar por transações ou restauração.
- Manter estado `Ready/Recovering/Unavailable` e resultado da última abertura `Normal/Restored/Rebuilt`; não fazer inferências de cobertura, exclusão ou baseline a partir desses estados.
- `open()` retorna erro se falhar antes de criar InventoryManager; quando abre com sucesso, sinaliza restauração/reconstrução para a inicialização.
- Storage é proprietário de backup/restauração e opera com valores padrão **sem Configuration**; preferências futuras são opcionais.
- Não introduzir gerenciador de jobs, callbacks de progresso de SQL ou `start/pause/stop`.
- Operações SQLite podem bloquear: não executá-las no loop GTK; preservar atomicidade e erros tipados.

## Persistência e integridade

- Manter um banco SQLite por pasta sincronizada, em `.drivesync/inventory.sqlite3`.
- Validar a identidade `SyncIdentity` na abertura, preservando isolamento entre sincronizações.
- Preservar bancos existentes em caso de indisponibilidade, corrupção ou esquema incompatível, retornando erros explícitos.
- Habilitar integridade referencial, chaves e restrições conforme o contrato.
- Utilizar transações atômicas, rollback, controle de revisão e idempotência nas operações aplicáveis.
- Persistir dados estruturados em formato relacional, observando a necessidade de serialização individual de campos específicos.
- Preservar valores opcionais e distinguir informações desconhecidas de ausência confirmada.
- Proteger informações sensíveis e evitar vazamentos em mensagens de erro e logs.

## Invariantes de domínio

- `SyncItemId` mantém a identidade lógica independentemente de nome ou caminho.
- `LocalFile` e `RemoteFile` representam estados observados; campos não informados permanecem desconhecidos.
- `PresenceState` distingue `Unknown`, `Present` e `AbsentConfirmed` conforme evidência.
- `SyncBaseline` registra somente um estado comprovadamente sincronizado.
- A fila `SyncOperation` preserva estado, tentativas e recuperação.
- `SyncActivity` mantém histórico associado a um item.
- A confirmação de sincronização atualiza, de maneira atômica, os registros relacionados previstos no contrato.

## Implementação e testes

Implementar por unidades funcionais coerentes, validando a assinatura e os cenários de cada método antes de avançar. Utilizar SQLite real temporário e dados sintéticos para testar abertura, integridade, relações, consultas, alterações, concorrência, reinício, rollback e falhas.

Executar as verificações Rust no ambiente Flatpak definido pelo projeto, conforme [AGENTS.md da raiz](../../../AGENTS.md). Registrar resultados realmente executados, limitações e lacunas da especificação.

As decisões técnicas ainda pendentes devem ser identificadas explicitamente, sem assumir comportamentos não documentados.
