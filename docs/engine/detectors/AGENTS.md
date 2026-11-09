# Orientações para agentes — Detector

## Objetivo

Implementar a detecção unificada de alterações locais e remotas com armazenamento durável e entrega confirmável de eventos.

## Regras de implementação

- Seguir as assinaturas, tipos, estados e erros de [INTERFACES.md](INTERFACES.md).
- Preservar integralmente o objeto `File` de cada ocorrência; manter identidade e ordenação.
- Entregar eventos por `FileBatch`, com reservas, confirmação e reentrega segura após interrupções.
- Tratar observações parciais e falhas sem inferir exclusões confirmadas.
- Utilizar as interfaces dos componentes de observação e persistência documentados neste diretório.
- Acessar APIs de serviços externos somente por [Providers](../providers/README.md).
- Não assumir que o contrato documental já corresponde a código Rust implementado.

## Controle e observabilidade

- `start` inicia obrigatoriamente **Local e Remoto**; não criar API pública para iniciar somente uma origem.
- `pause/resume` apenas suspendem/liberam o consumo externo. Não parar watcher, polling ou escrita em Storage durante a pausa.
- `stop` encerra ambas as observações e pausa entrega, mas preserva dados persistidos. Não decidir no Detector se deverá ocorrer Scanner/reconciliação após a parada.
- `status` agrega estado operacional, entrega, filas e status/atividade/erros **individuais** de Local e Remoto, sem mascarar a origem saudável quando a outra falha.
- Se a entrega está pausada, `take_events` devolve erro tipado sem reservar lote. Pausa e reserva precisam ter ordenação concorrente segura.
- `acknowledge/release_events` seguem funcionais para lotes anteriormente reservados.
- Testar os comandos por contratos internos falsos, sem implementar dependência direta do Orquestrador.

## Validação

Executar os cenários de [TESTS.md](TESTS.md), incluindo eventos locais e remotos simultâneos, durabilidade, idempotência, falhas e recuperação. Utilizar dependências simuladas e dados isolados. Seguir as instruções gerais do [AGENTS.md da raiz](../../../AGENTS.md).
