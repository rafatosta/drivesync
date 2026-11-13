# Detector Local

## Objetivo

Identificar alterações no sistema de arquivos de uma raiz monitorada e produzir eventos que descrevam as ocorrências observadas.

## Funcionamento

Combina notificações do sistema de arquivos com varreduras de verificação. Ao encontrar uma ocorrência, registra sua identidade, localização, tipo de alteração e metadados disponíveis. A observação deve distinguir resultados completos de leituras parciais ou inacessíveis, preservando incertezas em vez de presumir exclusões.

As ocorrências são emitidas individualmente em objetos `File`, com dados locais quando disponíveis.

## Interface

- `new`: prepara a observação de uma raiz.
- `start`: inicia a captura de mudanças.
- `scan_once`: executa uma varredura de verificação.
- `stop`: interrompe a observação.
- `status`: apresenta estado, **atividade atual**, contadores de rodada, cobertura e falhas.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.

## Controle pelo Detector pai

A pausa de **entrega ao Orquestrador** não pausa o observador Local: as alterações continuam sendo emitidas ao sink persistente do Detector. Apenas `stop` interrompe efetivamente a observação. O Local não avalia as consequências de períodos interrompidos nem chama Scanner por iniciativa própria; o Orquestrador decide eventuais verificações. A interface de status permite ao pai informar a condição local independentemente da condição remota.


## Fronteiras e estado da especificação

O Local não possui banco, fila durável ou baseline. A saída única é o sink individual do pai; a confirmação desse sink representa aceite, não sincronização concluída. Somente a pendência de entrega em trânsito e as pistas de observação são mantidas em memória, de forma limitada.

A coleta de metadados é injetada por uma porta estável e recebe cancelamento cooperativo. O detector transporta `LocalFile` integralmente, sem conhecer sua lista de atributos. Os tipos e campos compartilhados continuam pertencendo a Models e dependem da revisão documentada naquele módulo.

`scan_once` é a varredura de verificação que **emite eventos**. Não é o Scanner independente, que registra observações no Inventário. O scan de cobertura programado por `start` verifica a captura atual; não promete compensar o período parado ou reconstruir histórico.

Esta etapa define interfaces internas e cenários TDD, sem criar código Rust ou testes executáveis. As regras operacionais estão na seção 8 de INTERFACES e a rastreabilidade por método/porta está nas seções 9–13 de TESTS.
