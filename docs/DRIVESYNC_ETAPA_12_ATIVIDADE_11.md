# Etapa 12 — Atividade 11 — Resolução de conflito

Em 24/11/2026, usei dois itens independentes na pasta de teste `1njToKXiEyjWqNMrupttJPmO_7G2l5c8E` da conta `rtosta.coc@gmail.com`. Cada item tinha identidade local e remota própria e baseline sincronizado antes das edições divergentes.

- O diálogo envia conta, ID do conflito, revisão exibida e escolha. O Engine verifica o escopo e a revisão sob o bloqueio do Inventory e recusa nova decisão enquanto houver uma escolha registrada. Na execução, o Analyzer e o Operator verificam a evidência autorizada de ambos os lados. O arquivamento existente publica `local.bin` e `remote.bin` antes da substituição.
- A confirmação desabilita novo envio imediatamente. O aceite da escolha leva a `Applying`. O diálogo só mostra `Resolved` ao encontrar a confirmação persistida para aquele ID. O Inventory aposenta o conflito ao gravar um baseline verificado. Enquanto a lista pública contém a pendência, o dashboard a mantém em `SyncAttention`.
- **Adiada e obsoleta.** O botão de adiamento fechou a revisão e a releitura pública voltou com a mesma lista. Em seguida, alterei a réplica local com a revisão aberta. O Engine publicou outro ID para o mesmo item. O clique na confirmação antiga mostrou `Stale` e a revisão seguinte exibiu os bytes novos.
- **Falha.** Enquanto o primeiro item retomava a convergência, a primeira confirmação de `KeepRemote` exibiu `Operation failed`. O registro remoto permaneceu na revisão 13, sem escolha. A causa exata do erro de submissão não foi capturada. Ao repetir depois da convergência do primeiro item, a escolha foi aplicada.
- **KeepLocal.** A escolha foi aceita pelo diálogo e a convergência terminou após reabrir o Flatpak. O Inventory confirmou o ID `998ad82ed6a448fe230149575184bcc7` na revisão 22, com o mesmo arquivo remoto na versão 7. O diálogo dessa execução foi interrompido pela asserção do roteiro que esperava uma falha após parar a sessão. A correção posterior passou a recusar escolhas novas em sessões já encerradas.
- **KeepRemote.** O Flatpak instalado exibiu `Submitting`, `Applying` e `Resolved`. O Inventory confirmou o ID `56c5fc5dfa9cd4c88e99121b842dc266` na revisão 20, com o mesmo arquivo remoto na versão 6. A lista pública terminou vazia.
- **Bytes.** Li os arquivos locais e fiz GET independente do conteúdo no Google Drive. Comparei os bytes em ambos os lados com os payloads escolhidos e os SHA-256 no baseline. Comparei também os dois arquivos do arquivamento de cada conflito com os payloads que estavam na revisão efetivamente autorizada.

| Escolha | SHA-256 confirmado nos dois lados e no baseline |
| --- | --- |
| `KeepLocal` | `0f49ecd0de35c4411b01a8eece05005c7d79046edcc67a01592a52a14aa99a40` |
| `KeepRemote` | `2c93a98b049ce03c2cf6a895795345fb71d4b886d8c97f1816b0630c782477cf` |

Os logs `installed-validated.log`, `installed-resume.log`, `installed-retry.log`, `verify.log`, `workspace-final.log` e `inventory-verified.json` estão em `relatorios/etapa-12-atividade-11`, no workspace de validação. O pacote Devel `stage12-event11` teve instrumentação temporária de cliques e leitura de rótulos fora do commit. A execução final usou o OSTree `5087a20d64bfed374555b94f5065d819fadff0f45b17d35ff098aa4219dfda99`.
