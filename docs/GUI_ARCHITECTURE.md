# Arquitetura da interface gráfica — 2026-11-11

## Janela única e áreas da interface

O aplicativo tem uma janela principal. Ela apresenta o **Setup** ou o
**Dashboard**. O Setup ocupa essa própria janela: não abre outro diálogo de
configuração por cima dela e não possui tela intermediária de contas pendentes.
Preferências continuam sendo uma página auxiliar do Dashboard.

> Componentes devem ser independentes em sua implementação, explícitos em suas
> interfaces e limitados em suas responsabilidades. Páginas compõem componentes;
> controladores coordenam ações; serviços e engine executam operações.

```text
src/
├── main.rs
├── application.rs
├── application/
│   ├── accounts.rs
│   ├── lifecycle.rs
│   ├── navigation/
│   └── background/
├── core/
├── ui/
│   ├── components/
│   ├── setup/
│   │   ├── dialog.rs
│   │   ├── controller.rs
│   │   ├── models.rs
│   │   ├── components/
│   │   └── pages/
│   ├── dashboard/
│   │   ├── window.rs
│   │   ├── controller.rs
│   │   ├── components/
│   │   ├── pages/
│   │   └── restoration/
│   └── preferences/
└── services/
    ├── accounts.rs
    ├── setup.rs
    ├── preferences.rs
    └── background.rs
```

`application.rs` organiza inicialização e ciclo de vida. A navegação central
revalida as configurações; o controle de background apresenta a mesma janela
quando o aplicativo é ativado. Inicialização oculta pode montar o Setup sem
apresentar a janela. Não é criada uma janela adicional para esse caso.

`setup/dialog.rs` constrói conteúdo e controles do assistente, sem criar uma
janela GTK. Seu controlador monta esse conteúdo no `ToastOverlay` existente e
retém a navegação enquanto ela estiver fora da árvore visível. Durante o Setup,
ações de preferências e recargas do Dashboard não modificam essa navegação oculta.

Os serviços executam integração com a API pública da engine, GIO, preferências e
transporte do portal, sem construir widgets. Credenciais permanecem na engine.

## Componentes e redesenho

Cada componente mantém privados seus widgets, imagens e detalhes de layout.
Expõe somente sua raiz para composição, dados de apresentação e eventos com
significado. Controladores não procuram widgets por nome nem manipulam a árvore
de outro componente. Identificadores de widgets nos testes não são contratos de
produção.

- `ProviderChoice`: recebe metadados e seleção; emite identificador e estado
  marcado/desmarcado. Atualizações de apresentação não emitem seleção do usuário.
- `FolderChoice`: recebe caminho e disponibilidade; emite pedido de escolher pasta.
- `Summary`: apresenta provedor, identidade e pasta, sem executar verificações.
- `AccountDetails`: emite editar, salvar, cancelar, reautorizar e desconectar; seu controlador recebe um handle de apresentação com referências fracas.
- `ConnectionView`: apresenta a reautorização independente, por eventos e um
  handle que não expõe seus botões e rótulos.

As páginas compõem os componentes de cada etapa. Um redesenho de layout, cores ou
imagens que preserve esses contratos não exige alterar seus consumidores ou
outros componentes. Estilos específicos permanecem no componente correspondente.

## As quatro páginas

Com exceção da seleção inicial de provedor, as etapas compartilham um cabeçalho
vertical: área de ilustração de 184 pixels lógicos, título centralizado e descrição.
As margens, a largura de conteúdo e as posições do cabeçalho são comuns; os
controles específicos seguem abaixo. A troca de página usa uma transição suave
de 180 ms e respeita a preferência de animações do GTK.

As etapas compartilham o tamanho mínimo de 760 × 800 pixels lógicos. A janela
pode ser ampliada pelo usuário, e a navegação não redefine seu tamanho. Esse
mínimo pertence ao conteúdo do Setup e deixa de valer quando ele é removido.

| Página | Comportamento |
| --- | --- |
| Escolher provedor | Solicita ao cadastro público de Providers os provedores disponíveis. A UI não mantém uma lista alternativa. Desmarcar a seleção bloqueia Continuar. |
| Conectar conta | Centraliza o ícone e nome do provedor, explicação de acesso, botão Entrar com Google e aviso de segurança. Durante a autorização permite cancelar e copiar o link disponível. Continuar aparece após a autorização. A conta permanece provisória durante o Setup. |
| Configurar sincronização | Escolhe a pasta local e verifica existência, tipo e permissão anunciada de escrita. As escolhas ficam apenas em memória. A seleção remota ainda está indisponível. |
| Iniciar sincronização | Exibe o resumo. O botão Iniciar sincronização permanece bloqueado enquanto os requisitos não forem atendidos. Não há atalho para abrir o Dashboard. |

A consulta de provedores é local e infalível no contrato atual. Publica somente
implementações cadastradas, atualmente Google, com identificador, nome, ícone e
capacidade de conexão. O campo `icon_svg: &'static str` contém o SVG incorporado
por `include_str!`; o recurso Google fica em
`engine/src/providers/google/assets/drive.svg`. A UI renderiza esse conteúdo como
ícone escalável tanto na escolha do provedor quanto no seletor de pastas remotas,
sem depender de caminhos instalados ou do tema de ícones. Operações específicas
permanecem no módulo do provedor.

O seletor local não cria pastas nem altera arquivos. Sua consulta de permissão
não demonstra acesso durável pelo portal nem substitui as verificações da futura
engine de sincronização. Nenhuma mensagem afirma que arquivos ou conflitos foram
analisados quando essa análise não existe.

## Descartar Setup incompleto

Não há retomada de configuração incompleta. O comportamento é:

1. Abrir o Setup, selecionar o provedor, conectar e escolher as pastas.
2. Manter as escolhas apenas durante essa sessão do assistente.
3. Ao fechar sem concluir, cancelar a autenticação em andamento e aguardar seu
   resultado se uma persistência de credenciais já tiver começado.
4. Remover credenciais locais e cadastro das contas provisórias e descartar seus
   eventuais rascunhos antigos.
5. Somente então fechar a janela e sair. Na próxima abertura, começar na primeira
   página, sem provedor selecionado e sem conta conectada.

Fechar não solicita uma segunda confirmação de desconexão. A ação **Sair** e
Ctrl+Q passam pelo mesmo fechamento do Setup. A política de manter o aplicativo
em background não intercepta esse fechamento. O controlador mantém a aplicação
ativa até terminar o cancelamento e a limpeza.

A API atual da engine persiste a autorização quando a autenticação termina.
Durante o Setup esse cadastro é provisório: a UI acompanha as identidades e as
remove ao abandonar o fluxo. Uma resposta de autenticação que chegar depois do
pedido de fechamento também entra nessa limpeza. Antes de declarar o logoff
concluído, o controlador consulta registros incompletos que uma falha de
persistência possa ter deixado sem retornar seu identificador.

Se a remoção ou a limpeza falhar, a janela permanece aberta com o erro e permite
repetir o fechamento. Não informa que a conta foi desconectada sem confirmação da
engine. Após encerramento abrupto, ou ao encontrar registros de versões antigas,
a inicialização descarta as contas incompletas antes de montar um Setup novo;
não apresenta uma lista para retomar esses registros.

O código de produção não grava novos rascunhos. O leitor do formato legado em
`$XDG_CONFIG_HOME/drivesync/setup/<id-codificado>.ini` existe somente para
verificação e descarte desses arquivos antigos. O gravador fica restrito aos
fixtures de teste de migração. A codificação do identificador impede formar
caminhos arbitrários; a limpeza não percorre os diretórios pessoais escolhidos.

## Desconexão e concorrência

**Desconectar conta…** continua disponível no Setup. A ação explícita solicita
confirmação e, após sucesso, volta à primeira página na mesma janela. Não cria uma
tela intermediária. O componente de detalhes do Dashboard também possui essa ação.

`services/accounts.rs` chama `Engine::disconnect_account`, reutilizando a
transação de Accounts para apagar credenciais locais e metadados. O serviço remove
somente o rascunho da conta. Os arquivos locais e na nuvem são preservados; não há
revogação global de autorização no Google.

Tentativas de autenticação mantêm uma licença compartilhada de leitura; a remoção
exige exclusão de escrita. A engine recusa remover uma conta enquanto uma tentativa
ativa ainda pode persistir sua identidade, inclusive uma adição de identidade
ainda desconhecida. O fechamento do Setup aguarda o término de sua própria
tentativa antes de remover a conta. Remoção de cadastro já ausente é idempotente.

## Dashboard e limites atuais

A navegação exige autorização e validação dos dois diretórios para construir o
Dashboard. Como a API de configuração remota ainda não existe, essa validação é
indisponível e o botão final permanece desativado. A engine também ainda não
oferece reconciliação nem execução de sincronização pela API pública de conexão.
Os escopos de autenticação atuais são de identidade.

A implementação futura precisará validar e persistir a configuração completa,
aceitar o início da sincronização e finalizar a sessão provisória antes de
apresentar o Dashboard. A engine atual não possui associações de sincronização
executáveis; essas associações precisarão integrar o contrato de exclusão de
remoção quando forem implementadas.

## Validação

O teste visual isolado verifica as quatro páginas, tamanho reduzido, tema escuro,
seleção e desmarcação, uma única janela visível, o botão final bloqueado, fechamento
antes de conectar, descarte após conectar, desconexão explícita e retorno à primeira
página. Também cobre erro de limpeza e nova tentativa, resultado tardio de
autenticação simulado e descarte de dados legados em inicialização oculta.

Os testes da engine usam vault e respostas HTTP isolados para verificar remoção de
contas populadas, exclusão contra autenticação, idempotência e preservação de outra
conta e de um arquivo local. A tabela de roteamento cobre as combinações de
autorização e validação das duas pastas.

As verificações Rust são executadas no SDK do manifesto Flatpak. Os fixtures
visuais usam Broadway, conta fictícia e diretórios XDG isolados; não acessam Google,
keyring pessoal ou portais de autostart. O seletor real do desktop e acessibilidade
assistiva precisam de validação no aplicativo instalado. Veja
[capturas](../data/screenshots/README.md).
