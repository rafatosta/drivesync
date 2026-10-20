# Algoritmos — changes

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
CHANGES-001 local; CHANGES-002 remoto.

## Objetivo
Coleta incremental local e remota.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Ler cursor e base.
2. coletar alterações com IDs e tombstones.
3. paginar.
4. registrar duravelmente.
5. avançar cursor só após gravação.
6. rescan após evento perdido.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Estado desconhecido nunca é ausência. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### CHANGES-001

- **Técnica conhecida:** Observação local incremental com reconciliação.
- **Referências primárias:** [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html).
- **Política do DriveSync:** Mesclar eventos com inventário periódico e identidade; evento ausente não significa arquivo removido.
- **Testes/oráculos a detalhar:** Evento perdido, renomeação, editor salva com rename, horário deslocado.
- **Limitações / questões abertas:** Definir frequência de rescan baseada em medição, não suposição.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### CHANGES-002

- **Técnica conhecida:** Log incremental remoto com cursor durável.
- **Referências primárias:** [Google Drive retrieve changes](https://developers.google.com/workspace/drive/api/guides/manage-changes), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list).
- **Política do DriveSync:** Capturar startPageToken antes de scan inicial e reconciliar janela; registrar alterações antes de persistir cursor; mudanças de acesso não implicam exclusão.
- **Testes/oráculos a detalhar:** Interrupção entre páginas, mudança durante scan inicial, tombstone ambíguo, token inválido.
- **Limitações / questões abertas:** Estratégia exata de recuperação de token inválido precisa ser ensaiada.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
