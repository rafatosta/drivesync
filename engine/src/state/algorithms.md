# Algoritmos — state

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
STATE-001 gravar; STATE-002 recuperar.

## Objetivo
Persistência transacional e journal SQLite.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Validar schema.
2. persistir intenção antes de efeito externo.
3. confirmar resultados de forma atômica.
4. persistir cursor apenas após eventos.
5. ao corromper preservar banco e suspender destrutivas.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Banco corrompido não é banco vazio. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### STATE-001

- **Técnica conhecida:** Transações locais SQLite e journal de intenção.
- **Referências primárias:** [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [SQLite WAL](https://www.sqlite.org/wal.html).
- **Política do DriveSync:** Persistir intenção antes de efeito externo; confirmar base somente após efeito verificado; transação SQLite NÃO é transação distribuída Google+disco.
- **Testes/oráculos a detalhar:** Crash antes/depois do commit, WAL, energia interrompida, fsync falho.
- **Limitações / questões abertas:** Escolha de synchronous, WAL e local do DB precisam de testes reais.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### STATE-002

- **Técnica conhecida:** Corrupção e recuperação conservadora.
- **Referências primárias:** [SQLite integrity_check](https://www.sqlite.org/pragma.html#pragma_integrity_check), [SQLite WAL](https://www.sqlite.org/wal.html).
- **Política do DriveSync:** Preservar banco corrompido e quarentenar; bloquear destrutivas; reconstruir apenas por inventário controlado.
- **Testes/oráculos a detalhar:** Falha ao abrir DB, página corrompida, cursor parcial, operação em trânsito.
- **Limitações / questões abertas:** Critérios explícitos de retorno do modo degradado.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
