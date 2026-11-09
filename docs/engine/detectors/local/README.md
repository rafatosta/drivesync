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
