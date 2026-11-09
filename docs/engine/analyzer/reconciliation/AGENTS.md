# Orientações para agentes — Reconciliador

## Objetivo

Implementar o algoritmo privado de reconciliação que recebe **um `SyncItem`** e devolve uma **lista de operações tipadas**, além de divergências e requisitos de evidência, sem executar nenhuma operação.

## Leitura obrigatória

- [README.md](README.md): objetivo e funcionamento.
- [INTERFACES.md](INTERFACES.md): assinaturas, tipos, semântica de campos e invariantes.
- [TESTS.md](TESTS.md): matriz de cenários e critérios de conformidade.
- [Analisador](../INTERFACES.md): contrato público de processamento e modelo `OperationCommand`.
- [Conflitos](../conflicts/INTERFACES.md): contrato de avaliação independente das divergências.
- [Inventário](../../inventory/INTERFACES.md): `SyncItem`, `SyncBaseline`, revisão e estados de presença.
- [Operador](../../operations/INTERFACES.md): requisições necessárias para comandos completos.
- [AGENTS.md da raiz](../../../../AGENTS.md): convenções, segurança, commits e ambiente Flatpak.

## Regras de implementação

- Implementar a interface `reconcile(ReconciliationRequest)` com uma entrada e uma saída; não implementar processamento em lote.
- Manter algoritmo síncrono, determinístico, sem I/O e sem estado externo mutável.
- Comparar separadamente presença, conteúdo, nome, localização e metadados, com o baseline confirmado como referência histórica.
- Reutilizar tipos proprietários do Inventário, Models e Analisador; não criar cópias de `SyncItem`, `SyncBaseline`, `LocalFile`, `RemoteFile` ou `OperationCommand`.
- Devolver várias operações para um item quando necessário, preservando ordenação, IDs de etapa e dependências.
- Emitir comando somente com identidade, origem, destino e precondições suficientes; lacunas vão para `requirements`.
- Devolver divergências para avaliação independente pelo Analisador/Conflitos, sem chamar o módulo Conflitos.
- Nunca chamar o Operador, o Inventário, Providers ou qualquer mecanismo de persistência.
- Não inferir exclusão a partir de ausência de snapshot; respeitar `PresenceState`, escopo de observação e evidências.
- Não decidir unilateralmente qual versão descartar em conflito; preservar ambos os lados e exigir avaliação.
- Não inventar política de exclusão automática, equivalência de conteúdo nem capacidade do provedor.
- Se o plano depender de outro item ainda não disponível, registrar requisito em vez de processar esse outro item.
- Manter a implementação testável com entradas sintéticas e resultados inspecionáveis.

## Testes e entrega

Usar os testes `RC-01` a `RC-20` e `RC-S01` a `RC-S12` como critérios TDD, incluindo entradas inválidas, reobservação, conflitos, múltiplos comandos e dependências.

Testes Rust e documentação rustdoc devem ser executados dentro do Flatpak configurado pelo repositório. Declarar apenas verificações realmente realizadas e registrar bloqueios decorrentes de contratos ainda conceituais.

Limitar alterações desta etapa aos quatro arquivos da raiz de `docs/engine/analyzer/reconciliation/`. Reavaliar integração com o Analisador após concluir também os contratos internos; não modificar outros módulos sem solicitação explícita.

**Estado:** contrato documental inicial, sem implementação Rust comprovada.
