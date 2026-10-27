# Pesquisa comparativa — experiência de resolução de conflitos

**Data:** 2026-10-27  
**Natureza:** estudo conceitual de engenharia de software e UX; não constitui decisão de implementação nem requisito aprovado.  
**Escopo:** como comunicar, explicar, comparar e resolver divergências de sincronização.  
**Relações:** [casos e testes](validation-checklist.md), [viabilidade técnica](technical-feasibility.md), [algoritmos](algorithms.md).

## Problema do usuário

"Há dois conflitos" não descreve o problema nem auxilia a decisão. O usuário precisa identificar **qual arquivo**, **em quais locais/contas**, **o que mudou**, **qual é a consequência de cada escolha**, **se há risco de perda**, **o que foi preservado** e **o que ocorrerá se adiar**. Conflito de versões, exclusão, caminho e erro operacional não devem ter mensagem indistinta.

## Pesquisa de soluções existentes

| Ferramenta | Tratamento observado ou documentado | Lição conceitual | Limite da analogia |
|---|---|---|---|
| **Git** (aberto) | Mesclagem automática quando alterações são conciliáveis; em conflitos, mantém estados base/local/externo, mostra trechos conflitantes e permite diff/mergetool e resolução explícita | Mostrar diferenças relevantes antes de decidir; contextualizar base e alternativas | É controle de versões; a abordagem de texto/linhas não serve genericamente a arquivos binários |
| **Syncthing** (aberto) | Em edição simultânea divergente, preserva cópia com sufixo `sync-conflict`; usa critérios determinísticos para escolher nome/versão principal; conflito também pode ocorrer entre edição e exclusão | Preservar versões e rastrear a origem; não interpretar escolha automática do nome principal como escolha semântica do conteúdo correto | Arquivos extras propagados podem confundir; não oferece por si um comparador universal |
| **Nextcloud desktop** (aberto) | Conflitos podem gerar cópia conflitante local separada, exigindo revisão e reconciliação manual | Comunicar pendência e permitir localizar as duas cópias | Não pressupor experiência de comparação integrada universal; conferir versão do cliente |
| **MEGAsync** (cliente publicado com código-fonte) | Referência de fluxos de "sync issues"/cópias conflitantes e decisões de preferência de versões conforme cenário/versão | Tornar opções compreensíveis e distinguir conflitos de erros | Comportamentos e telas variam; **não assumir** que todas as opções estão disponíveis em toda versão sem ensaio visual reproduzível |
| **Google Drive para computadores** | Documenta erros de sincronização, modos de espelhamento/streaming e recuperação de arquivos; não foi estabelecida existência de comparador universal para todo formato | Informar claramente sincronização pendente, recuperação e segurança | Não extrapolar o comportamento do cliente proprietário para capacidades da API |

**Distinção metodológica:** a tabela reúne comportamentos descritos pelas fontes e lições propostas para o DriveSync. Não é comparação de desempenho nem teste de interfaces lado a lado. Confirmar versões específicas antes de transformar exemplos em requisitos normativos.

### Fontes para revisão

- Git: [git-merge](https://git-scm.com/docs/git-merge), incluindo conflitos de três vias e `git mergetool`.
- Syncthing: [Understanding Synchronization — Conflicting Changes](https://docs.syncthing.net/users/syncing.html).
- Nextcloud: [Desktop client documentation](https://docs.nextcloud.com/desktop/latest/).
- MEGA: [repositório MEGAsync](https://github.com/meganz/MEGAsync) e [central de ajuda](https://help.mega.io/).
- Google: [Drive for desktop Help](https://support.google.com/drive/answer/10838124).

## Princípios de experiência propostos

1. **Explicar antes de pedir ação:** mensagem em linguagem natural ("O arquivo foi alterado neste computador e no Google Drive após a última sincronização"), não apenas código C01.
2. **Preservar dados primeiro:** nunca exigir decisão sob ameaça de perda; indicar quais cópias de fato foram preservadas, sem prometer preservação quando uma cópia ainda estiver inacessível.
3. **Comparar com contexto:** nome, caminho completo, conta, tipo, origem, data/hora e fuso, tamanho, estado de acesso, última sincronização, pré-visualização quando suportada e diferenças identificáveis.
4. **Não equiparar 'mais recente' a 'correto':** timestamp é evidência, não veredicto; metadados podem variar ou faltar.
5. **Escolha reversível quando possível:** informar o destino da versão não selecionada e oferecer rastreabilidade da decisão.
6. **Adiar sem bloquear o todo:** conflito pendente não deve impedir arquivos independentes; permanecer visível após reinício.
7. **Separar divergência de erro:** quota, autenticação e falha de transferência requerem reparo/tentativa, não escolha arbitrária de conteúdo.
8. **Acessibilidade GNOME:** componentes libadwaita, textos claros, navegação por teclado, leitores de tela, estados não comunicados só por cor.

## Taxonomia destinada à interface

| Rótulo para o usuário | Exemplos de códigos internos | Explicação e ação prioritária |
|---|---|---|
| **Duas versões diferentes** | C01, C06, C21 | Mostrar diferenças, oferecer manter local/remota/ambas; C21 pode não ser conflito real |
| **Arquivo excluído ou indisponível** | C03, C04, C08, C11, C16, C25 | Explicar a incerteza: excluído, movido ou acesso perdido; priorizar preservar e verificar |
| **Nome ou localização em conflito** | C02, C05, C07, C10, C13–C20 | Exibir árvore/caminhos e possíveis nomes de saída |
| **Arquivo ou propriedade incompatível** | C22–C24 | Explicar limitação de permissão, metadado ou tipo e ações suportadas |
| **Problema de sincronização** | E01–E10 | Mostrar causa, progresso, reintento e orientação; não apresentar como escolha entre versões |

## Fluxo de interface (proposta, não implementado)

1. **Visão geral: "Precisa de atenção"** — número de itens por conta/pasta; separar *Conflitos* de *Problemas de sincronização*; acesso à lista persistente.
2. **Lista de pendências** — nome do arquivo, tipo de divergência em palavras simples, caminho, conta, estado de segurança; filtros por tipo e origem. Evitar apenas ícone genérico de erro.
3. **Detalhe** — "O que aconteceu?" + "Compare as versões" (computador vs Drive), dados comparáveis e mensagem sobre incertezas. Se relevante, apresentar estado anterior como referência.
4. **Comparação adequada ao conteúdo**:
   - texto/código: diff lado a lado, destacando linhas e, quando possível, base comum;
   - imagens: miniaturas, ampliação e sobreposição opcional;
   - PDF: metadados e prévias apenas se houver renderizador seguro;
   - planilhas, vídeos, arquivos binários e documentos nativos: metadados + ações "Abrir versão local/remota" quando realmente disponíveis; não prometer diff sem suporte.
5. **Opções de resolução** — manter versão local, manter versão do Drive, manter ambas, decidir depois. Mostrar **consequências específicas** antes de confirmar. Desabilitar opções tecnicamente impossíveis.
6. **Confirmação e recuperação** — revalidar origem/versão, executar sem sobrescrita silenciosa, registrar resultado e cópias; em caso de mudanças desde a comparação, solicitar nova análise.
7. **Continuidade** — manter itens independentes sincronizando e permitir revisar histórico das decisões.

### Exemplo de linguagem

- Resumo: "Encontramos 3 arquivos com alterações que precisam de atenção."
- Explicação: "Relatório.xlsx foi alterado neste computador e no Google Drive após a última sincronização."
- Confiança: "Nenhuma das versões disponíveis foi descartada." **Somente exibir quando verificado.**
- Ação: "Compare as versões e escolha o que manter. Você também pode decidir depois."
- Exclusão ambígua: "O arquivo não está disponível no Google Drive. Ele pode ter sido excluído, movido ou estar sem permissão de acesso."

## Preferências de resolução (propostas)

| Preferência | Padrão proposto | Segurança/limite |
|---|---|---|
| Sempre perguntar em divergência real | **Ligada** | Escolha explícita do usuário |
| Manter ambas automaticamente | Desligada | Preservação segura, mas pode gerar duplicatas; nunca sobrescrever cópia |
| Preferir mais recentemente modificado | Desligada | Não aplicar a exclusões, dados ambíguos, diferenças de clock ou tipos não suportados |
| Preferir local / remoto | Desligada | Somente categorias permitidas com versão divergente preservada e consequência visível |
| Notificar novas pendências | Ligada | Agrupar notificações, evitar ruído |
| Propagar exclusões automaticamente | **Desligada** | Exigir política específica, confirmação e recuperação, especialmente em lotes |

As preferências são **hipóteses de produto**, não garantias nem substitutos para invariantes de segurança. Se a integridade, a origem ou a versão estiverem incertas, **bloquear e solicitar atenção** mesmo com regra automática ativa. Preservação antes de substituição não deve ser um toggle que permita perda silenciosa.

## Aprimoramentos da experiência após a revisão

- Apresentar grupos separados **Conflitos**, **Incompatibilidades** e **Problemas de sincronização**; o mesmo painel pode reunir as três situações.
- Antes de qualquer escolha, mostrar **impacto da ação**: versão mantida, versão preservada, local de recuperação e limitações de reversibilidade.
- Diferenciar **preservação concluída**, **preservação pendente** e **versão indisponível**; não declarar segurança sem evidência.
- Permitir resolução em lote apenas para tipos homogêneos, com prévia do impacto e salvaguardas para operações destrutivas.
- Para **N03/N04**, comunicar indisponibilidade da pasta-raiz sem chamar de exclusão; para **N06**, exigir nova comparação quando a versão mudar.
- Para **N08**, suspender exclusões em massa e explicar quantidade, proporção e escopo afetado, sem estabelecer limiares arbitrários.
- Manter registros distintos de **cópias de conflito**, **histórico de versões** e **decisões**.

## Questões abertas para engenharia de requisitos

- Quais formatos ganharão comparação visual nativa na primeira versão?
- Onde manter cópias de conflito e por quanto tempo? Como indicar espaço consumido?
- Como apresentar conflitos de centenas de arquivos sem estimular decisões em lote perigosas?
- Quando uma regra automática pode agir? Como o usuário visualiza e reverte decisões?
- Como comunicar quando o Drive retorna "indisponível", mas não é possível distinguir exclusão de revogação?
- Qual a interação GNOME exata (lista de atenção, detalhe, diálogos de confirmação) e seus testes de usabilidade?

## Critérios de validação futura da UX

- [ ] Usuário consegue explicar **o que aconteceu** sem ler documentação técnica.
- [ ] Usuário reconhece **qual versão pertence a qual origem**.
- [ ] Consequências de "manter local/remoto/ambas" ficam claras antes da ação.
- [ ] Mensagens diferenciam exclusão confirmada de perda de acesso/incerteza.
- [ ] É possível adiar e retornar à mesma pendência após reinício.
- [ ] Para arquivo incompatível, a UI não promete comparação ou ação indisponível.
- [ ] Erros operacionais têm orientação própria e não são confundidos com conflito de versões.
- [ ] O fluxo é utilizável com teclado, leitores de tela e sem distinção exclusiva por cor.
- [ ] Preferências automáticas não violam a política de preservação e ações destrutivas.

**Conclusão conceitual:** combinar a comparação contextual do Git, a preservação de cópias do Syncthing/Nextcloud e decisões legíveis ao usuário. A diferenciação de produto proposta é tornar o conflito **compreensível e rastreável**, não apenas detectável.
