# Fechamento da etapa Detector

## Escopo entregue

A API pública `drivesync_engine::detectors::Detector` compõe Local e Remoto privadamente, escolhe o especialista remoto pelo provedor de Accounts e usa uma única fila SQLite por conta, SyncId e par de raízes. Google é o especialista remoto implementado. Provedores sem registro falham explicitamente.

Estão implementados `new`, `start`, `pause`, `resume`, `stop`, `request_scan`, `status`, `take_events`, `acknowledge` e `release_events`. O consumidor não instancia filhos. A fachada não constrói Inventário, Scanner, Analisador ou operações de sincronização.

- `start` solicita ambas as origens, inclusive quando uma falha; a entrega inicial é gravada antes do início das fontes.
- `pause/resume` controlam somente o consumo. Captura, polling e gravação continuam. Pausa e reserva são serializadas pela mesma transação de controle da fila.
- `stop` pausa entrega, solicita parada de ambas e preserva eventos/checkpoint. StopNow cancela HTTP em andamento; DrainInFlight permite terminar a rodada remota limitada já iniciada. Ambos aguardam callbacks locais em trânsito. Hints locais ainda não aceitos não são declarados duráveis; reinício produz reavaliação de escopo, e a coordenação decide a política após o intervalo parado.
- `status` lê caches, sem HTTP, filesystem ou SQLite. Preserva saúde e erro tipado de cada origem; uma falha remota não transforma o Local saudável em falha.
- A fila fornece lotes mistos de File completos em ordem global de commit, não em ordem causal ou por timestamp. Um lease e uma transação confirmam/liberam todo o lote, incluindo ambas as origens. Os horários do lote delimitam os instantes observados recebidos.
- Eventos não confirmados reaparecem depois da expiração do lease, inclusive após encerramento abrupto. IDs e payloads são preservados; não se promete exatamente uma vez para o consumidor.
- O aviso opcional de disponibilidade ocorre somente depois do commit e não confirma nem entrega eventos. Falha do aviso não desfaz persistência. O sink deve sinalizar de forma leve, sem reentrar em controles bloqueantes do próprio Detector.

## Reobservação e Scanner

`request_scan` solicita reobservação aos produtores; não invoca o Scanner Local nem escreve no Inventário. Escopos locais são reobservados com metadados e incerteza explícita; All inclui verificação da cobertura dos watches e reavaliação da raiz. A implementação Google pode ampliar um escopo remoto para uma rodada de recuperação da raiz configurada, preservando a segurança do cursor. Aceitação não significa conclusão nem prova de ausência.

O Scanner Local permanece responsável pelo levantamento local via Inventário. Análise, baseline, conflitos e execução de comandos permanecem fora do Detector.

## Contadores

O índice durável registra a origem real de emissão, inclusive avisos sem payload. `QueueStats.detected` representa todas as observações; `initial_observations` separa CaptureStarted, incluindo bootstrap após recuperação; `changes_detected` exclui essas observações iniciais. A página Developer apresenta eventos posteriores no indicador principal e observações iniciais/de recuperação nos detalhes. Avisos e incertezas continuam identificados; os contadores não provam sincronização.

## Formato em desenvolvimento

Não há compatibilidade ou migração de bancos antigos. A criação instala o esquema atual completo (versão SQLite 2); abertura de formato diferente falha explicitamente, sem backfill, conversão ou reconstrução automática. Reabrir o mesmo formato continua recuperando eventos e leases após interrupções. O codec de File é validado estritamente.

`PersistentDetector` e os caminhos antigos de composição do Orquestrador foram removidos. Os especialistas Local/Remoto mantêm testes independentes, mas a aplicação compõe apenas a fachada `Detector`.

## Validação e limites

A suíte independente do Orquestrador usa portas de origem falsas e SQLite real para verificar início de ambas, falha parcial, pausa, lotes mistos, ack idempotente, release, reentrega, concorrência, aviso pós-commit, consulta de status durante início lento e contagem de bootstrap. O teste consumidor externo usa captura nativa real, conta sintética que exige reautorização, reobservação local e um processo filho encerrado abruptamente.

O Developer usa `Orchestrator::observe_detector` apenas como adaptador/consumidor terminal diagnóstico; não é a validação do Orquestrador de sincronização completo. O play/pause controla a entrega de ambas as origens da conta selecionada. A fila pública não confirma sozinha.

Não foram repetidas mutações no Drive real nesta etapa. Autenticação real, portal/pacote instalado, permissões e expiração reais do Google permanecem validações de integração/ambiente. Backup/restauração via Configuration é uma dependência futura já registrada no contrato de Storage, não uma implementação simulada desta etapa. Permanecem os limites documentados de capacidade, tamanho de payload e árvore/paginação; falhas são explícitas, sem limpeza ou reconstrução silenciosa.
