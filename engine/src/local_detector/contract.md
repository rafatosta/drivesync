# Contrato proposto — detector de alterações locais

**Estado: requisitos aprovados; implementação parcial.** A aprovação deste
contrato não declara o detector remoto validado nem autoriza sincronização.

## Responsabilidade

O detector observa as pastas locais configuradas, mantém inventários separados
por cliente OAuth, conta e raiz e entrega observações para reavaliação. Não
transfere, renomeia, substitui ou exclui arquivos do usuário; não decide
operações nem resolve conflitos. A engine não depende da interface gráfica.

O monitoramento pretendido funciona enquanto o aplicativo estiver em execução,
inclusive em segundo plano quando autorizado pelo ambiente. Ao iniciar ou
retomar, o detector compara o inventário persistido com o estado atual para
identificar diferenças ocorridas durante a interrupção. Isso **não recupera
um histórico completo**: por exemplo, um arquivo criado e apagado enquanto
o aplicativo estava fechado pode não deixar vestígios observáveis.

## Fontes e limitações técnicas

- [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html): as
  notificações podem ser agrupadas ou perdidas por overflow; a monitoração de
  árvores exige acompanhar diretórios individualmente. Eventos são pistas de
  mudança, não um diário durável nem uma captura atômica.
- [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html): a
  resolução relativa a um diretório e as restrições de links podem ajudar a
  impedir que percursos saiam da raiz. A disponibilidade no kernel e no
  sandbox exige verificação; nenhuma alternativa pode enfraquecer o limite.
- [Permissões Flatpak](https://docs.flatpak.org/en/latest/sandbox-permissions.html):
  acesso a pastas fora do sandbox depende da concessão correspondente. Acesso
  por portal e notificações do filesystem precisam de validação no aplicativo
  instalado; não presumir equivalência com acesso direto no host.
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html): uma
  transação publica alterações em conjunto sob as premissas do SQLite,
  filesystem e hardware; isso não substitui ensaios de interrupção.

Essas referências foram consultadas em 2026-11-04. Sistemas de arquivos de
rede, FUSE, montagens e alterações via `mmap` podem não emitir todas as
notificações esperadas. A aplicação deve usar reconciliações e reportar
explicitamente quando não consegue estabelecer um resultado confiável.

## Escopo e observações

- Entrada: cliente, conta, pasta local configurada e configuração do
  monitoramento. A raiz e suas subpastas pertencem somente ao escopo
  correspondente; alterar a seleção cria outro escopo sem apagar sua fila
  anterior. Não usar dados da engine antiga.
- O inventário inicial descreve os itens preexistentes e um marcador de
  conclusão; não os classifica como criações. Observações posteriores
  incluem identidade de entrega estável, caminho relativo representável sem
  perda, tipo, metadados disponíveis e estado anterior e atual quando
  conhecidos. Mudanças cuja operação ou identidade não possa ser determinada
  com segurança são entregues para reavaliação.
- Arquivos regulares são comparados também por hash de conteúdo. A leitura
  pode ter custo significativo e alterar o horário de acesso; não modifica
  intencionalmente conteúdo, permissões nem timestamps. Um arquivo que muda
  durante a leitura não deve produzir um hash tratado como versão estável.
- Links simbólicos são inventariados como links para reavaliação, sem seguir
  seus destinos. A resolução e a observação devem permanecer dentro da raiz
  mesmo sob renomeação ou substituição concorrente de componentes. Arquivos
  especiais não são abertos como arquivos regulares.
- Não atravessar pontos de montagem dentro da raiz; uma subárvore montada
  torna a verificação incompleta. Comparar apenas IDs de dispositivo não basta
  para detectar todos os bind mounts; a técnica e sua disponibilidade no
  Flatpak precisam ser demonstradas antes de declarar cobertura completa.
- Uma identidade baseada em inode e dispositivo pode ser reutilizada ou
  representar múltiplos hard links; ela não prova por si só continuidade ou
  renomeação. Ausência, acesso negado e falhas de leitura não autorizam
  inferir exclusão ou decidir uma operação.

## Monitoramento e reconciliação

O mecanismo concreto deve ser validado antes da integração: notificações
locais são usadas para antecipar verificações, e reconciliações completas
recuperam diferenças de estado. Uma reconciliação periódica também é necessária
para mudanças que não gerem notificações: quando o aplicativo estiver ativo,
ela é agendada a cada 15 minutos, além das verificações motivadas por
notificações. O agendamento não garante conclusão em 15 minutos quando uma
varredura é demorada ou falha. Limites de trabalho e política de consumo de
recursos ainda precisam ser definidos e testados.
A comparação pura entre inventários estruturalmente válidos e o cálculo
limitado de hash de um arquivo regular já aberto existem isoladamente. O
cálculo compara metadados antes e depois da leitura, sem garantir um snapshot
nem verificar se o caminho ainda identifica o arquivo; essas garantias
permanecem sob responsabilidade do futuro varredor. Um diário SQLite isolado
persiste inventários e observações transacionalmente, mas não consegue
verificar a completude de uma varredura nem o vínculo da raiz ao caminho.
Não há varredura nem monitoramento implementados.

Pseudocódigo de alto nível:

1. Abrir a raiz configurada com acesso apropriado e verificar sua identidade.
   Preparar o monitoramento antes e durante a construção do inventário, sem
   seguir links. Se acesso, identidade ou cobertura não puderem ser confirmados,
   reportar falha ou resultado incompleto.
2. Percorrer recursivamente os itens acessíveis e calcular hashes de arquivos
   regulares. Verificar a estabilidade das leituras e da cobertura; reconciliar
   eventos recebidos durante a varredura. Corridas ou mudanças persistentes
   que impeçam uma conclusão segura produzem resultado incompleto, não sucesso.
3. Comparar o inventário concluído com o anterior. Emitir observações
   conservadoras para itens e subárvores afetados; não inventar sequências de
   operações intermediárias.
4. Persistir inventário e eventos em uma transação e publicar apenas depois
   de concluí-la. Retomar o monitoramento; notificações, intervalos periódicos
   ou recuperação de falha disparam nova reconciliação.
5. Se houver overflow, perda de watch, troca de raiz, desconexão de montagem,
   permissão insuficiente ou interrupção, marcar o escopo como incompleto e
   reconciliar antes de voltar a declarar cobertura completa. Falha de um
   escopo não deve interromper os demais.

Estados externos distinguem inicialização, acompanhamento, reconciliação,
incompleto, falha recuperável e intervenção necessária. A interface de
notificação de falhas e o ciclo de vida por conta devem ser definidos antes
da integração com o aplicativo.

## Persistência e entrega

O estado é próprio da nova engine, versionado, privado e isolado por escopo.
Uma transação confirma em conjunto o inventário e seus eventos pendentes;
falhas ou interrupções não podem avançar o inventário sem os eventos
correspondentes. Um resultado de commit ambíguo exige reler o estado antes
de repetir. Dados incompatíveis ou corrompidos são rejeitados sem apagá-los.

Eventos são entregues pelo menos uma vez, com IDs estáveis; somente uma
confirmação explícita e idempotente os remove. O consumidor deduplica pelo ID.
A troca de pasta e o encerramento do aplicativo não eliminam a fila anterior.
Limites de armazenamento, recuperação de disco cheio e concorrência entre
processos devem ser tratados explicitamente no desenho detalhado.

O diário implementado mantém gerações para rejeitar escritores obsoletos e
recusa uma mudança observada de dispositivo ou inode da raiz; essa comparação
não prova identidade contínua. Eventos permanecem na fila até confirmação
individual. A implementação ainda não está integrada a um varredor ou ao
ciclo de vida do aplicativo, e não estabelece limites de armazenamento.

## Oráculos antes da implementação

Testes usam filesystem e SQLite reais isolados, além de uma fonte de eventos
simulada para falhas determinísticas. Devem cobrir:

- inicialização vazia e preexistente, subpastas, nomes não UTF-8 e links;
- criação, edição, substituição atômica, renomeação, movimentação e remoção,
  inclusive subárvores e mudanças durante o inventário;
- mudanças com mesmo tamanho ou timestamps restaurados, leitura concorrente,
  hard links e arquivos especiais, sem seguir links fora da raiz;
- reinício, mudanças enquanto parado, eventos pendentes e confirmação;
- overflow, perda de notificações, mudanças sem evento, limites de watches,
  acesso negado, desmontagem, substituição de raiz e estado incompatível;
- falhas e interrupções antes e depois do commit, concorrência e isolamento
  entre contas e pastas; conferir inventário e fila além do resultado;
- ciclo de vida do aplicativo, acesso efetivo pelo portal/Flatpak e
  monitoramento em segundo plano em ambiente isolado e autorizado.

Os testes devem demonstrar ausência de alterações no conteúdo dos arquivos
observados; mudanças de horário de acesso causadas por leitura devem ser
medidas e documentadas. Não interpretar testes simulados, compilação ou testes
no SDK como validação do aplicativo empacotado.
