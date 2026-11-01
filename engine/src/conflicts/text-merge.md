# SYNC-026 — estudo e candidato textual de três vias

## Contrato e pré-condições

`merge_plain_text` é um gerador puro de candidatos, reexportado pela biblioteca.
É usado pela prévia explícita `preview_text_merge`; não é uma política automática. Recebe
MIME explícito `text/plain`, bytes opcionais de B e bytes de L/R. O chamador deve
comprovar que B é a base comum confirmada do mesmo item/conta/raiz. A função não
faz essa autenticação. Hashes e revisões sem bytes não permitem reconstruir B.

Política DriveSync: somente UTF-8 sem NUL, até 1 MiB por entrada e saída,
substituições por posição com igual quantidade de linhas e terminadores por
posição iguais (LF, CRLF ou última linha sem terminador). Não interpretar JSON,
CSV, XML, Markdown, Office, arquivos nativos Google ou binários como texto plano
apenas porque seus bytes são UTF-8. Nenhuma nova dependência ou processo externo.

## Algoritmo determinístico

1. Recusar MIME não suportado ou base ausente.
2. Recusar tamanho excessivo, UTF-8 inválido ou NUL em qualquer versão.
3. Separar linhas preservando terminadores; recusar quantidades ou terminadores
   diferentes. Arquivo vazio tem zero linhas.
4. Para cada posição: se L=R usar L; senão, se R=B usar L; senão, se L=B usar R;
   senão recusar a operação inteira por sobreposição.
5. Concatenar sem normalizar bytes, recusando saída acima do limite.

Tempo e memória são lineares nos bytes limitados. Resultados tipados:
`Candidate(bytes)` ou `Preserve(reason)` com formato, base, tamanho, codificação,
estrutura ou sobreposição como motivo. Nunca retornar saída parcial ou marcadores
como se fossem conteúdo conciliado. Entradas emprestadas permanecem intactas.

## Limites de correção e integração

Esta é uma especialização **posicional**, não um diff3 geral: igual número de
linhas não prova ausência de inserções/exclusões compensadas ou movimentações.
Mesmo mudanças em linhas distintas podem ter dependência semântica. Portanto
candidato não significa resolução segura e requer revisão; não autoriza aplicar,
limpar pendência, avançar B ou escolher conteúdo prevalecente automaticamente.
MIME e B são dados fornecidos pelo chamador, não evidência produzida pelo algoritmo.

O journal retém bytes de `text/plain` até 1 MiB em transação com a confirmação
verificada de download, upload ou resolução. A chave inclui cliente, conta,
raiz remota/local, ID, revisão e SHA-256. O limite por escopo é 64 MiB, sem
coleta automática; falta de espaço bloqueia retenção, nunca remove evidências.
Bases anteriores à retenção permanecem ausentes; não são reconstruídas.

`preview_text_merge` exige conflito concorrente pendente, continuidade do inode,
base confirmada retida e comparação L/R verificada. O candidato encapsula essas
versões, escopo e ETag exato, sem expor credenciais. `accept_reviewed` é o opt-in
específico para este candidato `text/plain`; nenhuma preferência geral o ativa.
O usuário deve revisar todo o resultado, inclusive inserções/exclusões compensadas.

A escolha `MergeText` exige essa evidência privada e usa o executor existente:
1. Persistir a intenção exata e conferir conta/raízes/base e versões ao vivo.
2. Preservar B e resultado em arquivos privados, L em snapshot privado e R em backup;
   conferir hashes de L/R contra os bytes revisados antes de avançar `Preserved`.
3. Publicar `RemotePending`, atualizar o ID original com a precondição ETag e
   verificar conteúdo remoto; após resposta perdida, reconciliar antes de repetir.
4. Preparar instalação, persistir inode em `LocalPending`, deslocar L sem sobrescrever
   e instalar resultado; conferir versões preservadas, resultado local e remoto.
5. Confirmar nova B com bytes e auditoria no journal em transação; concluir histórico.

Cada retomada confere base e resultado preservados. Falha, offline, corrupção,
base ausente, formato/estrutura não suportados ou mudança de versão não resolvem
o conflito. Intenções já aceitas e efeitos parciais permanecem recuperáveis;
falhas anteriores a efeitos podem abortar e exigir nova revisão. Relatório e
recuperação mostram caminhos de B, L, R e resultado. Backups novos usam 0600;
o inode L deslocado mantém as permissões originais. Sem migração ou limpeza
silenciosa do formato interno 5. Interrupções reais e ensaios desktop/Google
continuam separados nas #52 e #66, sem homologação implícita.

A função não tem I/O, estado global ou journal: repetição, concorrência entre
chamadas e nova invocação após reinício têm a mesma saída para os mesmos bytes.
Descartar um candidato não altera originais. Não existe janela de efeito externo
para simular crash, rede ou rollback SQLite neste algoritmo. Esses efeitos e
respectivos testes pertencem à integração #68, não são aprovados por testes puros.

## Fundamentação

Técnica: comparação com ancestral comum e distinção de alterações sobrepostas,
conforme [GNU Diffutils, Merging From a Common Ancestor](https://www.gnu.org/software/diffutils/manual/html_node/diff3-Merging.html),
consultado em 2026-11-01. O GNU diff3 inclui alinhamento que esta especialização
não implementa; nenhuma equivalência geral é alegada. Os limites de formato,
memória, revisão e recusa são políticas do DriveSync, não garantias da referência.

## Oráculos P05

- `p05_independent_replacements_and_identical_edits`: edições independentes,
  edições iguais, Unicode, CRLF e última linha sem newline, vazio.
- `p05_refuses_unknown_base_format_binary_overlap_and_structure`: todas as
  recusas, inserção/exclusão observável e alteração de terminador.
- `p05_bounds_output_and_never_returns_partial_candidate`: limite inclusivo de
  entrada, saída combinada excessiva e sobreposição após uma linha conciliável.
- `p05_repeat_parallel_and_fresh_invocations_preserve_all_inputs`: saída idêntica
  em chamadas independentes concorrentes/repetidas, B/L/R intactos.
- O doctest público demonstra o exemplo nominal sem rede ou dados reais.

Esses oráculos cobrem geração e bloqueios de P05. Não homologam automação,
semântica de formatos, aplicativo instalado ou Google Drive real.
