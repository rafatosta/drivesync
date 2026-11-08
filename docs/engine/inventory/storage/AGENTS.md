# Inventário / storage — instruções para agentes

> Aplicar junto de [inventory/AGENTS.md](../AGENTS.md) e deste submódulo: [README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md). Este é **um contrato documental proposto**, não uma alegação de implementação.

## Missão e limite de edição

Única infraestrutura de acesso ao SQLite de cada sincronização, incluindo abertura segura, integridade e transações.

**Pode atuar futuramente:** apenas nos arquivos Rust do submódulo `inventory/storage/` e testes pertinentes, **quando a implementação for explicitamente solicitada**. Nesta fase, não modificar código compilado, dados de usuário ou interfaces de outros módulos.

**Interfaces recebidas:** `OpenInventoryRequest`, `SyncIdentity`, closures/operações internas sobre `InventoryTx<'_>` e estruturas dos repositórios.

**Dependências aceitas:** Tipos públicos do Inventário apenas para identidade/erros; repositórios recebem `InventoryTx`, sem conexão independente.

**Proibições:** Não interpreta `SyncItem`, não decide operações, não faz rede ou detecção, não acessa `detector.sqlite3` e não cria tabela `remote_events`/`remote_cursor`.

## Regras contratuais

- `open` — validar identidade e existência sem recriar banco inválido.
- `read` — oferecer acesso de leitura sob contexto privado.
- `transaction` — executar trabalho multi-repositório com commit/rollback atômico.
- Manter esquema, FK, índices e tradução de falhas SQL para `InventoryFailure`.
- Manter a única API pública em `InventoryManager` (16 métodos). As assinaturas internas de `INTERFACES.md` são propostas; validar ownership/lifetimes de Rust antes de transformar em código.
- O armazenamento físico só pode ser manipulado pelo contexto interno de `inventory/storage/`. Em operação multi-repositório, **um único commit/rollback**.
- Não criar compatibilidade retroativa, banco de eventos remotos, checkpoint do Detector ou integração direta com provedor neste submódulo.
- Tratar falha como erro tipado; não alegar sucesso ou eliminar metadados/observações quando a prova for incompleta.
- Não mudar `models/`, `detectors/`, `SyncItem` ou os 16 métodos públicos para acomodar uma implementação parcial sem revisão arquitetural explícita.

## Desenvolvimento e TDD

1. Consultar [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md), identificando lacunas.
2. Implementar pequeno conjunto coeso por vez, sem stubs que retornem sucesso fictício.
3. Executar os testes de persistência em SQLite isolado e simular falhas/reabertura.
4. Verificar compatibilidade com `InventoryManager` e transações compartilhadas; rejeitar dependências circulares.
5. Informar quais cenários foram executados, aprovados, falharam ou continuam bloqueados.

## Bloqueios que não podem ser inventados

DDL final, WAL/locking, timeouts, provisionamento com raiz inacessível, índices, conexões e teste de crash em subprocesso.

Caso algum ponto seja essencial, registrar `BLOCKED` e pedir decisão técnica antes de implementar. O planejamento não autoriza destruir arquivos locais/remotos nem trabalhar com credenciais reais.
