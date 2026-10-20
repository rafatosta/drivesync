# Contrato UI: processo único

A UI GNOME consome a fachada pública `DriveSyncEngine` **por chamadas Rust dentro do mesmo processo**. Não usa D-Bus para falar com o motor.

## Estados de janela
`VISIBLE`, `HIDDEN`; o motor possui estado próprio e independente de visibilidade.

## Comandos conceituais
`ShowWindow` reexibe a instância atual; `HideWindow` não altera tarefas; `QuitApplication` solicita desligamento recuperável; `PauseSync` e `ResumeSync` são comandos distintos.

## Invariantes
Fechar X ≠ Sair; fechar X ≠ Pausar. Nova ativação não cria motor duplicado. Interface não faz transferências nem interage diretamente com tokens/SQLite.

## Erros
Inicialização background não autorizada, falha do motor, saída interrompida: informar estado real sem inventar sincronização ativa.
