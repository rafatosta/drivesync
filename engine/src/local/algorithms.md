# Algoritmos — local

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
LOCAL-001 monitorar; LOCAL-002 estabilizar.

## Objetivo
Inventário e eventos do filesystem.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Validar raiz.
2. observar identidade,tamanho,mtime e hash sob demanda.
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
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

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
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### LOCAL-002

- **Técnica conhecida:** Estabilização de gravação e verificação de alterações concorrentes.
- **Referências primárias:** [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html), [Rust std::fs metadata](https://doc.rust-lang.org/std/fs/fn.metadata.html).
- **Política do DriveSync:** Aguardar quiescência é heurística, não prova de arquivo fechado; revalidar tamanho, mtime e identidade após leitura; replanejar se mudou.
- **Testes/oráculos a detalhar:** Gravação lenta, edição simultânea, arquivo trocado mantendo nome.
- **Limitações / questões abertas:** Avaliar estratégia de snapshot/stream para origem modificada durante upload.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
