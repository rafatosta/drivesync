# Contrato — download

## Objetivo
Baixar em staging e validar antes de instalar.

## Utilizado por
queue,coordinator.

## Entradas
Dados conceituais tipados, IDs estáveis, contexto de operação, versões e pré-condições quando aplicáveis.

## Saídas
Resultado tipado ou erro recuperável, definitivo, conflitante ou ambíguo.

## Limites
Não realizar atividades de outros módulos; não depender de toolkit gráfico ou linguagem específica.

## Invariantes
Arquivo incompleto não substitui arquivo válido. Respeitar `docs/invariants.md`.

## Qualidade
Especificar idempotência, efeitos colaterais, concorrência, cancelamento, erro e recuperação antes da implementação.

The installed Flatpak runtime may expose `/run/user/<uid>/doc` as the fixed
`../../flatpak/doc` mount alias. Resolve only this platform alias for root access
inside Flatpak, keeping persisted scope strings unchanged. All components beneath
the mount, root identity, parent identity, file identity and hashes still require
the existing descriptor-relative no-follow checks. Other symlink targets fail
closed; never canonicalize arbitrary root/descendant symlinks as a workaround.


## Cópias de conflito — SYNC-010

A execução de divergência binária no mesmo caminho pode preservar a versão
remota em cópia adicional sem substituir o original local. O journal separado
não confirma instalação no caminho original nem sincronização. Relatório e
recuperação seguem [o contrato de preservação](../conflicts/preservation.md).
Nomes reservados de backup não entram em inventário/upload. Impossibilidade ou
prova incompleta mantém a pendência e os artefatos existentes.
