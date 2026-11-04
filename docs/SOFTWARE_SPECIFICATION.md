# Especificação de software — DriveSync

## Escopo

DriveSync é um aplicativo Linux experimental para autenticar e gerenciar contas
Google e escolher uma pasta local e uma pasta do Google Drive para cada conta.
A interface usa GTK4/Libadwaita e o aplicativo é distribuído como Flatpak.

O aplicativo **não sincroniza, transfere ou exclui arquivos**, nem resolve
conflitos. Selecionar pastas não inicia operações sobre seu conteúdo. Um
diagnóstico explícito consulta metadados e alterações remotas; não há
monitoramento automático. DriveSync não é um backup.

## Autenticação e contas

- Conectar uma conta requer OAuth 2.0 para aplicativo desktop, navegador externo,
  PKCE e callback em loopback. O aplicativo não solicita a senha Google.
- A identidade é vinculada ao `sub` do Google, não ao e-mail exibido. E-mail
  verificado pode estar ausente. As credenciais ficam no Secret Service,
  isoladas por aplicativo, cliente OAuth e conta; tokens não são exibidos nem
  registrados em logs.
- O fluxo solicita `openid`, `email` e o escopo completo `drive`. Este último é
  amplo e restrito: concede mais acesso do que a listagem de pastas utilizada
  atualmente. A escolha de um escopo menor e a verificação Google exigem
  avaliação própria; não presumir que o token seja somente leitura.
- A renovação e o acesso usam credenciais da conta selecionada. Falhas de rede,
  cofre ou autenticação são reportadas sem declarar que um perfil salvo está
  autenticado. Somente a ausência confirmada de credenciais permite descartar
  automaticamente um perfil; erros do cofre não comprovam essa ausência.
- O usuário pode adicionar contas, alternar entre elas, editar uma descrição
  local e sair de uma conta. A saída é uma ação explícita que remove a
  credencial local e o perfil correspondente, sem apagar arquivos locais ou
  remotos nem afetar as demais contas. Ela não revoga o acesso no Google.

## Seleção e persistência de pastas

- A seleção remota lista pastas por conta, com paginação e IDs validados.
  Informações e erros da API são tratados sem exibir respostas privadas.
- A seleção local exige uma pasta absoluta, existente e gravável, diferente da
  raiz do filesystem. A verificação de escrita cria e remove um arquivo
  temporário exclusivo; não altera conteúdo do usuário.
- Caminhos locais são normalizados e origens do portal Flatpak são verificadas.
  Pastas locais iguais ou aninhadas entre contas ou clientes OAuth não são
  aceitas. Não há correspondência por simples prefixo textual.
- O índice local mantém perfis, descrições e associações de pastas, sem
  credenciais. Escritas são serializadas entre processos e publicadas por
  substituição atômica. Dados inválidos ou incompatíveis são rejeitados, não
  apagados ou reinicializados automaticamente.
- Nenhum estado anterior de sincronização é lido, migrado, recuperado ou
  removido. Arquivos e diretórios existentes, inclusive metadados de versões
  anteriores, permanecem no lugar.

## Interface e arquitetura

A aplicação compõe as funcionalidades de contas e pastas usando a fachada de
`drivesync-engine`; a biblioteca não depende de GTK. Trabalho bloqueante de
rede e cofre é executado fora da thread gráfica. O dashboard apresenta a conta
selecionada, suas pastas e ações, sem controles de sincronização.

Preferências globais permitem configurar aparência e continuidade em segundo
plano quando autorizada pelo ambiente de desktop. Permanecer em segundo plano
não inicia transferência de arquivos. A estrutura e os perfis Flatpak são
descritos em [GUI architecture](gui-architecture.md) e nos
[manifestos](../flatpak/).

## Segurança e verificação

Nenhuma operação automática pode apagar arquivos locais, remotos ou
credenciais. A saída solicitada pelo usuário é a única operação desta interface
que remove a credencial local da conta correspondente. Falhas devem permanecer
visíveis e não podem ser apresentadas como sucesso.

Os testes automatizados usam dados fictícios, serviços simulados e diretórios
isolados. Compilação, lint, testes e documentação Rust devem ser executados no
SDK e na extensão definidos pelo manifesto de desenvolvimento. Testes locais
não comprovam OAuth ou Google Drive reais, portal de documentos, Secret Service
do usuário, nem funcionamento do Flatpak instalado. Ensaios reais exigem
ambiente isolado e autorização apropriada.

Contratos detalhados: [autenticação](../engine/src/auth/contract.md),
[contas](../engine/src/accounts/contract.md),
[configuração](../engine/src/configuration/contract.md) e
[pastas remotas](../engine/src/remote/contract.md).

O [contrato do detector remoto](../engine/src/remote_detector/contract.md)
define suas garantias e limitações. O diagnóstico é invocado manualmente; a
validação com Google Drive real e o aplicativo empacotado ainda é necessária.

O [contrato do detector local](../engine/src/local_detector/contract.md)
define um observador de atividade separado da investigação e da sincronização.
Avisos identificam escopo e caminhos afetados; perda de cobertura exige
conferência explícita. Árvore por identidade, download, investigação de conteúdo
e execução remota pertencem a etapas posteriores. O observador e o diagnóstico
independente estão implementados, com fila limitada em memória e falhas de
cobertura explícitas. O dashboard inicia observadores por conta e exibe um
histórico temporário de nomes, limitado a 200 registros. Troca de pasta reinicia
a sessão; descarte da página e encerramento cancelam a observação. O portal/FUSE
não possui cobertura estabelecida e produz estado incompleto; iniciar a
observação não comprova entrega de eventos do host.
