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

O contrato `InventoryManager` documenta 17 operações agrupadas em abertura e estado da sincronização, consulta e atualização de itens, associação de identidades, confirmação do estado sincronizado, fila de operações e histórico de atividades.

As assinaturas, entradas, retornos e regras de consistência estão em [INTERFACES.md](INTERFACES.md).

## Estado operacional e execução

O Inventário é um serviço **de acesso transacional delimitado**, não um observador contínuo. A nova consulta `status()` expõe rapidamente a disponibilidade conhecida (`Ready`, `Recovering`, `Unavailable`) e o resultado da recuperação (`Normal`, `Restored`, `Rebuilt`), sem consultar SQLite a cada chamada. `open()` também retorna o resultado da recuperação quando bem-sucedido; se falhar antes de construir a instância, retorna erro tipado, sem status inventado.

Transações continuam síncronas e atômicas; chamadas potencialmente bloqueantes devem ocorrer fora do loop principal GTK. Não criar `start/pause/stop`, porcentagem de transação ou reporter de progresso para consultas curtas. Os estados `SyncOperation` representam **operações de sincronização persistidas**, não a execução do próprio Inventário.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; sua existência não comprova implementação Rust.

## Recuperação do Inventário

A proteção do SQLite pertence a [Storage](storage/INTERFACES.md), com política padrão própria independente do módulo opcional [Configuration](../configuration/README.md). Corrupção comprovada gera erro e aviso ao usuário, tentativa de backup automático e, se esta falhar, novo aviso e reconstrução **não destrutiva** do banco, preservando o anterior para diagnóstico. Um inventário reconstruído não possui baseline confiável; o Scanner observa o estado local e o Analisador processa os estados disponíveis sem alterar seu algoritmo. Dúvidas sem evidência suficiente são encaminhadas ao usuário por item. Não interpretar ausência de registros como exclusão de arquivos.
