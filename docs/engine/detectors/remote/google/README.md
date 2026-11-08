# Detector Remoto Google

## Objetivo

Detectar alterações em arquivos e diretórios do Google Drive e representá-las como eventos de observação remota.

## Funcionamento

Utiliza exclusivamente as operações disponibilizadas pela interface [Providers Google](../../../providers/google/INTERFACES.md) para consultar metadados e mudanças. Interpreta paginação, cursores, identidades e respostas parciais. Converte os dados observados em eventos `File` com `RemoteFile` quando há identidade suficiente.

A observação preserva IDs de recursos, contexto da conta e metadados efetivamente retornados. Alterações sem identificação confiável, perdas de acesso e respostas incompletas geram indicação de reavaliação, sem presumir exclusão.

## Interface

Implementa as operações de observação definidas para o [Detector Remoto](../INTERFACES.md): `new`, `start`, `poll_once`, `stop` e `status`.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.
