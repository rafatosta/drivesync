# Inventário

## Objetivo

Persistir e disponibilizar o estado de cada pasta sincronizada, incluindo identidades de arquivos e diretórios, vínculos locais e remotos, metadados observados, último estado confirmado, operações pendentes e histórico de atividades.

## Funcionamento

Cada sincronização mantém seu inventário em um banco SQLite próprio, `.drivesync/inventory.sqlite3`, associado à pasta local. O `InventoryManager` disponibiliza a interface pública para consultar e atualizar registros, preservando a consistência por meio de transações.

Os dados são organizados em seis componentes:

| Componente | Dados e operações |
| --- | --- |
| [Items](items/README.md) | Identidades, vínculos, presença, estado e revisão de `SyncItem` |
| [Metadata](metadata/README.md) | Atributos locais e remotos observados |
| [Baseline](baseline/README.md) | Último estado sincronizado confirmado |
| [Operations](operations/README.md) | Fila durável e estados das operações |
| [Activity](activity/README.md) | Histórico de atividades por item |
| [Storage](storage/README.md) | Persistência SQLite e transações |

## Interface pública

O contrato `InventoryManager` documenta 16 operações agrupadas em abertura e estado da sincronização, consulta e atualização de itens, associação de identidades, confirmação do estado sincronizado, fila de operações e histórico de atividades.

As assinaturas, entradas, retornos e regras de consistência estão em [INTERFACES.md](INTERFACES.md).

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; sua existência não comprova implementação Rust.

## Recuperação do Inventário

A proteção do SQLite pertence a [Storage](storage/INTERFACES.md), coordenada pela política de [Configurations](../configuration/README.md). Corrupção comprovada gera erro e aviso ao usuário, tentativa de backup automático e, se esta falhar, novo aviso e reconstrução **não destrutiva** do banco, preservando o anterior para diagnóstico. Um inventário reconstruído não possui baseline confiável; o Scanner observa o estado local e o Analisador processa os estados disponíveis sem alterar seu algoritmo. Dúvidas sem evidência suficiente são encaminhadas ao usuário por item. Não interpretar ausência de registros como exclusão de arquivos.
