# SYNC-026 — estudo e candidato textual de três vias

## Contrato e pré-condições

`merge_plain_text` é um gerador puro de candidatos, reexportado pela biblioteca.
Não é chamado automaticamente pelo coordenador, comparador ou executor. Recebe
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

O journal atual conserva metadados/fingerprints, não os bytes de uma base comum
por identidade. A [issue #68](https://github.com/rafatosta/drivesync/issues/68)
acompanha armazenamento verificável de B, alinhamento geral, consentimento,
preservação B/L/R, revalidação e execução recuperável. Até essa integração,
as políticas e conflitos existentes permanecem conservadores. Não anunciar
mesclagem automática disponível na interface.

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
