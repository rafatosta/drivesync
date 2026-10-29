# Algoritmos — local

**Status:** varredura e estabilização implementadas conforme
[contrato vigente](reconciliation.md). Monitoramento contínuo permanece proposta.

## Identificadores
LOCAL-001 monitorar; LOCAL-002 estabilizar.

## Objetivo
Inventário e eventos do filesystem.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Validar raiz.
2. observar identidade, tamanho, mtime/ctime e hash dos candidatos estáveis.
3. reagrupar eventos de editores.
4. rescan após overflow.
5. testar estabilidade antes de upload.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Symlink não deve escapar da raiz autorizada. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
O inventário local percorre n itens e lê B bytes de arquivos regulares estáveis:
O(n+B), com armazenamento O(n) de evidências e buffers limitados. A janela de
500 ms é compartilhada por toda a varredura. Metadados e leituras são observações,
não um snapshot atômico de toda a árvore.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### LOCAL-001

- **Técnica conhecida:** Monitoramento de arquivos e reconciliação por varredura.
- **Referências primárias:** [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html), [GIO monitor events](https://docs.gtk.org/gio/signal.FileMonitor.changed.html).
- **Política do DriveSync:** Eventos são pistas; varredura completa reconcilia perda de eventos; manter segurança de caminhos e symlinks.
- **Testes/oráculos a detalhar:** Renomeação, editor com arquivo temporário, diretórios grandes, overflow/monitor indisponível.
- **Limitações / questões abertas:** Monitor recursivo e comportamento de mounts/permissões Flatpak requerem prova em Fedora.
- **Situação:** varredura por ciclo implementada; monitor contínuo permanece proposta.
  Oráculos locais em [tests.rs](tests.rs); aceitação de portal/mount instalado pendente.

### LOCAL-002

- **Técnica conhecida:** Estabilização de gravação e verificação de alterações concorrentes.
- **Referências primárias:** [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html), [Rust std::fs metadata](https://doc.rust-lang.org/std/fs/fn.metadata.html).
- **Política do DriveSync:** Aguardar quiescência é heurística, não prova de arquivo fechado; revalidar tamanho, mtime e identidade após leitura; replanejar se mudou.
- **Testes/oráculos a detalhar:** Gravação lenta, edição simultânea, arquivo trocado mantendo nome.
- **Limitações / questões abertas:** quiescência e snapshot não bloqueiam escritores externos; assinatura é revalidada por bloco e identidade/hash antes do bloco final, sem transação entre filesystem e Google.
- **Situação:** estabilização, snapshot privado e revalidações implementados; ver [upload](../upload/create-only.md). Aceite real permanece pendente.
