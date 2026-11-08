# Detector Storage

## Objetivo

Armazenar de forma durável as ocorrências detectadas e disponibilizá-las para entrega, confirmação e recuperação.

## Funcionamento

Persiste cada evento `File` integralmente, preservando identidade, dados observados e ordem de enfileiramento. A entrega utiliza reservas temporárias (leases): eventos reservados podem ser confirmados ou devolvidos à fila; reservas não concluídas podem ser recuperadas após interrupções.

O armazenamento deve preservar a integridade dos registros, tratar falhas explicitamente e evitar perda silenciosa de eventos. A tecnologia e a localização física são detalhes de implementação do próprio componente.

## Interface

- `open`: abre o armazenamento.
- `enqueue`: registra uma ocorrência.
- `claim_batch`: reserva eventos disponíveis.
- `acknowledge`: confirma eventos reservados.
- `release_batch`: libera reservas.
- `status`: informa o estado do armazenamento.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.
