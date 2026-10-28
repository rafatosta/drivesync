# Casos de teste — download

| ID | Entrada/condição | Resultado obrigatório |
|---|---|---|
| DOWNLOAD-T01 | Fluxo nominal | Saída tipada conforme contrato |
| DOWNLOAD-T02 | Entrada inválida | Nenhum efeito e erro classificado |
| DOWNLOAD-T03 | Rede/disco indisponível | Estado preservado, retry controlado |
| DOWNLOAD-T04 | Crash antes de commit | Recuperação e reconciliação segura |
| DOWNLOAD-T05 | Operação repetida | Idempotência ou revalidação |
| DOWNLOAD-T06 | Alteração concorrente | Conflito, nenhuma perda silenciosa |

A tabela acima é um catálogo genérico. Para a cobertura local implementada,
consultar a [matriz do eixo B/L/R](../../state/preparation.md#rastreabilidade-das-regras-do-eixo-1);
para o aceite com serviço real, seguir as baterias abaixo.

## Aceite local de identidade — SYNC-002

Consultar os [oráculos I01–I28](identity-test-cases.md), vinculados ao contrato
proposto RF-ID-01–07 na especificação. O catálogo distingue verificadores
existentes de variantes necessárias e não representa testes aprovados.

## Baterias com Google Drive real

**Estado: pendentes de execução.** Este roteiro define o que demonstrar; não é
registro de testes aprovados. Os resultados, comandos e build utilizados devem
acompanhar a issue/PR, sem transformar este documento em diário de execução.
Requisitos: [RF-SYNC-01–05](../../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização).
A [matriz de testes locais](../../state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
não substitui nenhuma das baterias reais abaixo.

### Preparação comum e evidências

- Usar conta Google de teste, perfil isolado e raízes local/remota exclusivas,
  com arquivos dispensáveis. Manter upload desabilitado para esta bateria de
  download; preparar alterações remotas por ferramenta/API ou interface do Drive.
- Registrar commit/build, APP_ID, ambiente de execução (SDK ou aplicativo
  instalado), conta e IDs das raízes. Não misturar identidades do portal entre
  host, SDK e aplicativo instalado; não reescrever device/inode no journal.
- Usar binários comuns, por exemplo `.txt`, com conteúdos distinguíveis
  `BASE-001`, `LOCAL-002` e `REMOTO-002`; incluir arquivo vazio e arquivo grande
  nos testes de integridade/interrupção. Workspace/exportação não pertence ao
  aceite do eixo 1.
- Cada cenário parte de preparação independente. Atualizar bytes **do mesmo ID
  remoto** quando o teste exige mudança de revisão; excluir e reenviar com o mesmo
  nome testa outra identidade. Não decidir por nome, data ou tamanho isolados.
- Fechar todos os processos que usam a raiz antes de retirar ou manipular o estado.
  O journal fica em `.drivesync/state.sqlite3` dentro da raiz; apagar apenas dados
  privados do aplicativo não simula sua perda. Preservar uma cópia de evidência
  fora da raiz e considerar arquivos auxiliares SQLite antes de qualquer alteração.
  Não executar manutenção destrutiva sobre perfis/arquivos pessoais.
- Para cada caso, registrar passos, IDs/revisões remotas, hashes e bytes locais e
  remotos antes/depois, device/inode quando aplicável, B/fase persistida e ação ou
  classificação observada. Não incluir credenciais/tokens nos registros.
- Repetir após reinício real do processo e novo ciclo, conferindo idempotência,
  ausência de perda e ausência de duplicação. Inspecionar estado somente com o
  aplicativo parado ou por mecanismo de leitura consistente.
- Preservação de bytes sozinha não comprova a classificação. Se a interface não
  distinguir mudança local, concorrência e estado desconhecido, exigir diagnóstico
  suficiente da base/fase e da decisão. Sem essa evidência, o caso fica inconclusivo.

### Bateria obrigatória de aceite SYNC-001 — A01–A12

Fonte: [comentário de aceite da issue #1](https://github.com/rafatosta/drivesync/issues/1#issuecomment-6480152816).
Os procedimentos exercitam a implementação de [planejamento](../../planner/algorithms.md),
[persistência](../../state/algorithms.md), [instalação](../execution.md) e
[atualização/recuperação](../updates.md).

| ID / regra | Preparação e procedimento com Drive real | Oráculo obrigatório |
| --- | --- | --- |
| A01 / RF-SYNC-01 | Criar arquivo remoto, instalar em raiz local vazia, fechar/reabrir o app e sincronizar novamente. | Bytes íntegros; B durável recuperada; repetição sem download/substituição desnecessária nem duplicação. |
| A02 / RF-SYNC-02 | Executar apenas preparação/inventário, sem instalar. Reiniciar, criar arquivo local preexistente no destino e preparar/sincronizar novamente. | Observação/plano não confirma instalação; B continua ausente; arquivo preexistente não é adotado nem sobrescrito. |
| A03 / RF-SYNC-04 | Após instalação confirmada, atualizar bytes do mesmo ID remoto, mantendo local intacto. Sincronizar e reiniciar. | Atualização segura, versão anterior preservada; nova B somente após confirmação durável. Próximo ciclo sem falsa pendência ou transferência redundante. |
| A04 / RF-SYNC-03 | Após B confirmada, editar somente o local; sincronizar, reiniciar e repetir. | Classificar mudança local, preservar bytes e B anterior; nenhum download por cima. Upload não é exigido. |
| A05 / RF-SYNC-03, C01 | Após B confirmada, alterar local e remoto para conteúdos diferentes; sincronizar, reiniciar e repetir. | Classificar mudanças concorrentes em relação a B; preservar cada versão no respectivo lado; não confirmar convergência falsa. |
| A06 / RF-SYNC-03, C06 | Confirmar B, fechar/desconectar o app, alterar os dois lados, reabrir/reconectar e sincronizar. | Recuperar B anterior e detectar divergência por revisão/fingerprint, sem decisão apenas por timestamps. |
| A07 / RF-SYNC-02, N01 | Sem B, criar arquivos no mesmo caminho dos dois lados; testar separadamente conteúdos iguais e diferentes. | Nenhuma adoção, sobrescrita ou exclusão inferida de nome/data/igualdade de bytes. |
| A08 / RF-SYNC-02, N02 | Confirmar instalação, fechar app e retirar o journal da raiz de teste, mantendo arquivos e demais evidências; reabrir/sincronizar. | Preservar local/remoto; não reconstruir B nem inferir exclusão ou instalação anterior pela presença dos arquivos/marcador. |
| A09 / RF-SYNC-02, N02 | Em preparações separadas, usar banco corrompido, banco ilegível e banco estruturalmente válido sem evidência suficiente de instalação. | Inválido/ilegível produz erro seguro sem reset nem efeito destrutivo; incompleto mantém B desconhecida sem autorização inferida. Registrar qual variante foi exercitada. |
| A10 / RF-SYNC-05 | Testar instalação inicial e atualização: interromper durante streaming, após staging verificado e, de forma controlada, após efeito no filesystem e antes da confirmação SQLite. Reiniciar em cada janela. | Parcial/incompleto não fornece B. Operação posterior incompleta mascara candidata anterior no planejamento; replay revalida evidência, preserva destinos concorrentes e só confirma efeito demonstrado. Repetição não perde nem duplica versões. |
| A11 / RF-SYNC-04 | Confirmar V1 → V2 remota, reiniciar e editar somente o local; sincronizar e repetir. | Demonstrar B de V2; classificar edição local em relação a V2, preservando-a. Não aceitar base presa em V1. |
| A12 / RF-SYNC-01 | Substituir arquivo confirmado por outro de mesmo nome/bytes e inode diferente. Em cenários independentes, trocar conta, raiz remota e raiz local. | Revalidar identidade; nenhum novo arquivo/escopo herda autorização por nome/hash ou B alheia. Registrar identidades antes/depois e bloqueio observado. |

**Critério de encerramento:** todos os cenários e variantes aplicáveis possuem
evidência dos oráculos, incluindo classificação/base e repetição após reinício.
N01/N02 e C01/C06 precisam ser demonstrados no recorte da issue. Não aceitar
preservação acidental como prova de uso de B nem marcar A10 completo somente por
matar um processo aleatoriamente durante transferência. Falta de instrumentação
ou diagnóstico torna a variante pendente; não reduz seu critério.

### Bateria complementar de integração com o provedor e empacotamento

Estes casos complementam o aceite B/L/R e não habilitam sincronização bidirecional.
Devem ter evidências próprias, sem alegar cobertura de todos os conflitos gerais.

| Grupo | Procedimento | Resultado esperado |
| --- | --- | --- |
| G01 — Autenticação/listagem | Conectar conta de teste, consultar raiz e pasta selecionada; fechar/reabrir e repetir. | Conta/raiz corretas, erros sanitizados e nenhum token em logs. O smoke test de navegação cobre somente parte deste grupo, nunca A01–A12. |
| G02 — Integridade e revisão | Instalar binário comum, vazio e grande; atualizar bytes mantendo ID; provocar revisão apenas de metadados mantendo ID/caminho/bytes finais. | Hashes/tamanhos corretos; revisão nova reconhecida. Caso somente metadados: sem nova transferência de mídia, backup ou troca de inode, após revalidações. Conferir journal e contagem de mídia, não apenas bytes finais. |
| G03 — Rede e disponibilidade | Desconectar durante transferência e entre ciclos; testar perda de acesso e envio à Lixeira apenas de fixtures remotas descartáveis. Restaurar condições e repetir. | Erro/indisponibilidade não implica exclusão local; B não avança com parcial. Preservar bytes e retomar sem duplicação. Não presumir que todo 404 significa exclusão; a distinção de causas requer evidência. |
| G04 — Concorrência observável | Alterar remoto durante download e criar/editar/substituir destino local enquanto a transferência ocorre; ocupar um nome de backup em raiz isolada. | Revalidação detecta divergências observadas; nenhuma substituição de dados concorrentes. Preservar versões/evidências e relatar conflito ou mudança remota. Não prometer snapshot distribuído. |
| G05 — Aplicativo instalado/portal | Executar A01, A03, A10 e A12 também no Flatpak instalado, com seleção de raiz pelo portal e permissões reais; fechar/reabrir pelo desktop. | Mesmas garantias sob o caminho salvo e identidade do portal; nenhuma adoção por alias de host/SDK. Build no SDK sozinho não comprova empacotamento, grants ou ciclo de vida desktop. |

Queda real de energia e falhas de fsync/disco exigem ambiente descartável e ensaio
específico de armazenamento; não são comprovadas por testes com Google, SIGKILL
ou triggers SQLite. Resolução assistida, atualização remota de existentes,
propagação de exclusões e documentos Workspace precisam de contratos/baterias
próprios quando implementados; não são condições adicionais desta issue.

### Ferramentas disponíveis e lacunas de execução

O exemplo [sync-cycle.rs](../../../examples/sync-cycle.rs) permite ciclo explícito
com conta/raízes esperadas, opt-in e conferência de contagens. O script
[check-live-sync.py](../../../../scripts/check-live-sync.py) auxilia cancelamento,
SIGKILL e recuperação de uma fixture. Consultar o
[roteiro operacional histórico](../../../../docs/old/live-sync-tests.md), conferindo
as opções no código atual antes do uso. Toda compilação deve ocorrer no SDK/Rust
fixados pelo manifesto Flatpak do projeto; não compilar com Cargo do host.

Essas ferramentas não automatizam todos os casos: preparar sem instalar (A02),
inspecionar classificação/B e interromper a janela após efeito e antes da
confirmação (parte de A10) precisam de controles/evidências específicos. O hold
antes de rename não demonstra a janela depois do rename. Um conector Drive
prepara/confere o lado remoto, mas sozinho não controla o aplicativo nem lê seu
journal/filesystem local. Não executar comandos contra conta ou pasta escolhida
por inferência; identificar explicitamente o perfil e as raízes descartáveis.
