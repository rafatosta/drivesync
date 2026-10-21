# Módulo accounts

## Objetivo
Manter contas cadastradas e associar cada conta a perfis de sincronização.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
coordinator, ui.

## Saídas e compromissos
Metadados da conta e vínculos com diretórios.

## Limites: o que NÃO faz
Não guardar tokens em texto claro.

## Algoritmos previstos
ACCT-001 cadastrar conta; ACCT-002 remover vínculo.

## Erros e cenários a verificar
Contas duplicadas; troca de perfil; credenciais ausentes.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** identidade estável, perfis persistentes e restauração na abertura
implementados em [ACCT-003](algorithms.md#acct-003--persistir-e-restaurar-perfis-na-abertura).
Raízes e desvinculação permanecem pendentes; tokens ficam no auth/Secret Service.
