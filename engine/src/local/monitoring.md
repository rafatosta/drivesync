# LOCAL-001 — proposta de monitoramento local

**Estado:** contrato e oráculos aprovados para implementação; descoberta de
diretórios por descritores implementada, sem observador integrado ou aceite
completo.
Requisito de produto: [LOCAL-001](../../../docs/SOFTWARE_SPECIFICATION.md#local-001--antecipação-por-eventos-locais).
Inventário e estabilização permanecem sob [SYNC-003](reconciliation.md).

## Responsabilidade e fronteira

Um observador pertencente ao serviço da aplicação entrega pistas de alteração
ao agendador por conta. O engine continua responsável por inventário completo,
identidade, classificação e execução autorizada. Nenhum evento é inventário,
journal durável, prova de escritor fechado ou autorização para exclusão.
Não há credenciais ou rede no observador. O serviço não depende de janela ou
navegação enquanto o processo da aplicação estiver ativo.

As entradas são identidade da conta, associação de raiz local/remota, geração
de configuração e cancelamento. Uma geração de configuração é independente da
geração atual de worker do controlador: iniciar um ciclo não invalida seus
monitores. A saída é `Changed(account, generation)` ou estado de cobertura
`Starting`, `Covered`, `Degraded(reason)` ou `Stopped`. `Covered` descreve
apenas monitores instalados na última revisão de cobertura, não promete que
o backend entrega todos os eventos. O estado é apresentado separadamente da
fase de sincronização, sem revelar credenciais ou caminhos em diagnóstico.

Razões tipadas de degradação distinguem raiz ausente/inacessível/substituída,
enumeração incompleta, monitor indisponível, limite de recursos e perda de
cobertura detectada. Cancelamento é resultado distinto de falha. A origem
não precisa oferecer um sinal de overflow: quando não houver essa capacidade,
perdas não detectadas continuam possíveis e são recuperadas pelo scan.

## Limites e política temporal proposta

- Até 4.096 diretórios monitorados por conta e 16.384 no processo; ao exceder
  qualquer limite, sinalizar degradação, cancelar monitores excedentes e
  manter o scan periódico. Não assumir que o sistema aceite esses números.
- Agrupar por relógio monotônico com prazo `min(último + 2 s, primeiro + 10 s)`.
  O timer do serviço atualmente observa prazos a cada segundo. O prazo é
  apenas de elegibilidade, não um limite de início quando há impedimento,
  trabalho em execução ou atraso de despacho.
- Reconstrução de cobertura falha aguarda 30, 60, 120, 240 e depois no mínimo
  300 segundos entre tentativas; somente uma reconstrução por conta pode
  ocorrer simultaneamente. Reconfiguração explícita inicia uma nova geração;
  encerramento cancela retries. Scan periódico independente continua ativo.
- Uma rajada é representada por estado limitado por conta, não por uma lista
  ilimitada de eventos. A descoberta tem limite de 100.000 entradas por
  tentativa, como o inventário; ultrapassá-lo é falha explícita, não sucesso
  parcial. Memória e descritores devem ser liberados no cancelamento.

Esses valores são decisões propostas do produto para revisão, não garantias do
GIO nem medições de desempenho. Alterá-los requer atualizar contrato e testes.

## Transições e algoritmo

1. Ao configurar uma raiz elegível, incrementar geração, cancelar observador
   anterior e iniciar `Starting`. Nenhum callback anterior altera a nova
   geração. Uma conta removida e o encerramento passam a `Stopped`; o worker
   antigo mantém sua guarda até concluir ou atingir ponto seguro.
2. Descobrir diretórios em worker cancelável, por caminhada sem seguir
   symlinks e com verificações de identidade por descritor. Instalar os
   monitores no contexto GIO adequado em lotes limitados. Não seguir caminho
   substituído entre descoberta e instalação: verificar a identidade e
   ancestralidade, cancelar e degradar se não puder comprovar a associação.
   Um backend que não permita essa comprovação não recebe cobertura declarada.
3. Após instalação, solicitar reconciliação e nova verificação de cobertura
   para a janela entre enumeração e instalação. Alterações estruturais
   solicitam redescoberta; monitores de diretórios removidos ou movidos para
   fora são cancelados, e diretórios novos ou movidos para dentro são
   percorridos recursivamente. Incerteza de cobertura mantém `Degraded`;
   a reconstrução completa e verificações bem-sucedidas retornam `Covered`.
4. O callback apenas registra sinais limitados e encaminha necessidade de
   atualizar cobertura. Nunca executa scan, hashing, banco, rede ou
   descoberta recursiva no contexto GTK. Eventos duplicados ou fora de ordem
   apenas solicitam nova observação do estado atual.
5. Na primeira pista elegível, registrar primeiro e último instante; nas
   seguintes, atualizar último sem deslocar o prazo além do máximo. Preservar
   prazo periódico anterior se for menor. Ao iniciar worker, consumir somente
   a rajada conhecida até o início; eventos posteriores criam outra rajada.
   Durante execução não iniciar outro worker. Ao concluir, conservar rajada
   posterior, inclusive em conclusão simultânea, e aplicá-la somente quando
   a política existente permitir.
6. Eventos não alteram pausa, bloqueio, autenticação, indisponibilidade de
   raiz, offline, retry ou backoff; não aprovam revisão em lote. Manter a
   necessidade pendente durante impedimento e reavaliá-la na retomada
   autorizada. O inventário periódico, reinício e retorno de conectividade
   não dependem de eventos para recuperar alterações.
7. Não suprimir eventos apenas por ocorrerem durante operação do DriveSync
   ou no mesmo caminho: podem coincidir com escrita externa. Arquivos
   reservados são excluídos conforme o contrato de inventário, sem ocultar
   seus pais ou renomes para nomes elegíveis. Descargas podem produzir um
   ciclo adicional; execução idempotente deve convergir sem reescrever
   continuamente a árvore. Os testes devem demonstrar que escritas internas
   terminam e não geram ciclo autoalimentado. Caso não seja demonstrável,
   interromper a integração e revisar o contrato antes de introduzir qualquer
   mecanismo de supressão.

Falha de observação não pode produzir inventário completo parcial nem
equivaler a ausência. O engine revalida raiz e identidade antes dos efeitos;
substituição, erro de acesso ou desmontagem nunca autorizam exclusão ou
ampliação de permissões. A sincronização somente prossegue quando suas
próprias verificações de segurança permitirem.

## Oráculos

| Cenário | Resultado exigido |
| --- | --- |
| Eventos a cada segundo de 0 a 9; sem impedimento | prazo é 10 s; se periódico vencer antes, mantê-lo |
| Evento durante worker e no limite da conclusão | nenhum worker simultâneo; ao menos uma avaliação posterior |
| Pausa, bloqueio, offline ou backoff e eventos | restrição e prazo proibido preservados; retomada reconcilia |
| Criação, edição, metadados, exclusão e rename na raiz e subárvores | conta correta recebe pista; editor com temporário não elimina estabilização |
| Diretório criado, movido para dentro ou fora durante descoberta | revisar cobertura, cancelar monitores obsoletos e reconciliar janela incerta |
| Conta removida ou raiz substituída durante callback/worker | rejeitar geração antiga, manter guarda de worker e preservar dados |
| Symlink, raiz inacessível, erro de instalação, limite ou perda detectada | degradação visível, nenhum inventário parcial completo e retry limitado |
| Eventos perdidos, aplicativo reiniciado ou offline | scan periódico ou retomada encontra alterações sem journal de eventos |
| Download e edição externa concorrente | edição permanece observável; testes demonstram convergência sem loop |

Testes unitários injetam relógio e backend de monitores, incluindo eventos
duplicados, fora de ordem e fluxo contínuo. Testes de integração usam
filesystem e SQLite temporários, provedor remoto simulado e falhas injetadas.
Homologação no Flatpak instalado com raiz descartável cobre portais, mounts,
perda de acesso e serviço em segundo plano. Google real exige consentimento
e conta de teste; execução no SDK não comprova integração real.

## Fundamentação e limitações

Referências consultadas em 2026-11-01:

- [GIO monitor_directory](https://docs.gtk.org/gio/method.File.monitor_directory.html):
  instalação pode falhar; mudanças feitas por hard links não são todas
  observáveis por monitores de diretório.
- [GIO changed](https://docs.gtk.org/gio/signal.FileMonitor.changed.html):
  eventos específicos de movimento dependem de flags e do backend.
- [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html):
  perda de eventos e corridas exigem verificação e reconstrução.

A compatibilidade exata das APIs do SDK, associação segura de monitor a
diretório validado e comportamento em Flatpak, portals e mounts são hipóteses
a verificar. Se não puderem ser comprovados, declarar degradação e não
afirmar cobertura ou aceite. As referências não comprovam a implementação.
