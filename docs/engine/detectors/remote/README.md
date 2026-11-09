# Detector Remoto

## Objetivo

Observar alterações em recursos remotos e emitir eventos individuais que representem as ocorrências identificadas.

## Funcionamento

O detector remoto executa observações iniciais e incrementais, conforme as capacidades do serviço. O especialista correspondente consulta as operações disponibilizadas em [Providers](../../providers/README.md), interpreta páginas de resultados, mantém o checkpoint de observação e transforma as informações obtidas em eventos `File`.

O checkpoint avança somente depois que os eventos correspondentes têm entrega durável confirmada. Respostas parciais, cursores inválidos e indisponibilidade de acesso devem ser representados sem inferências de exclusão.

## Interface

- `new`: prepara a observação.
- `start`: inicia ciclos de observação.
- `poll_once`: executa uma rodada de consulta.
- `stop`: interrompe a observação.
- `status`: apresenta estado, **atividade atual**, contadores da rodada e erros de observação.

## Especialistas

- [Google](google/README.md): detecção especializada de alterações do Google Drive.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.

## Controle pelo Detector pai

A pausa de **entrega ao Orquestrador** não pausa o polling remoto: eventos continuam sendo emitidos ao sink persistente do Detector e o checkpoint só avança após persistência durável. Apenas `stop` interrompe a observação de fato, preservando o checkpoint. O Orquestrador decide eventuais verificações após intervalo sem detecção; o Remoto não assume reconciliação ou execução de Scanner. O pai deve expor a condição remota separadamente da local.
