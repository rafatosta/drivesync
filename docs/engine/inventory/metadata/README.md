# Inventory — Metadata

## Objetivo

Persistir e reconstruir os metadados locais e remotos associados aos itens do inventário.

## Funcionamento

Armazena os atributos selecionados de `LocalFile` e `RemoteFile`, preservando valores conhecidos, campos opcionais e estados de coleta. A leitura reconstitui apenas informações efetivamente registradas.

## Interface

- `LocalMetadataRepository.apply/load`: grava e recupera metadados locais.
- `RemoteMetadataRepository.apply/load`: grava e recupera metadados remotos.

**Entradas principais:** `SyncItemId`, `LocalFile`, `RemoteFile`, `MetadataValue<T>` e `ProviderFileData`, quando aplicável.

**Resultados:** metadados reconstruídos ou falhas tipadas.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
