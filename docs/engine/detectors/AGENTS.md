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


## Publicação parcial autorizada — coleta local

- O usuário solicitou acesso público somente à coleta local já implementada. A API atual é `drivesync_engine::detectors::LocalMetadataCollector` (new/scope/collect) e os tipos relacionados, conforme INTERFACES § 8.
- Preservar os submódulos, a trait de composição e hooks/backend como privados. Não expor injeção de syscall ou descritores. O controle local foi autorizado posteriormente, conforme INTERFACES § 9.
- A implementação é reexportada com nome público e métodos inerentes; a trait interna permanece para composição futura. Não duplicar DTOs ou reimplementar coleta no módulo pai.
- Testes de consumidor externo devem importar somente detectors/models e exercitar a API pública; doctests compile_fail verificam os limites de visibilidade.
- Esta publicação não autoriza implementar métodos provisórios da fachada unificada, modelos remotos ou fila. A fachada unificada continua pendente; LocalDetector::new/start/stop/status foram autorizados posteriormente e estão implementados.
