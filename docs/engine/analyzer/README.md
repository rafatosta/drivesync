# Analisador

## Objetivo

Processar **um `SyncItem` por chamada**, determinar as transformações necessárias entre estado local, remoto e último baseline confirmado, coordenar a avaliação de conflitos e executar os comandos autorizados por meio da API pública do [Operador](../operations/INTERFACES.md).

O resultado de cada chamada é conclusivo para aquela tentativa: **concluído**, **pendente de resolução** ou **falha**, com evidências sobre as operações que tenham ocorrido. O Analisador encerra a chamada quando precisa de uma decisão externa; não fica bloqueado aguardando o usuário.

## Modelo de funcionamento

1. Recebe `AnalyzeRequest` com um `SyncItem` de [Inventário](../inventory/INTERFACES.md) e, opcionalmente, uma resolução para uma pendência anterior.
2. Valida identidade, revisão e suficiência dos estados observados. O baseline é o **último estado sincronizado confirmado**, não uma suposição de equivalência.
3. Coordena dois algoritmos privados, independentes:
   - **Reconciliação** compara os estados e devolve uma **lista ordenada de operações propostas**, incluindo dependências quando necessárias.
   - **Conflitos** avalia uma divergência contextualizada entre as versões envolvidas e retorna uma conclusão, uma possível resolução automática ou uma decisão externa necessária.
4. Valida os comandos propostos e executa-os sequencialmente por meio do [Operador](../operations/INTERFACES.md), conservando identidades, precondições e evidências.
5. Confirma no Inventário apenas estados efetivamente verificados, preservando histórico e recuperação de efeitos parciais.
6. Retorna `AnalysisOutcome` ao chamador.

Nenhum dos dois algoritmos privados chama o Operador. **Somente o Analisador coordena e aciona a execução.** O Orquestrador entrega itens individuais e recebe o desfecho; não interpreta nem executa comandos.

## Fluxo

```mermaid
flowchart TD
  A["Orquestrador: um SyncItem"] --> B["Analyzer.analyze"]
  B --> V["Validar estado e resolução"]
  V --> R["Reconciliador: plano de operações"]
  R --> C{"Divergência a avaliar?"}
  C -->|Sim| F["Conflitos: avaliação isolada"]
  F --> D{"Decisão segura?"}
  D -->|Não| P["Pending: decisão/reobservação"]
  D -->|Sim| E["Analisador: validar e executar comandos"]
  C -->|Não| E
  E --> O["Operador"]
  O --> I["Inventário: confirmar efeitos verificados"]
  I --> T["Completed ou Failed com histórico"]
```

## Interface pública

- `Analyzer::new(AnalyzerConfig) -> AnalyzerResult<Analyzer>`: configura acesso aos contratos dependentes.
- `Analyzer::analyze(AnalyzeRequest) -> AnalyzerResult<AnalysisOutcome>`: processa exatamente um `SyncItem`, com resolução opcional.

`AnalysisOutcome` oferece `Completed`, `Pending` e `Failed`. `Pending` devolve um objeto autocontido para registrar a pendência e apresentar alternativas, sem suspender outros itens. `Failed` preserva os efeitos já ocorridos. A mesma função `analyze` retoma um item após decisão externa, validando novamente o estado.

As assinaturas, campos, tipos, regras de falha e integração estão em [INTERFACES.md](INTERFACES.md).

## Integração com os demais contratos

- [Inventário](../inventory/INTERFACES.md): fonte de `SyncItem`, `SyncBaseline`, estados de presença, revisão, fila durável, confirmação e atividades.
- [Models](../models/INTERFACES.md): modelos observados `LocalFile` e `RemoteFile` importados pelo Inventário.
- [Operador](../operations/INTERFACES.md): contratos de requisição, resultado e erros das dez operações executáveis.

A implementação não deve criar cópias concorrentes dos modelos proprietários desses módulos.

## Invariantes

- Uma chamada analisa **um único `SyncItem`**, sem receber lote de `File`.
- `SyncItem.id`, `SyncItem.revision` e identidades remotas qualificadas não são intercambiáveis.
- `PresenceState::Unknown` não é ausência. A exclusão exige prova suficiente de `AbsentConfirmed`.
- Resultado de algoritmo não autoriza por si só alterar arquivos: precondições e capacidades precisam ser validadas antes da execução.
- A sequência de comandos pode incluir mais de uma operação; o resultado preserva ordem, dependências e progresso confirmado.
- `Completed` significa que não há ação necessária ou que todos os efeitos requeridos foram verificados **e registrados**.
- Não aguardar decisões humanas dentro de `analyze`; devolver `Pending` com contexto recuperável.
- Nunca repetir automaticamente operação de efeito remoto indeterminado; reobservar primeiro.
- Não avançar o baseline por sucesso presumido, efeito parcial ou mera observação.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** contrato arquitetural proposto para a primeira versão documental. Não representa implementação Rust comprovada. Os contratos próprios dos algoritmos internos serão documentados posteriormente.
