# Arquitetura da interface gráfica — 2026-11-11

## Organização e fronteiras

A interface GTK4/Libadwaita possui duas áreas principais: o diálogo **Setup**,
para a configuração inicial de uma conta, e a janela **Dashboard**. Preferências
continuam sendo uma página auxiliar da janela principal.

> Componentes devem ser independentes em sua implementação, explícitos em suas
> interfaces e limitados em suas responsabilidades. Páginas compõem componentes;
> controladores coordenam ações; serviços e engine executam operações.

```text
src/
├── main.rs
├── application.rs
├── application/
│   ├── lifecycle.rs
│   ├── navigation/
│   └── background/
├── core/
│   ├── build_info.rs
│   ├── i18n.rs
│   └── messages.rs
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

`application.rs` organiza inicialização e ciclo de vida. Seus submódulos tratam
roteamento e a reação da aplicação a permissões do desktop. `core` mantém os
auxiliares existentes de tradução, versão e mensagens sanitizadas.

Os serviços não constroem widgets: executam integração com a API pública da engine,
GIO, persistência de preferências e transporte do portal. Decisões de mostrar,
ocultar ou fechar janelas pertencem à aplicação e à apresentação.

## Contratos de componentes e redesenho

Cada componente mantém privados seus widgets, imagens e detalhes de layout.
Expõe sua raiz para composição, métodos de apresentação com significado e eventos
com dados. Controladores não procuram widgets por nome nem dependem da estrutura
de filhos. Identificadores de widgets usados nos testes não são contratos de
produção.

Exemplos:

- `ProviderChoice`: recebe metadados do catálogo e seleção; emite `ProviderId`.
- `FolderChoice`: recebe caminho e disponibilidade; emite pedido de escolher pasta.
- `Summary`: recebe provedor, identidade e pasta; apresenta um resumo somente leitura.
- `AccountDetails`: emite editar, salvar nome, cancelar, reautorizar e configurar;
  recebe atualizações por um handle de apresentação com referências fracas.
- `ConnectionView`: expõe ações de autenticação, cópia e fechamento, além de um
  handle de apresentação; o controlador desconhece seus botões e rótulos.

As páginas do Setup compõem os componentes específicos de cada etapa. O diálogo
compõe as páginas e apresenta `SetupState`; seu controlador coordena eventos e
serviços. O controlador do Dashboard recebe um evento de atualização da conta e a
página responsável pela sidebar atualiza sua própria apresentação.

Um redesenho de layout, cores ou imagens que preserve esses contratos não exige
mudanças nos consumidores ou em outros componentes. Classes nativas de estilo são
aplicadas aos widgets de cada componente; não foram introduzidos seletores globais
que dependam da árvore interna de outro componente.

## Setup

| Página | Comportamento implementado |
| --- | --- |
| Escolher provedor | Consulta `services::setup::list_providers`, que delega ao cadastro público `providers::list_providers`. A UI não mantém outra lista. Lista vazia tem apresentação explícita. |
| Conectar conta | Inicia autenticação Google pelo navegador, permite copiar o link, cancelar e tentar novamente. O sucesso permanece no Setup. |
| Configurar sincronização | Permite escolher uma pasta local pelo seletor nativo e verificar que ela é um diretório acessível naquele momento. Salva a escolha como rascunho por conta. |
| Iniciar sincronização (revisão) | Exibe conta, provedor e pasta escolhida. Informa que configuração remota e verificações de arquivos ainda não foram realizadas. |

A consulta de provedores é local e infalível no contrato atual; não há chamada de
rede, autenticação nem catálogo alternativo na UI. O catálogo publica apenas
implementações cadastradas, atualmente Google, com identificador, nome, ícone e
capacidade de conexão. Operações específicas permanecem no módulo do provedor.

### Navegação, cancelamento e retomada

- Sem contas, o Setup é oferecido quando a janela principal estiver visível.
  Inicialização oculta em background não apresenta um diálogo por conta própria.
- Adicionar conta abre ou focaliza a instância existente do Setup.
- Voltar preserva escolhas compatíveis; trocar provedor invalida identidade e
  rascunho dependentes na sessão, sem remover uma conta já persistida.
- Operações em andamento bloqueiam avanço e retorno. Autenticação possui ação
  específica de cancelamento e uma geração para invalidar retornos cancelados.
- Fechar solicita cancelamento da autenticação. Uma persistência já iniciada pode
  terminar e ser refletida no Dashboard; não existe promessa de rollback.
- Conta autenticada já foi salva pela engine, mesmo sem concluir configuração.
  O Dashboard oferece **Configurar sincronização…** para retomar seu rascunho.
- Contas que exigem reautorização retomam a página de conexão; a chamada passa o
  `AccountId` existente e preserva a exigência da mesma identidade na engine.
- A ação independente **Autenticar novamente** permanece disponível no Dashboard.

### Persistência e limites atuais

Rascunhos ficam em `$XDG_CONFIG_HOME/drivesync/setup/<id-codificado>.ini`.
O identificador é codificado byte a byte para não formar caminhos arbitrários.
A URI da pasta preserva nomes não UTF-8; não há tokens, senhas ou flags de
configuração concluída nesse arquivo. Gravações são assíncronas, atômicas e
privadas. Erros de leitura e gravação são apresentados e preservam a escolha
editável; a ausência de arquivo representa um rascunho novo.

**Esta entrega remodela a interface; não implementa a engine de sincronização.**
A API pública atual de conexão oferece autenticação e cadastro de contas, sem
contratos de configuração remota, reconciliação ou execução de sincronização.
Os escopos atuais de conexão são de identidade. O seletor local não cria pastas,
não altera arquivos e não demonstra permissão durável de escrita pelo portal.

Por isso, a seleção remota aparece como indisponível e **Iniciar sincronização**
permanece desativado. **Ver conta no painel** fecha o diálogo e seleciona a conta
no Dashboard, sem declarar que a sincronização começou. Nenhuma mensagem afirma
que arquivos foram verificados ou que conflitos estão ausentes.

A conclusão funcional das páginas 3 e 4 depende de contratos da engine para:

1. Enumerar e validar pastas remotas com autorização adequada.
2. Validar e persistir a configuração completa e o acesso local necessário.
3. Analisar os arquivos quando houver uma verificação anunciada na revisão.
4. Aceitar uma solicitação de início e disponibilizar seu estado ao Dashboard.

## Validação

Há testes de estado para navegação, autorização e invalidação por provedor; testes
de serialização e isolamento do caminho dos rascunhos; e teste visual isolado que
percorre as quatro páginas, exercita seleção real, instância única, retomada de
rascunho, revisão, conta que exige login e abertura após startup oculto.

As capturas usam conta fictícia, diretórios XDG isolados e backend Broadway.
Incluem uma janela reduzida e tema escuro. Não acessam Google real, keyring pessoal
nem portais de autostart. O seletor real do desktop e acessibilidade assistiva
exigem validação no aplicativo instalado. Veja [capturas](../data/screenshots/README.md).
