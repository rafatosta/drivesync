# Detector Storage

## Objetivo

Armazenar de forma durável as ocorrências detectadas e disponibilizá-las para entrega, confirmação e recuperação.

## Funcionamento

Persiste cada evento `File` integralmente, preservando identidade, dados observados e ordem de enfileiramento. A entrega utiliza reservas temporárias (leases): eventos reservados podem ser confirmados ou devolvidos à fila; reservas não concluídas podem ser recuperadas após interrupções.

O armazenamento deve preservar a integridade dos registros, tratar falhas explicitamente e evitar perda silenciosa de eventos. O Storage deriva `<raiz-local>/.drivesync/detector.sqlite3`; compartilha o diretório reservado com `inventory.sqlite3`, mas não acessa esse outro banco.

## Interface

- `open`: abre o armazenamento.
- `enqueue`: registra uma ocorrência.
- `claim_batch`: reserva eventos disponíveis.
- `acknowledge`: confirma eventos reservados.
- `release_batch`: libera reservas.
- `status`: informa o estado do armazenamento.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** fila unificada local/remota implementada e testada; ver INTERFACES §§ 7–9. Backup/restauração central permanece futuro. Não há compatibilidade nem migração de formatos antigos.

## Proteção e recuperação

O Storage permanece proprietário da fila, identidade, esquema e transações. Registra seu banco de forma restrita em [Configuration](../../configuration/README.md), responsável pelo **mecanismo técnico comum de snapshot/restauração SQLite**, sem ler ou alterar eventos, leases e cursores.

Corrupção comprovada exige notificação e bloqueio de acesso, solicitação de recuperação ao mecanismo central, fechamento/isolamento seguro de conexões e validação de identidade/schema após a substituição. Se não houver backup íntegro, a restauração falha **sem reconstruir automaticamente**. A eventual criação de banco vazio é uma decisão separada de inicialização, com esquema criado pelo próprio Storage e dados antigos preservados. A fila vazia não comprova ausência de eventos externos: o fluxo de sincronização deve reobservar o necessário antes de prosseguir. Falhas transitórias, identidade divergente e esquema incompatível não autorizam restauração.
