# Contrato proposto — detector de alterações remotas

**Estado: implementação experimental; validação real pendente.** Este contrato
não autoriza operações sobre conteúdo local ou remoto.

## Limites e fontes

O detector consulta metadados, mantém um inventário e entrega observações para
reavaliação. Não planeja nem executa transferências, exclusões ou resolução de
conflitos. Usa a autenticação e a identidade por cliente OAuth e conta já
existentes, sem expor credenciais ou cursores em logs.

O [registro de alterações do Google Drive](https://developers.google.com/workspace/drive/api/guides/about-changes)
contém estados atuais, não deltas: entradas podem ser substituídas por uma
entrada mais recente e um item removido do registro pode apenas ter perdido
acesso ou mudado de corpus. Portanto, o contrato **não promete um evento para
cada operação do usuário** nem identifica uma exclusão física a partir de um
tombstone. O registro de um usuário e o de um drive compartilhado são distintos;
quando a conta é membro, o segundo é necessário para suas alterações. A
[listagem de arquivos](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list)
não oferece uma captura transacional e admite respostas incompletas e mudanças
nos resultados durante a paginação.

SQLite oferece [transações atômicas](https://www.sqlite.org/atomiccommit.html)
sob as premissas documentadas para filesystem e hardware; isso não substitui
testes de interrupção nem de integração com o Drive.

## Entrada, escopo e saída

- Entrada: cliente OAuth, identidade verificada da conta, ID da pasta raiz e
  configuração explícita do acompanhamento, incluindo o corpus e limites de
  trabalho. O estado é isolado por cliente, conta, raiz e corpus; nenhum ID ou
  nome de arquivo é usado como caminho local.
- A configuração do corpus deve corresponder ao acesso real. O registro de
  drive compartilhado usa o `driveId` também no token e nas consultas. Acesso
  insuficiente ou incapacidade de confirmar o corpus não permite tratar a
  consulta como completa ou substituir silenciosamente o registro.
- Saída tipada: inventário inicial, alterações, nenhuma alteração, verificação
  incompleta, falha recuperável ou falha que exige intervenção. Uma falha não
  significa ausência de mudanças.
- Cada observação tem ID de entrega estável, conta, raiz, ID do arquivo ou
  escopo afetado, metadados disponíveis e pertinência `dentro`, `fora` ou
  `desconhecida`, incluindo pertinência anterior quando conhecida. O consumidor
  deve poder solicitar reavaliação usando essas informações.
- Comparações de metadados podem indicar o que foi observado, mas não garantem
  causalidade ou todas as operações intermediárias. Usar reavaliação quando a
  operação ou o pertencimento não puderem ser determinados. Tombstones sem
  metadados e sem histórico suficiente permanecem desconhecidos, não são
  classificados como exclusões nem como alterações confirmadas da pasta.
- Atalhos são itens próprios, não diretórios a percorrer; o alvo não passa a
  pertencer à árvore pelo simples fato de haver um atalho.

## Inicialização e consulta

O inventário inicial representa o estado observado; não é uma coleção de
criações. Mudanças simultâneas à construção devem ser reavaliadas e não
descartadas.

1. Validar identidade, raiz, corpus e acesso. Obter o token inicial **antes**
   de percorrer todos os descendentes acessíveis, com paginação completa.
   Rejeitar busca incompleta, tokens malformados, ciclos e falhas parciais.
2. Consultar todas as páginas do registro desde o token inicial. Conservar
   observações potencialmente relevantes, inclusive tombstones cuja pertinência
   seja desconhecida. Reavaliar árvores afetadas por mudanças de pasta e
   reconciliar o inventário; uma mudança de ancestral pode afetar descendentes
   sem entradas individuais no registro.
3. Repetir a verificação de estabilidade e a reconciliação enquanto houver
   mudanças durante o levantamento, respeitando limites explícitos. Sem
   convergência, acesso completo ou evidência suficiente, retornar incompleto;
   não publicar um inventário como completo nem avançar o cursor confirmado.
   A consistência continua condicionada à semântica e à disponibilidade do
   provedor; o teste real deve verificar as hipóteses aplicáveis.
4. Publicar inventário inicial, observações pendentes e cursor final
   conjuntamente em uma transação durável. Nunca substituir um estado
   incompatível ou corrompido por um inventário vazio.
5. Em consultas posteriores, ler o cursor confirmado e todas as páginas, sem
   assumir que mudanças da conta pertencem à raiz. Comparar dados anteriores e
   atuais, consultar/reavaliar ancestrais e subárvores quando necessário e
   preservar a incerteza. Alterações exclusivamente fora da raiz não são
   identificadas como pertencentes a ela.
6. Confirmar inventário atualizado, eventos ainda não entregues e novo cursor
   em uma única transação. Não há avanço definitivo após falha de rede,
   paginação, validação, reconciliação ou persistência.

Uma pasta movida para dentro pode trazer descendentes sem eventos próprios;
uma pasta movida para fora ou inacessível pode afetar os descendentes já
inventariados. Essas situações exigem reavaliação da subárvore, não uma
inferência de exclusão. Mudanças no acesso ou no corpus da raiz podem exigir
intervenção e nova construção, sem apagar eventos pendentes.

## Persistência e concorrência

O estado da nova engine usa armazenamento próprio, versionado e separado dos
bancos antigos, com permissões restritas. Não há leitura, migração ou remoção
dos dados da engine anterior. O inventário inclui IDs, relações de parentesco
e metadados necessários à comparação; o cursor e a fila pendente são escopados
pela mesma identidade. Não persistir credenciais nesse armazenamento.

Publicações usam transação SQLite e controle de concorrência que impede duas
consultas da mesma geração de confirmar resultados incompatíveis. A consulta
remota não deve bloquear indevidamente o progresso de outras contas. Falhas
antes do commit preservam o cursor e a fila anteriores; um commit concluído
conserva os eventos mesmo após interrupção. Se o resultado do commit for
ambíguo, a próxima tentativa verifica o estado persistido antes de repetir.

Entregas são pelo menos uma vez: consultar a fila não remove eventos; somente
confirmação explícita e idempotente pelo consumidor os retira. O consumidor
deduplica pelo ID estável. Mudar a pasta cria um escopo distinto e não apaga
automaticamente a fila do escopo anterior. Token inválido ou estado
incompatível exige intervenção ou reconstrução explícita que preserve eventos
pendentes; não reinicializar silenciosamente.

## Oráculos de aceitação

Usar provedor simulado e filesystem/SQLite isolados. Exercitar inventário vazio
e preexistente, mudanças durante a inicialização, criação, edição, renomeação,
movimentos para dentro e fora, tombstones, pastas e descendentes, mudanças
externas, múltiplas mudanças coalescidas, paginação e nenhuma alteração.
Verificar ainda falhas de conexão, pesquisa incompleta, repetição de token,
interrupções antes e depois do commit, confirmação de entrega e reinício,
concorrência entre consultas, duas contas, múltiplas raízes, token rejeitado e
mudança de configuração. Em cada falha conferir cursor, inventário e eventos,
além do resultado tipado; não basta observar que o processo não falhou.

Testes reais, separados dos automatizados, usam somente conta e pasta de teste
isoladas com autorização adequada. A pessoa executa no Drive criação, edição,
renomeação, movimentação e exclusão de arquivos dispensáveis, e compara cada
estado observado com diagnóstico sanitizado. Não registrar tokens nem dados
pessoais. O [roteiro operacional](../../../docs/remote-detector-diagnostic.md)
descreve o comando e a execução manual.

## Limitações da implementação

Cada verificação faz uma nova varredura da árvore e exige um intervalo sem
alterações no registro inteiro do corpus; atividade contínua pode produzir
resultado incompleto. O limite é de 10.000 requisições, 100.000 itens e oito
rodadas por verificação. Pastas sem capacidade confirmada de listar filhos
interrompem a verificação. Um token rejeitado requer intervenção; não há
reconstrução automática. Uma resposta HTTP 400 genérica não comprova que o
cursor seja inválido. O acompanhamento de uma pasta em drive compartilhado
requer selecionar explicitamente o registro do drive e acesso de membro; o
caso de acesso individual por não membro não está implementado. A perda de
acesso ou mudança de corpus da raiz bloqueia a verificação em vez de trocar
de registro silenciosamente.
