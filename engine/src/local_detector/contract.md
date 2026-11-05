# Detector de atividade local

## Escopo aprovado

Observar atividade na raiz configurada e nos diretórios descendentes enquanto
uma sessão estiver ativa. Entregar pistas identificando o escopo (cliente,
conta e raiz), caminhos relativos sem perda de bytes, tipo conhecido e operação
observada. Não ler conteúdo, calcular hashes, reconstruir inventários, associar
IDs do Drive nem executar sincronização. Escrita não prova conteúdo diferente;
remoção não autoriza exclusão remota. Identidade, investigação e execução são
responsabilidades posteriores.

## Modelo e entrega

Uma sessão possui escopo imutável e instância própria de inotify. Os avisos são
criação, remoção, escrita, fechamento após abertura para escrita, metadados,
entrada, saída, movimento correlacionado e necessidade de conferência.
Movimentos são correlacionados pelo cookie do kernel durante uma janela limitada;
origem e destino são caminhos históricos, não identidade persistente.
O tipo distingue pasta, arquivo regular, link, especial e desconhecido.
Pastas usam a indicação do kernel; outros tipos usam descritores `O_PATH`
somente para metadados, sem seguir links ou atravessar montagens. Remoções e
saídas sem metadados preservados têm tipo desconhecido.
Essas pistas não comprovam continuidade de identidade sob substituição.
Renomear e mover usam a mesma operação; nenhum deles exige leitura de conteúdo.

Fila em memória, limitada, em ordem de processamento. `poll` coleta um lote
limitado e transfere os avisos ao consumidor (sem confirmação persistente).
Não há promessa de entrega após queda do processo. Overflow da fila da aplicação
ou do kernel sinaliza perda explicitamente. Não agrupar avisos na aplicação;
o kernel já pode agrupá-los. Limites de watches (inclusive IDs removidos aguardando confirmação do kernel), eventos por lote e movimentos
pendentes são configuráveis e finitos.

## Cobertura e segurança

Estados: inicializando, observando, incompleto e parado. Incompleto é persistente
na sessão; criar outra sessão reinstala os watches, mas não recupera histórico.
Toda inicialização emite necessidade de conferência: nada prova o estado anterior
ou recupera mudanças enquanto parado. Subárvore nova/recebida também exige
conferência: seus descendentes podem ter mudado antes da instalação dos watches.
O observador não pode declarar sincronização concluída.

Usar descritores de diretórios com resolução `openat2`, sem links simbólicos,
sem atravessar montagens internas (incluindo bind mounts). Não abrir conteúdo
nem arquivos especiais. A raiz não pode ser `/`, relativa ou conter componentes
`.`/`..`; não seguir links em componentes da raiz. Verificar substituição da
raiz antes e depois de cada lote. Verificar o diretório associado ao watch
antes de atribuir caminhos a eventos: divergência produz cobertura incompleta.
Descritores fixam objetos; não congelam a árvore. Renomeações concorrentes
podem invalidar caminhos, portanto avisos não comprovam pertencimento atual.
A exigência anterior de manter conteúdo dentro da raiz durante toda leitura
não se aplica ao observador, que não lê conteúdo; o contrato do futuro
investigador de conteúdo deverá ser aprovado separadamente.

Overflow, perda de watch, inacessibilidade, montagem interna, limites e corridas
produzem conferência explícita. Perda da raiz para a sessão; demais falhas
mantêm os watches restantes em estado incompleto. Remover watches de subárvores
que saíram e atualizar caminhos dos watches em movimentos internos. Durante
movimento pendente de diretório, não atribuir eventos descendentes a caminhos
possivelmente antigos: exigir conferência. Filesystems fora de ext, Btrfs, XFS, tmpfs e overlay iniciam incompletos
com `FilesystemUnverified`, incluindo FUSE do portal de documentos. Essa lista
não prova cobertura total: mmap e operações por links externos continuam sendo
pontos cegos. O teste no portal deve distinguir início da sessão de entrega real
de notificações originadas no host.

## Algoritmo

1. Validar escopo e limites; abrir raiz e inotify não bloqueante.
2. Instalar watch antes de enumerar cada diretório; percorrer somente
   diretórios abertos relativamente à raiz com restrições de resolução.
3. Enfileirar conferência inicial; coletar eventos em lotes limitados.
4. Resolver watch/caminho e correlacionar cookies; atualizar ou remover watches
   de subárvores e instalar watches em diretórios novos/recebidos.
5. Emitir avisos conservadores; expirar movimentos sem par como saídas.
6. Ao perder cobertura, emitir motivo e caminho; nunca tratar ausência de
   aviso como prova de ausência de mudança. Encerrar descarta watches e fila.

## Critérios de aceite

Testes com filesystem temporário real: criação, escrita, substituição atômica,
remoção, renomeação, movimentos internos/externos e subárvores; nomes não UTF-8,
links, isolamento e reinício. Testar deterministicamente overflow, perda de
watch, inacessibilidade e limites. Garantir trabalho/memória limitados e ausência
de escrita nos arquivos observados. Diagnóstico independente imprime avisos
sem acessar credenciais. Verificação Rust no SDK GNOME 50; acesso pelo portal e
Flatpak instalado são validações separadas, com resultados explicitados.

## Fundamentação

- [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html): cookies,
  fila ordenada, agrupamento, overflow, recursão e corridas de instalação.
- [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html): resolução
  relativa, NO_SYMLINKS, BENEATH e NO_XDEV; sem fallback que enfraqueça restrições.
- [rustix inotify](https://docs.rs/rustix/latest/rustix/fs/inotify/index.html):
  interface segura às operações Linux, reutilizando a dependência existente.
- [Linux filesystem magic](https://github.com/torvalds/linux/blob/master/include/uapi/linux/magic.h): identificação conservadora do backend; não prova cobertura.
- [Flatpak](https://docs.flatpak.org/en/latest/sandbox-permissions.html): acesso
  concedido à pasta não implica cobertura comprovada pelo portal.

Referências verificadas em 2026-11-04. O registro de árvore, versões anteriores,
identidades do Drive, investigação de conteúdo, reconciliação periódica e fila
durável são requisitos de etapas futuras, não funções deste observador.

## Integração desktop e feedback temporário

A página de cada conta inicia uma sessão para a pasta configurada sem depender
de rede. Trabalho de filesystem ocorre em worker, fora da thread GTK. A troca
de conta selecionada não interrompe outras contas. Trocar a configuração reinicia
a sessão correspondente e limpa o histórico; descartar a página ou encerrar o
aplicativo cancela seus workers. A fila entre worker e interface é limitada;
backpressure não descarta lotes silenciosamente.

O histórico temporário exibe o nome, tipo observado e atividade de cada item, em ordem
mais recente primeiro, sem afirmar qual operação definitiva ocorreu. Avisos
repetidos podem gerar nomes repetidos. Limite de 200 linhas por conta; itens
mais antigos são retirados, com indicação de que são apenas os mais recentes.
Tipos desconhecidos são apresentados como itens, sem inventar classificação.
Escritas e fechamentos não são apresentados como confirmação de salvamento.
Caminhos completos, timestamps e conteúdo não aparecem nas linhas. Uma mudança
de pasta reinicia esse feedback para não misturar raízes. Cobertura incompleta,
falha e ausência de configuração ficam visíveis fora da lista. Não persistir
esse histórico nem iniciar transferências. Reconstruir o dashboard também
reinicia o feedback, sem prometer histórico entre sessões.

Para configurações salvas por portal, a integração consulta novamente a origem
no portal e compara com a origem salva. Se houver acesso direto de leitura já
concedido a essa mesma pasta, o observador o utiliza sem regravar configurações
nem criar arquivos de teste. Sem essa permissão, permanece no portal e sinaliza
suas limitações. O aplicativo não amplia permissões automaticamente.

A integração desktop grava os lotes no [registro local](../local_state/contract.md)
antes da entrega à interface. A fila do observador permanece volátil; somente
lotes confirmados no registro são duráveis. Startup exige conferência para
mudanças ocorridas antes desse commit ou durante uma interrupção. O histórico
visual não é a fila de trabalho persistente.
