# Especificação de Software — DriveSync

## Sumário

- [1. Identificação e controle do documento](#1-identificacao-e-controle-do-documento)
  - [1.1 Identificação do produto](#11-identificacao-do-produto)
  - [1.2 Versão e estado da especificação](#12-versao-e-estado-da-especificacao)
  - [1.3 Histórico de revisões](#13-historico-de-revisoes)
  - [1.4 Fontes e referências](#14-fontes-e-referencias)
- [2. Visão geral do produto](#2-visao-geral-do-produto)
  - [2.1 Problema e motivação](#21-problema-e-motivacao)
  - [2.2 Objetivos do produto](#22-objetivos-do-produto)
  - [2.3 Público-alvo e partes interessadas](#23-publico-alvo-e-partes-interessadas)
  - [2.4 Escopo do produto](#24-escopo-do-produto)
  - [2.5 Funcionalidades incluídas](#25-funcionalidades-incluidas)
  - [2.6 Funcionalidades excluídas e limitações](#26-funcionalidades-excluidas-e-limitacoes)
  - [2.7 Glossário e terminologia](#27-glossario-e-terminologia)
- [3. Contexto e restrições](#3-contexto-e-restricoes)
  - [3.1 Ambiente Linux e diretrizes de design GNOME](#31-ambiente-linux-e-diretrizes-de-design-gnome)
  - [3.2 Sistemas e serviços externos](#32-sistemas-e-servicos-externos)
  - [3.3 Google Drive API e OAuth 2.0](#33-google-drive-api-e-oauth-20)
  - [3.4 Sistema de arquivos e armazenamento local](#34-sistema-de-arquivos-e-armazenamento-local)
  - [3.5 Premissas e dependências](#35-premissas-e-dependencias)
  - [3.6 Restrições tecnológicas e operacionais](#36-restricoes-tecnologicas-e-operacionais)
- [4. Requisitos funcionais](#4-requisitos-funcionais)
  - [4.1 Identificação e autenticação](#41-identificacao-e-autenticacao)
  - [4.2 Gestão de contas Google](#42-gestao-de-contas-google)
  - [4.3 Seleção e configuração de pastas](#43-selecao-e-configuracao-de-pastas)
  - [4.4 Descoberta e inventário de arquivos](#44-descoberta-e-inventario-de-arquivos)
  - [4.5 Download e atualização local](#45-download-e-atualizacao-local)
  - [4.6 Upload e criação remota](#46-upload-e-criacao-remota)
  - [4.7 Detecção de mudanças e sincronização](#47-detecção-de-mudanças-e-sincronização)
  - [4.8 Pausa, retomada e execução em segundo plano](#48-pausa-retomada-e-execucao-em-segundo-plano)
  - [4.9 Tratamento de conflitos e exclusões](#49-tratamento-de-conflitos-e-exclusoes)
  - [4.10 Recuperação de falhas e reconciliação](#410-recuperacao-de-falhas-e-reconciliacao)
  - [4.11 Documentos nativos do Google Workspace](#411-documentos-nativos-do-google-workspace)
  - [4.12 Estado, histórico e notificações](#412-estado-historico-e-notificacoes)
  - [4.13 Preferências e gerenciamento da aplicação](#413-preferencias-e-gerenciamento-da-aplicacao)
- [5. Requisitos não funcionais](#5-requisitos-nao-funcionais)
  - [5.1 Segurança e controle de acesso](#51-seguranca-e-controle-de-acesso)
  - [5.2 Privacidade e tratamento de dados](#52-privacidade-e-tratamento-de-dados)
  - [5.3 Integridade e prevenção de perda de dados](#53-integridade-e-prevencao-de-perda-de-dados)
  - [5.4 Confiabilidade, resiliência e disponibilidade](#54-confiabilidade-resiliencia-e-disponibilidade)
  - [5.5 Desempenho e consumo de recursos](#55-desempenho-e-consumo-de-recursos)
  - [5.6 Usabilidade e acessibilidade](#56-usabilidade-e-acessibilidade)
  - [5.7 Compatibilidade e portabilidade](#57-compatibilidade-e-portabilidade)
  - [5.8 Observabilidade e diagnósticos](#58-observabilidade-e-diagnosticos)
  - [5.9 Manutenibilidade e testabilidade](#59-manutenibilidade-e-testabilidade)
- [6. Regras de negócio e políticas operacionais](#6-regras-de-negocio-e-politicas-operacionais)
  - [6.1 Identidade e isolamento de contas](#61-identidade-e-isolamento-de-contas)
  - [6.2 Vinculação de pastas e escopo de sincronização](#62-vinculacao-de-pastas-e-escopo-de-sincronizacao)
  - [6.3 Política de upload e download](#63-politica-de-upload-e-download)
  - [6.4 Política de conflitos e alterações concorrentes](#64-politica-de-conflitos-e-alteracoes-concorrentes)
  - [6.5 Política de exclusões e operações destrutivas](#65-politica-de-exclusoes-e-operacoes-destrutivas)
  - [6.6 Política de falhas, retries e retomadas](#66-politica-de-falhas-retries-e-retomadas)
  - [6.7 Política de consistência e persistência de estado](#67-politica-de-consistencia-e-persistencia-de-estado)
  - [6.8 Limites e formatos de arquivos suportados](#68-limites-e-formatos-de-arquivos-suportados)
- [7. Arquitetura do software](#7-arquitetura-do-software)
  - [7.1 Visão e princípios arquiteturais](#71-visao-e-principios-arquiteturais)
  - [7.2 Componentes e responsabilidades](#72-componentes-e-responsabilidades)
  - [7.3 Motor de sincronização](#73-motor-de-sincronizacao)
  - [7.4 Interface GTK4 e Libadwaita](#74-interface-gtk4-e-libadwaita)
  - [7.5 Comunicação interna e contratos](#75-comunicacao-interna-e-contratos)
  - [7.6 Integração com Google Drive](#76-integracao-com-google-drive)
  - [7.7 Persistência local e cofre de credenciais](#77-persistencia-local-e-cofre-de-credenciais)
  - [7.8 Ciclo de vida e execução do aplicativo](#78-ciclo-de-vida-e-execucao-do-aplicativo)
  - [7.9 Decisões arquiteturais e justificativas](#79-decisoes-arquiteturais-e-justificativas)
- [8. Dados e interfaces](#8-dados-e-interfaces)
  - [8.1 Entidades e identificadores](#81-entidades-e-identificadores)
  - [8.2 Configuração e preferências](#82-configuracao-e-preferencias)
  - [8.3 Estado persistente da sincronização](#83-estado-persistente-da-sincronizacao)
  - [8.4 Interfaces com APIs externas](#84-interfaces-com-apis-externas)
  - [8.5 Interfaces com o sistema operacional](#85-interfaces-com-o-sistema-operacional)
  - [8.6 Interfaces entre módulos](#86-interfaces-entre-modulos)
  - [8.7 Formatos de dados e compatibilidade](#87-formatos-de-dados-e-compatibilidade)
- [9. Experiência e interface do usuário](#9-experiencia-e-interface-do-usuario)
  - [9.1 Fluxo de primeiro acesso](#91-fluxo-de-primeiro-acesso)
  - [9.2 Conexão e gestão de contas](#92-conexao-e-gestao-de-contas)
  - [9.3 Configuração de sincronização](#93-configuracao-de-sincronizacao)
  - [9.4 Painel principal e indicadores](#94-painel-principal-e-indicadores)
  - [9.5 Estados de erro e recuperação](#95-estados-de-erro-e-recuperacao)
  - [9.6 Integração com o ambiente GNOME](#96-integracao-com-o-ambiente-gnome)
- [10. Verificação e validação](#10-verificacao-e-validacao)
  - [10.1 Estratégia de testes](#101-estrategia-de-testes)
  - [10.2 Critérios de aceite dos requisitos](#102-criterios-de-aceite-dos-requisitos)
  - [10.3 Testes unitários e de integração](#103-testes-unitarios-e-de-integracao)
  - [10.4 Testes com a API Google](#104-testes-com-a-api-google)
  - [10.5 Testes de sincronização, conflitos e falhas](#105-testes-de-sincronizacao-conflitos-e-falhas)
  - [10.6 Testes de segurança e privacidade](#106-testes-de-seguranca-e-privacidade)
  - [10.7 Testes GNOME e Flatpak](#107-testes-gnome-e-flatpak)
  - [10.8 Matriz de rastreabilidade](#108-matriz-de-rastreabilidade)
- [11. Distribuição e operação](#11-distribuicao-e-operacao)
  - [11.1 Ambientes Development e Production](#111-ambientes-development-e-production)
  - [11.2 Compilação, empacotamento e versionamento](#112-compilacao-empacotamento-e-versionamento)
  - [11.3 Automação e entrega via GitHub Actions](#113-automacao-e-entrega-via-github-actions)
  - [11.4 Instalação, atualização e remoção](#114-instalacao-atualizacao-e-remocao)
  - [11.5 Configuração e verificação OAuth para distribuição](#115-configuracao-e-verificacao-oauth-para-distribuicao)
  - [11.6 Canais de distribuição e suporte](#116-canais-de-distribuicao-e-suporte)
- [12. Evolução e governança](#12-evolucao-e-governanca)
  - [12.1 Situação atual dos requisitos](#121-situacao-atual-dos-requisitos)
  - [12.2 Propostas de funcionalidades futuras](#122-propostas-de-funcionalidades-futuras)
  - [12.3 Riscos, pendências e dívida técnica](#123-riscos-pendencias-e-divida-tecnica)
  - [12.4 Política de mudanças da especificação](#124-politica-de-mudancas-da-especificacao)
  - [12.5 Rastreabilidade com código, testes e decisões](#125-rastreabilidade-com-codigo-testes-e-decisoes)
  - [12.6 Referências documentais e acervo histórico](#126-referencias-documentais-e-acervo-historico)

## 1. Identificação e controle do documento

### 1.1 Identificação do produto

O DriveSync é um cliente independente do Google Drive para Linux, destinado à sincronização de conteúdo entre contas Google e pastas locais. O projeto não é um produto oficial do Google nem implica vínculo ou endosso dessa empresa.

Esta especificação reúne a definição do produto, seus requisitos, políticas e critérios de qualidade, servindo de referência para desenvolvimento, revisão e validação.

### 1.2 Versão e estado da especificação

Documento em elaboração. As seções conceituais definem a intenção do produto e as regras que devem orientar sua evolução; não constituem declaração de funcionalidades entregues ou de requisitos já validados.

Seções sem conteúdo permanecem pendentes de especificação. A revisão do documento é identificada pelo controle de versão do repositório, independentemente da versão do aplicativo.

### 1.3 Histórico de revisões

O histórico de revisões é mantido no Git. Commits e revisões de código registram a motivação e o alcance das alterações, evitando duplicar neste documento um diário de desenvolvimento.

### 1.4 Fontes e referências

A especificação deve distinguir decisões próprias do produto, restrições impostas por serviços externos e hipóteses ainda em estudo. Afirmações sobre interfaces externas devem ser fundamentadas em documentação oficial aplicável e revisadas quando essas interfaces mudarem.

O acervo em [`old/`](old/README.md) preserva requisitos, princípios e estudos anteriores. Seu conteúdo é fonte de consulta, não confirmação automática de vigência. As referências específicas devem acompanhar a seção a que dão suporte.

## 2. Visão geral do produto

### 2.1 Problema e motivação

O DriveSync é um cliente independente do Google Drive para Linux. A interface usa GTK4 e Libadwaita e segue as diretrizes de design do GNOME, sem restringir o aplicativo a esse ambiente de desktop. Flatpak é uma tecnologia de distribuição, não parte da definição do produto. A compatibilidade das integrações com o desktop deve ser validada nos ambientes de destino.

### 2.2 Objetivos do produto

- Facilitar o acesso local ao conteúdo escolhido pelo usuário no Google Drive.
- Reduzir o trabalho manual de manter conteúdo local e remoto coerente, dentro das políticas de sincronização autorizadas.
- Preservar dados e tornar alterações, limitações e situações de conflito compreensíveis.
- Permitir o uso de contas distintas com separação de identidade, configuração e dados.
- Oferecer uma experiência integrada ao desktop Linux, com controle claro sobre a execução do aplicativo.

### 2.3 Público-alvo e partes interessadas

O público-alvo é formado por usuários de Linux que utilizam o Google Drive e precisam trabalhar com conteúdo em pastas locais, incluindo pessoas que mantêm contas pessoais e institucionais distintas.

As partes interessadas incluem os usuários, os mantenedores e colaboradores do projeto e os responsáveis por ambientes institucionais cujas políticas condicionem o acesso às contas. A experiência deve atender ao uso cotidiano sem exigir conhecimento dos mecanismos internos de sincronização.

### 2.4 Escopo do produto

O domínio do DriveSync compreende a associação entre contas Google, conteúdo remoto e pastas locais, a identificação de mudanças e a execução de operações autorizadas para manter esse conteúdo coerente. Inclui a comunicação de progresso, impedimentos e decisões que exigem participação do usuário.

O escopo de cada entrega deve ser declarado separadamente. A finalidade geral do produto não implica que todas as direções de transferência, tipos de arquivo ou políticas de sincronização estejam disponíveis em uma determinada versão.

### 2.5 Funcionalidades incluídas

### 2.6 Funcionalidades excluídas e limitações

O DriveSync não substitui uma solução de backup independente. Sincronização e preservação de versões durante operações não equivalem a uma política de retenção, restauração histórica ou recuperação de desastre.

A integração com outros provedores de armazenamento e a edição de documentos são externas ao escopo definido para o produto. O acesso ao conteúdo remoto depende das permissões concedidas e das condições do serviço. Limitações específicas de uma entrega devem ser documentadas sem transformá-las automaticamente em restrições permanentes do produto.

### 2.7 Glossário e terminologia

| Termo | Significado |
| --- | --- |
| Conta | Identidade do usuário no provedor, à qual pertencem permissões e configurações. |
| Pasta local | Diretório do computador escolhido para receber ou fornecer conteúdo no escopo autorizado. |
| Pasta remota | Pasta do Google Drive associada ao conteúdo acompanhado pelo aplicativo. |
| Raiz de sincronização | Associação que delimita a conta e o conteúdo local e remoto abrangidos pela sincronização. |
| Sincronização | Processo de observar, comparar e aplicar mudanças segundo uma política definida. |
| Download / upload | Transferência de conteúdo do serviço para o computador / do computador para o serviço. |
| Estado-base | Evidência de uma condição previamente confirmada, usada para comparar alterações posteriores. |
| Conflito | Situação em que alterações ou identidades concorrentes impedem uma decisão automática segura. |
| Reconciliação | Reavaliação das evidências disponíveis para esclarecer divergências ou efeitos de operações interrompidas. |
| Estado desconhecido | Ausência de evidência suficiente para concluir a condição de um item; não equivale à sua ausência. |
| Operação destrutiva | Operação capaz de remover conteúdo ou substituir uma versão que precisa ser preservada. |

## 3. Contexto e restrições

### 3.1 Ambiente Linux e diretrizes de design GNOME

Linux é a plataforma de destino do produto. As diretrizes de design do GNOME orientam a experiência visual e de interação, sem tornar esse ambiente de desktop um requisito de uso.

Integrações dependentes do ambiente devem ter suas condições de disponibilidade explicitadas. A indisponibilidade de uma integração deve ser comunicada de forma compreensível, sem sugerir que uma operação foi realizada quando não foi.

### 3.2 Sistemas e serviços externos

### 3.3 Google Drive API e OAuth 2.0

### 3.4 Sistema de arquivos e armazenamento local

### 3.5 Premissas e dependências

O uso do serviço remoto pressupõe uma conta autorizada e conectividade nas operações que dependem dele. O armazenamento local deve estar acessível e dispor das permissões e dos recursos necessários à operação solicitada.

Permissões, conectividade e disponibilidade de armazenamento podem mudar durante o uso. O aplicativo deve tratar essas condições como verificações necessárias, e não como garantias permanentes. Políticas de organizações e decisões do usuário sobre permissões devem ser respeitadas.

### 3.6 Restrições tecnológicas e operacionais

## 4. Requisitos funcionais

### 4.1 Identificação e autenticação

### 4.2 Gestão de contas Google

### 4.3 Seleção e configuração de pastas

### 4.4 Descoberta e inventário de arquivos

### 4.5 Download e atualização local

### 4.6 Upload e criação remota

### 4.7 Detecção de mudanças e sincronização

O eixo 1 da [SYNC-001](https://github.com/rafatosta/drivesync/issues/1) define a
base confirmada B e sua comparação com L/R no fluxo de download de binários.
Os requisitos abaixo não autorizam upload de atualizações, exclusões ou resolução
assistida. Complementam RF-CON e RF-REC; os detalhes de decisão, persistência e
recuperação ficam nos [algoritmos do planner](../engine/src/planner/algorithms.md),
[state](../engine/src/state/algorithms.md) e [download](../engine/src/download/execution.md).

| ID | Requisito e critério de aceite |
| --- | --- |
| RF-SYNC-01 | Persistir B por identidade do item, cliente, conta e raízes. Recuperar revisão remota e fingerprint local da instalação confirmada após reabertura. Mudança de inode, ID ou escopo não herda autorização por nome ou igualdade de bytes (A01/A12). |
| RF-SYNC-02 | Distinguir inventário/plano de instalação confirmada. Ausência de B, inclusive banco perdido ou evidência incompleta, não autoriza adoção, sobrescrita ou exclusão de arquivo preexistente, seja seu conteúdo igual ou diferente do remoto (A02/A07/A08/A09; N01/N02). Estado inválido deve gerar erro seguro sem reset silencioso. |
| RF-SYNC-03 | Comparar L/R com B confiável sem decidir por timestamps. Classificar mudança apenas local e mudanças concorrentes, preservando conteúdo e B ao repetir após reabertura (A04/A05/A06; C01/C06). Classificação não tem efeitos de escrita. |
| RF-SYNC-04 | Permitir receber uma revisão remota mais nova com local intacto somente após revalidações; nova B deve refletir a instalação confirmada. Após V1 → V2, edição somente local deve ser classificada em relação a V2, sem falso trabalho pendente ou download redundante (A03/A11). |
| RF-SYNC-05 | Intenção, transferência parcial e efeito sem confirmação não fornecem B. Operação posterior incompleta deve invalidar a candidata anterior para o planejamento até reconciliação; replay exige evidências verificadas e preserva destinos concorrentes. Repetição após confirmação deve ser idempotente (A10). |

**Implementação:** parcial no fluxo download-only; não cobre a matriz geral de
sincronização bidirecional. **Verificação:** testes puros e de integração local
com provedor falso exercitam as regras; a
[matriz de rastreabilidade](../engine/src/state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
identifica arquivos, testes e limites por cenário. **Aceite real:** a bateria
[A01–A12](https://github.com/rafatosta/drivesync/issues/1#issuecomment-6480152816)
com Google Drive real permanece pendente; nenhum requisito recebe validação
completa apenas por essa cobertura local.

#### Contrato de identidade — SYNC-002

**Estado: contrato acordado; implementação parcial no fluxo de download.**
I01–I28 possuem verificadores locais para o recorte acordado. A variante de I20
com proveniência explícita não se aplica à interface atual. A auditoria de aceite
é separada da existência dos testes; execuções e resultados pertencem aos
commits/PR, não a esta seção. Origem: [issue #2 e proposta I01–I28](https://github.com/rafatosta/drivesync/issues/2#issuecomment-6483197201).
O recorte é download de binários com remoto falso, filesystem/SQLite reais e
reabertura entre ciclos. Não exige upload, propagação de exclusões, resolução
assistida ou execução geral de movimentos. Complementa RF-SYNC-01–05,
RF-CON-02 e RF-REC; os oráculos e vínculos com testes estão no
[catálogo I01–I28](../engine/src/download/tests/identity-test-cases.md).

| ID | Requisito e critério observável |
| --- | --- |
| RF-ID-01 | Comparar separadamente identidade local (device/inode), conteúdo (SHA-256), localização (caminho e ancestralidade) e revisão remota. Mesmo objeto local com bytes diferentes é alteração de conteúdo; device/inode diferente é divergência de identidade, inclusive com bytes iguais. Expor motivo verificável, isolado ou combinado com mudança remota, sem exigir nomes específicos de enums (I02–I04/I17/I22). |
| RF-ID-02 | B pertence a cliente OAuth, conta, raízes e ID remoto. Outro ID não herda B nem autorização, mesmo com nome/pai/hash iguais. Evidência ausente ou inválida não é ausência de arquivo. Observação e transferência incompleta não confirmam vínculo (I01/I05/I08–I09/I25–I27). |
| RF-ID-03 | Preservar IDs homônimos e bloquear o caminho ambíguo e descendentes, sem mídia nem avanço de B nesses itens. Resultado independente de ordem/paginação; caminhos distintos não colidem apenas por nome. Reavaliar após desfazer colisão sem adotar o objeto restante pelo caminho (I10–I14). |
| RF-ID-04 | Mesmo ID com caminho novo conserva sua associação e produz diagnóstico de localização anterior/atual. Preservar origem e destino ocupado; não transferir ao destino uma autorização baseada apenas no caminho anterior. Recebimento no destino livre pode seguir o contrato de instalação inicial, com nova confirmação durável; não implica mover/apagar a origem (I07). |
| RF-ID-05 | Bytes históricos no mesmo ID e revisão atual maior representam nova alteração remota, elegível à atualização segura se L permanece íntegro. Novo ID representa outro objeto. Restauração local deve distinguir conteúdo alterado e identidade substituída; revisão regressiva/zero/ausente suspende ação como desconhecida/inconsistente (I15–I19). |
| RF-ID-06 | Não afirmar intenção de restauração por hash, timestamp ou regressão de revisão. Sem proveniência explícita, registrar alteração observável e sua causa ou pendência por evidência insuficiente. Essa incerteza sobre intenção não bloqueia, por si só, uma atualização segura do mesmo objeto com revisão maior. A variante de restauração comprovada depende de suporte explícito a essa evidência; não exige construir serviço de histórico nesta entrega (I20). |
| RF-ID-07 | Revalidar escopo, identidade, conteúdo e revisão antes dos efeitos e no replay. Preservar substitutos concorrentes, versões anteriores e B confirmada; confirmar somente efeito demonstrado. Indisponibilidade não significa exclusão; repetição/eventos antigos não ressuscitam vínculo nem duplicam efeitos. Estado restaurado exige confronto com evidências atuais (I21–I28). |

**Limites da evidência:** device/inode não é identidade eterna. Reutilização pode
ocorrer; igualdade desse par e dos bytes não prova continuidade universal.
Os testes de substituição devem manter o original vivo em outro caminho e
comprovar a diferença de identidade. Se banco, arquivos e demais vestígios
observáveis forem revertidos juntos a um estado consistente, este recorte não
promete detectar o rollback sem âncora independente.

**Restauração comprovável (I20):** requer evidência de origem explícita, vinculada
ao objeto/escopo, à versão histórica de origem e à versão/efeito atual, com
proveniência verificável. O journal de downloads comprova instalações passadas,
mas a repetição de um hash não comprova a operação de restauração. A interface
atual de inventário não fornece essa proveniência. Se tal suporte for introduzido,
seu contrato de aquisição e validação precisa ser especificado antes de alegar
restauração comprovada. Neste recorte, testar obrigatoriamente a ausência de
prova, sem fabricar uma indicação de restauração no falso provedor sem consumidor
real correspondente.

**Implementação e limites:** o planner separa divergência de device/inode
(`LocalIdentityChanged`/`ConcurrentIdentityChanges`) de alteração de bytes no
mesmo objeto (`LocalChanged`/`ConcurrentChanges`). Fingerprint incompleta é
`DeferredUnknown`; tipo incompatível é `LocalTypeMismatch`. Esses motivos são
verificáveis no plano persistido, sem prometer sua apresentação na GUI. O fluxo
incremental expõe localização anterior/atual e permite receber em destino livre
preservando a origem. O relatório de execução pode manter `DownloadConflict`.
O contrato não autoriza novas operações destrutivas nem comprova integração real.

#### SYNC-003 — inventário e reconciliação

Contrato de implementação autorizado pela demanda SYNC-003; o aceite depende
dos testes e níveis de evidência abaixo, não da existência desta especificação.

- Inventariar a árvore local inteira em cada ciclo, inclusive depois de offline
  ou aplicativo fechado. Não depender de notificações para descobrir mudanças.
  Falha de varredura não produz um inventário completo nem autoriza inferir ausência.
- Separar arquivos estáveis, temporários reconhecidos, em gravação, tipos
  incompatíveis e evidência desconhecida. Observar identidade, tamanho, mtime e
  ctime antes/depois de uma janela de quiescência de 500 ms compartilhada pela
  varredura; revalidar leitura e caminho. Datas servem para detectar instabilidade,
  nunca para escolher uma versão vencedora. Uma pausa maior que a janela não
  comprova encerramento do escritor; esta limitação também vale para snapshots.
- Reservar arquivos internos do DriveSync. Adiar padrões explícitos de temporários
  de editores e downloads (`.swp`, `.swo`, `~`, `.part`, `.crdownload`, `.goutputstream-*`,
  `.~lock.*#` e `~$*`). Não excluir universalmente arquivos ocultos nem `.tmp`.
- Conciliar B, inventário local e feed remoto por identidade/escopo. Hash igual
  não elimina substituição de inode ou ID; sem B, não adotar preexistentes.
  Item remoto indisponível, inventário incompleto e erro não equivalem a exclusão.
- Para versões concorrentes do mesmo objeto com B, comprovar equivalência por
  SHA-256 local e dos bytes remotos com tamanho/checksum e metadados revalidados.
  Falha/alteração durante a verificação adia o item. Equivalência não instala,
  adota ou avança B automaticamente; o executor continua revalidando o destino.
- Persistir observações/classificações por cliente, conta e raízes em transação
  independente do checkpoint remoto e do journal de instalação. Após reinício,
  observações antigas não autorizam escrita e devem ser substituídas por nova
  varredura. Falhas não apagam a geração anterior; formatos incompatíveis são
  rejeitados sem migração ou reset automático.
- Antes de criar upload, revalidar o candidato estável e o snapshot. Manter
  opt-in create-only, intents duráveis e reconciliação de efeitos ambíguos.
- Expor pendências e suas causas no relatório/interface existente. Alteração
  local ou concorrente não habilita atualização remota, exclusão ou resolução
  assistida completa nesta entrega.

Aceite: C06 (mudanças offline locais/remotas/concorrentes com B reaberta), C21
(timestamps diferentes, bytes equivalentes, revisão maior e identidade substituída)
e N10 (temporários, gravação durante quiescência/leitura/snapshot e retomada).
Repetição, cancelamento, falhas de persistência e isolamento devem preservar bytes
e B. Filesystem/SQLite reais com remoto simulado são evidência local; Google
Drive real, reinício de processo e aplicativo empacotado exigem validação própria.

A implementação está em `local`, `changes`, `state/reconciliation` e nos
executores existentes. A [matriz local de aceite](../engine/src/changes/reconciliation.md)
liga C06/C21/N10 aos testes determinísticos. O aceite com Drive real e o aplicativo
empacotado permanecem pendentes; executar ferramentas no SDK não os substitui.

### 4.8 Pausa, retomada e execução em segundo plano

### 4.9 Tratamento de conflitos e exclusões

Os requisitos RF-CON abaixo definem o comportamento pretendido. Não habilitam atualização remota de arquivos existentes, propagação de exclusões ou resolução automática. A situação da implementação está em 12.1; os critérios de aceite e vínculos com o catálogo estão em 10.5 e 10.8.

**Entradas comuns:** conta e raiz, identidade do item, estado-base confirmado B (ou ausência explícita), observações local L e remota R, revisões/fingerprints disponíveis, tipo, caminho, disponibilidade e política autorizada. Ausente, inacessível e desconhecido são estados distintos. Nome, caminho e timestamp isolados não comprovam identidade, equivalência ou precedência.

**Saída comum:** classificação, motivo, evidências usadas, escopo bloqueado e ações candidatas sujeitas à política. Classificar não transfere, renomeia, exclui nem concede autorização para executar. Dados insuficientes produzem resultado inconclusivo e bloqueio das alterações dependentes dessa evidência.

| ID | Requisito |
| --- | --- |
| RF-CON-01 | Comparar B/L/R e distinguir ausência de mudança, mudança unilateral, divergência concorrente, equivalência, incompatibilidade e indisponibilidade/erro. Sem B confiável, não inventar uma sincronização anterior nem deduzir exclusão. |
| RF-CON-02 | Reconhecer identidade separadamente de localização e conteúdo. Colisões de nomes, substituições, renomeações e movimentos devem considerar ancestralidade e dependências; não escolher arbitrariamente um item remoto homônimo. |
| RF-CON-03 | Bloquear substituição de conteúdo divergente até preservar as versões disponíveis e registrar sua origem e identidade. Distinguir preservação concluída, pendente e versão inacessível; falta de espaço ou colisão não autoriza descarte. |
| RF-CON-04 | Manter pendência durável por conta, raiz, item e revisões observadas. Adiar ou fechar a interface não resolve o conflito; itens comprovadamente independentes podem continuar. Descendentes de diretório ambíguo não são independentes. |
| RF-CON-05 | Oferecer resolução assistida somente para ações suportadas: manter local, manter remoto, manter ambas ou adiar. Explicar destino e preservação de cada versão; revalidar identidade, conteúdo, revisão, escopo e autorização imediatamente antes dos efeitos. Uma mudança invalida a decisão anterior. |
| RF-CON-06 | Tratar exclusão confirmada separadamente de perda de acesso e ausência de observação. A primeira entrega deste tratamento não propagará exclusões nem escolherá automaticamente a versão mais recente; pedidos destrutivos permanecem bloqueados. |

O [catálogo de 56 cenários e políticas](../engine/src/conflicts/validation-checklist.md) fornece casos de cobertura, não 56 funcionalidades implementadas. A matriz B/L/R desse catálogo orienta a classificação; uma ação candidata, como enviar mudança local, continua bloqueada se o executor ou a política não a suportar.

#### SYNC-008 — edição versus exclusão ou acesso perdido

Usar B confirmada e os caminhos históricos da árvore para proteger arquivos e
diretórios indisponíveis. Comparar o conteúdo e a identidade local com B;
relocação requer caminho antigo ausente e candidato único de fingerprint completo.
Alterações locais com remoto indisponível ficam pendentes de revisão, sem
afirmar exclusão. Substituição de identidade continua incompatível.

Somente metadados explícitos de lixeira associados ao mesmo ID estabelecem
observação de exclusão remota. `removed`, scan incompleto, HTTP 404, timeout e
perda de acesso permanecem ambíguos. Descendentes de uma pasta removida preservam
suas próprias evidências; a prova de lixeira da pasta não prova exclusão individual
de cada filho. Edição, relocação ou novo descendente eleva a atenção dos diretórios
históricos afetados. Incerteza sem alteração comprovada permanece desconhecida.

Bloquear recriação quando o caminho de B estiver ausente e R tiver sido editado,
renomeado ou movido. Nunca propagar exclusão, escolher por timestamp, recriar um
remoto indisponível ou avançar B como consequência desse diagnóstico. Preservação
significa manter as versões disponíveis intactas; não promete cópia de conflito
nem recuperação de conteúdo remoto inacessível. Resolução assistida permanece
fora da primeira entrega.

Aceite C03/C04/C08/C11/C16: prova explícita versus ambígua, arquivos/diretórios,
edição/relocação e substituição, offline/retomada, concorrência entre preparação
e execução, falha transacional, reabertura e repetição sem perda ou duplicação.
Oráculos em [conflicts/contract.md](../engine/src/conflicts/contract.md#sync-008--edição-e-indisponibilidade).
A validação usa provedor simulado e filesystem/SQLite reais isolados.
Os formatos internos de checkpoint v2 e reconciliação v3 rejeitam formatos antigos
sem migração nem reset automático; recuperação exige ação explícita.

#### SYNC-007 — edições simultâneas divergentes

A detecção usa a classificação centralizada e a prova de conteúdo existentes.
Com B confirmada, alterações de bytes em L e R exigem comparação L/R: igualdade
comprovada resulta em equivalência; diferença comprovada registra conflito e
bloqueia substituições. Revisão remota maior com bytes iguais a B continua sendo
mudança somente local. Datas nunca escolhem uma versão.

Depois da prova remota, revalidar assinatura, SHA-256 e identidade local pelo
descritor da raiz bloqueada antes de publicar equivalência, divergência ou decisão
de localização. Edição, substituição, remoção ou falha de leitura invalida a prova
local e adia o item; cancelamento ou suspensão da raiz aborta a preparação.
Essa revalidação é uma observação, não um bloqueio contra editores externos:
execução e futura resolução continuam exigindo nova verificação.

A pendência sinaliza as duas origens observadas (arquivo local e arquivo no Drive).
Preservar aqui significa deixá-las intactas, sem selecionar um vencedor, instalar
conteúdo, criar cópia de conflito ou avançar B. Offline/falha na prova significa
informação desconhecida; a retomada refaz a análise. Observações persistidas em
SQLite sobrevivem à reabertura, mas não autorizam execução sem nova observação.
A resolução assistida e a cópia durável de ambas as versões são entregas distintas,
ainda não habilitadas. A disponibilidade observada não garante retenção histórica
no Drive nem protege contra alterações externas posteriores.

Aceite C01/C06: comparação igual/diferente, offline/retomada, edição e substituição
durante a prova, persistência/reabertura e repetição sem cópias ou avanço de B.
Ver [contrato e oráculos](../engine/src/conflicts/contract.md#sync-007--detecção-de-edições-divergentes).
A validação determinística usa remoto simulado, arquivos e SQLite reais isolados;
não equivale a homologação com Drive real, UI ou queda real de processo.

#### SYNC-006 — classificação centralizada

O classificador puro de `conflicts` deve distinguir diagnóstico, encaminhamento
candidato e motivo tipados. A entrada normaliza diferenças de L/R em relação a B,
com identidade contínua, conteúdo, nome, pai e disponibilidade independentes.
B inválida, observação desconhecida e indisponibilidade não autorizam efeitos.
Não se deve apresentar uma revisão remota apenas de metadados como edição
concorrente de conteúdo quando os bytes de R continuam iguais a B.

C01/C03/C04 representam conflitos quando a divergência/exclusão for comprovada;
sem prova de exclusão, ausência e indisponibilidade permanecem distintas. Ausência
local com revisão remota nova no mesmo caminho instalado exige revisão antes de
recriar L. Mudança remota para caminho novo preserva o comportamento download-only
existente e os dados do caminho anterior.
C09/C12 devem receber diagnóstico conciliável quando identidade e atributos
independentes forem verificáveis; C21 não pode gerar conflito por timestamp ou
conteúdo comprovadamente igual. P01 mantém operação automática apenas quando
suportada e autorizada. Hash igual não adota identidade substituída nem cria B.

Implementação: decisão central e adaptadores em `conflicts`, coleta/prova em
`changes`, observações SQLite com B separada da instalação. O plano pode guardar
uma classificação de localização não executável e o caminho local associado.
A combinação é uma pendência, sem cópia falsa, aplicação automática ou confirmação
de sincronização. A atualização de localização remota, exclusões, cópias gerais
de conflito e resolução assistida permanecem fora desta entrega.

O reconhecimento de relocação local exige caminho anterior comprovadamente
ausente e candidato único com o hash/dispositivo/inode de B; não oferece identidade
permanente contra reutilização de inode. Relocação e edição local simultâneas ou
pais sem mapeamento verificável não recebem inferência otimista. Upload create-only
não deve criar outro objeto para identidade gerenciada observada em caminho novo.

Oráculos: [classificação SYNC-006](../engine/src/conflicts/algorithms.md#classificação-sync-006).
O aceite local usa provedor remoto simulado, filesystem e SQLite temporários reais;
resultados efetivos pertencem ao commit/PR. Não implica homologação com Drive real
ou aplicação completa dos cenários de preservação/resolução.

### SYNC-009 — nomes, caminhos e tipos (C02/C13/C14/C15/C18/C20)

Requisito: IDs distintos não podem compartilhar destino ambíguo. O planner deve
considerar caixa, normalização Unicode e sufixos gerados de Workspace; bloquear
nomes não representáveis, tipos incompatíveis e descendentes de destinos
ambíguos. Não escolher vencedor, sobrescrever dados nem renomear automaticamente.
A proteção conservadora também se aplica em filesystem sensível a caixa.

Implementação: chaves canônicas com full Unicode case folding para irmãos
remotos e caminhos do inventário local completo; validação portátil de componentes
antes/depois da conversão e limite do caminho incluindo a raiz. Bloqueios são
persistidos pelo plano existente, sem avanço da base. O contrato, limites e
oráculos estão em [PLAN/SYNC-009](../engine/src/planner/algorithms.md#sync-009--destinos-representáveis-e-sem-ambiguidades).

Aceite: duplicatas exatas e equivalentes, tipos em ambas as direções, caracteres,
nomes reservados, comprimentos em bytes e links gerados; itens independentes devem
continuar funcionando. Testes com provedor simulado, filesystem/SQLite temporários
reais verificam preservação, reabertura e idempotência. Concorrência permanece
sujeita às revalidações e instalação sem clobber do executor. Resultados de execução
pertencem ao PR; não equivalem a teste com Google Drive ou volume case-insensitive
real. Não se promete modelar todas as regras específicas de mounts.

### 4.10 Recuperação de falhas e reconciliação

**Entradas comuns:** identidade da conta/raiz, configuração autorizada, registros duráveis de operações e pendências, evidências atuais dos destinos, staging e cópias preservadas. **Saídas:** efeito confirmado, retomada segura, replanejamento, espera por condição externa ou bloqueio explícito. Recuperação não pode assumir que timeout significa ausência de efeito.

| ID | Requisito |
| --- | --- |
| RF-REC-01 | Antes de repetir operação interrompida, confrontar sua identidade e efeitos observáveis com o registro persistente. Confirmar o efeito existente quando comprovado; repetir somente quando comprovadamente seguro. Ambiguidade mantém a operação pendente, sem criar duplicatas por repetição cega. |
| RF-REC-02 | Em banco ausente para uma raiz anteriormente gerenciada, inválido, corrompido ou incompatível, preservar arquivos e impedir decisões que dependam do estado perdido. Não reinicializar silenciosamente nem converter inventário em baseline confirmado. |
| RF-REC-03 | Suspender alterações no escopo de raiz inacessível, identidade divergente ou autorização perdida. Mudança de conta, raiz ou configuração invalida trabalho cuja autorização deixou de valer; reconciliar efeitos já iniciados antes de replanejar. Atores concorrentes não podem receber autorização conflitante para a mesma raiz. |
| RF-REC-04 | Distinguir falha transitória, impedimento que exige usuário, conflito, incompatibilidade e resultado incerto. Tentativas devem ser limitadas por política explícita e canceláveis; reconexão não desfaz pausa solicitada pelo usuário. |
| RF-REC-05 | Confirmar conclusão apenas após verificar integridade, identidade e persistência do resultado. Recuperar ou abandonar staging sem tocar em conteúdo de origem desconhecida; preservar versões anteriores e trabalho confirmado após cancelamento ou reinício. |

Os tipos concretos de erro, limites de tentativas e mecanismos de persistência serão definidos nos contratos de implementação. Nenhum nome de estado aqui obriga um formato de banco ou uma API interna.

#### Recuperação explícita do estado — SYNC-005

A ação **Recover synchronization state…** exige confirmação e funciona sem rede.
Ela verifica a associação sob os bloqueios de raiz, conserva bancos válidos e
preserva bancos inválidos e sidecars nos caminhos originais. Substitutos vazios
são selecionados somente após validação e publicação durável de um seletor.
Formato incompatível não é migrado. O algoritmo e os limites estão em
[STATE-003](../engine/src/state/algorithms.md#state-003--reconstrução-explícita-sync-005).

Reconstrução não confirma instalações nem adota arquivos preexistentes. Sem B
confiável, esses arquivos continuam sujeitos à classificação conservadora. A
sincronização deve ser iniciada novamente para atualizar as observações remotas;
o término da reconstrução local não significa convergência. Estado inválido
suspende o escopo e registra o motivo fora da raiz quando esse registro está
íntegro e disponível.

Se evidências de upload forem perdidas, preservar e bloquear uploads, inclusive
nova habilitação, até reconciliação adicional; não repetir creates de resultado
incerto. Essa pendência aparece no relatório combinado. Seletor inválido ou
perdido bloqueia leituras normais; recuperação explícita preserva suas evidências
e trata todas as gerações como desconhecidas, sem retornar a bases antigas.
Objetos inseguros ou acesso insuficiente exigem restauração de acesso/metadados;
não há fallback para outro banco nem limpeza automática.

Critérios específicos: preservar observações de bancos íntegros; não substituir
originais; reabrir estado selecionado sem criar B artificial; repetir recuperação
sem nova seleção quando nada mudou; recusar worker concorrente; retomar trabalho
observado sem exigir novo evento; confirmar B somente após efeito íntegro durável.
Testes de SQLite/filesystem e provedor simulado não comprovam queda física de
energia, integração Google nem funcionamento da interface instalada.

### 4.11 Documentos nativos do Google Workspace

### 4.12 Estado, histórico e notificações

### 4.13 Preferências e gerenciamento da aplicação

## 5. Requisitos não funcionais

### 5.1 Segurança e controle de acesso

O aplicativo deve operar somente no escopo autorizado pelo usuário e com as permissões necessárias a cada operação. Credenciais devem ser protegidas contra exposição em interfaces, diagnósticos e registros.

A identidade da conta e a vinculação do conteúdo devem ser verificadas antes de operações que possam alterar dados. A perda de autorização deve impedir novas operações dependentes dessa autorização, sem ser interpretada como exclusão de conteúdo.

### 5.2 Privacidade e tratamento de dados

O tratamento de conteúdo e metadados deve limitar-se às finalidades necessárias ao funcionamento do produto. Informações de diagnóstico devem evitar exposição desnecessária de nomes, caminhos, identificadores pessoais e conteúdo de arquivos.

Qualquer coleta ou compartilhamento adicional de dados deve ter finalidade e condições claramente informadas. Solicitações de suporte não devem exigir o envio de credenciais ou de arquivos privados.

### 5.3 Integridade e prevenção de perda de dados

A preservação dos dados deve prevalecer sobre a conclusão automática de uma sincronização. Arquivos modificados ou de origem desconhecida não devem ser substituídos ou excluídos silenciosamente.

Uma operação somente deve ser considerada concluída após a confirmação de seus efeitos e do estado necessário para reconhecê-los posteriormente. Diante de evidência insuficiente, o aplicativo deve preservar o conteúdo e indicar o impedimento.

### 5.4 Confiabilidade, resiliência e disponibilidade

Falhas de rede, interrupções do processo e indisponibilidade de recursos devem ser tratadas como condições esperadas de operação. O produto deve permitir identificar trabalho concluído, pendente e de resultado incerto, evitando repetição que produza perda ou duplicação indevida.

A disponibilidade do aplicativo não implica disponibilidade do serviço remoto. Falhas restritas a uma conta devem permanecer isoladas sempre que isso não comprometer a integridade de recursos compartilhados.

### 5.5 Desempenho e consumo de recursos

O aplicativo deve manter a interface responsiva durante operações de sincronização e evitar trabalho repetido quando houver evidência suficiente de que o conteúdo permanece inalterado. O uso de rede, armazenamento, memória e processamento deve ser compatível com a execução em um computador de uso cotidiano.

Metas quantitativas e limites de capacidade devem ser definidos com cargas de referência e condições de medição explícitas. Esta diretriz não estabelece valores nem afirma resultados de desempenho.

### 5.6 Usabilidade e acessibilidade

A interface deve apresentar ações, estados e consequências em linguagem compreensível, distinguindo progresso, conclusão, pausa e necessidade de intervenção. Decisões com impacto sobre dados devem informar o conteúdo afetado e o efeito esperado.

As interações devem considerar navegação por teclado, tecnologias assistivas, legibilidade e diferentes dimensões de janela. Informações essenciais não devem depender exclusivamente de cor ou de elementos visuais sem descrição.

### 5.7 Compatibilidade e portabilidade

Os ambientes suportados devem ser declarados com base em validação. O uso das diretrizes GNOME não deve ser confundido com garantia de funcionamento de todas as integrações em qualquer desktop Linux.

Diferenças do sistema de arquivos, das permissões e dos serviços de desktop devem ser consideradas na definição de suporte. Limitações conhecidas devem ser informadas ao usuário.

### 5.8 Observabilidade e diagnósticos

O produto deve fornecer informações suficientes para compreender o estado das operações e investigar falhas, preservando a privacidade. Uma mensagem de erro deve distinguir, quando houver evidência, impedimentos de autorização, comunicação, armazenamento, integridade e conflito.

Indicadores de conclusão devem refletir o escopo efetivamente verificado. Itens ignorados, bloqueados ou pendentes não devem desaparecer em um estado agregado de sucesso.

### 5.9 Manutenibilidade e testabilidade

As responsabilidades e os contratos devem ser claros, permitindo revisar decisões e verificar comportamentos de forma independente. Regras de negócio devem ser distinguíveis de detalhes de apresentação e de integração externa.

Mudanças devem ser pequenas, justificadas e verificáveis. A documentação deve explicar requisitos, invariantes e decisões duradouras, enquanto evidências pontuais de execução permanecem associadas às alterações correspondentes.

## 6. Regras de negócio e políticas operacionais

### 6.1 Identidade e isolamento de contas

A identidade de uma conta deve permanecer independente de seu nome de exibição. Credenciais, configurações, observações e operações devem conservar a associação com a conta de origem.

Uma operação não deve utilizar autorização ou estado pertencente a outra conta. Falhas de autenticação de uma conta não devem autorizar mudanças nos dados de outra.

### 6.2 Vinculação de pastas e escopo de sincronização

A sincronização deve atuar em um escopo explicitamente escolhido pelo usuário. Alterar esse escopo exige reavaliar a associação entre o conteúdo local e remoto; dados existentes não devem ser assumidos como pertencentes à nova associação.

Raízes locais ativas não devem coincidir nem se sobrepor. A verificação deve considerar caminhos que representem o mesmo diretório, evitando que associações distintas disputem os mesmos dados.

### 6.3 Política de upload e download

### 6.4 Política de conflitos e alterações concorrentes

RF-CON-01–05 orientam a classificação e a resolução. A política inicial é solicitar decisão em divergência real, preservando conteúdo; equivalência comprovada não deve gerar uma falsa escolha entre versões. Datas podem informar a comparação, mas não decidem qual conteúdo é correto.

| Escolha | Efeito pretendido e restrição |
| --- | --- |
| Manter local | Tornar a versão local a escolhida para convergência, preservando a divergente. Se exigir atualizar arquivo remoto existente, permanecer indisponível até haver executor e salvaguardas aprovados. Não equivale a autorizar upload irrestrito. |
| Manter remoto | Receber a revisão remota revalidada e preservar a versão local divergente antes de substituí-la. Se preservação ou acesso falharem, bloquear. |
| Manter ambas | Preservar conteúdos distintos sob identidades e nomes sem colisão, com destinos explicados. A convergência de ambas só termina quando os efeitos autorizados forem confirmados; não basta gerar uma cópia temporária. |
| Adiar | Não aplicar a resolução e manter a pendência. Continuar somente trabalho independente. |

Não executar mesclagem automática nem preferências por timestamp na primeira entrega. Preferências automáticas, comparação visual por formato e operações em lote permanecem propostas. Cópia de conflito, histórico de versões e registro de execução têm finalidades distintas; o registro não substitui os bytes preservados. Nenhuma limpeza automática de cópias de conflitos pendentes será habilitada sem política explícita de retenção e recuperação.

Os estudos de [viabilidade](../engine/src/conflicts/technical-feasibility.md) e [UX](../engine/src/conflicts/ux-research.md) são referências de investigação. Sua existência não comprova garantias de concorrência nem aprova as opções futuras.

### 6.5 Política de exclusões e operações destrutivas

Falhas de acesso, comunicação ou autenticação não constituem evidência suficiente de exclusão. Uma operação destrutiva exige identidade, estado e autorização compatíveis com a política definida para o recurso.

Diante de inconsistência ou resultado incerto de uma operação anterior, novas operações destrutivas devem permanecer bloqueadas até reconciliação. A conveniência de automatizar uma ação não justifica ignorar a preservação de versões ou o controle do usuário.

Na primeira entrega de conflitos, RF-CON-06 bloqueia toda propagação de exclusão, inclusive em lote. O tamanho do lote nunca concede autorização. Para habilitar essa capacidade no futuro, especificar limiares de impacto, quantidade e proporção afetadas, confirmação de escopo, retenção, recuperação e testes de concorrência; limiares indefinidos mantêm a funcionalidade bloqueada.

### 6.6 Política de falhas, retries e retomadas

Falhas transitórias podem permitir novas tentativas, desde que sua repetição seja segura e limitada por uma política explícita. Falhas permanentes ou que dependam de decisão do usuário devem ser apresentadas como impedimentos.

Quando houver dúvida sobre o efeito de uma tentativa anterior, o estado deve ser reconciliado antes de repeti-la. Uma pausa solicitada pelo usuário deve ser respeitada; recuperação de conectividade não deve anulá-la silenciosamente.

### 6.7 Política de consistência e persistência de estado

O estado persistente deve refletir evidências suficientes para reconhecer o que foi observado e executado. Seu avanço não deve ultrapassar alterações que possam ser recuperadas ou reconciliadas após interrupção.

Dados inconsistentes ou incompatíveis não devem ser descartados ou reinicializados silenciosamente. A ausência de estado confiável deve reduzir a autorização para alterar conteúdo, e não ampliá-la.

### 6.8 Limites e formatos de arquivos suportados

## 7. Arquitetura do software

### 7.1 Visão e princípios arquiteturais

### 7.2 Componentes e responsabilidades

### 7.3 Motor de sincronização

### 7.4 Interface GTK4 e Libadwaita

### 7.5 Comunicação interna e contratos

### 7.6 Integração com Google Drive

### 7.7 Persistência local e cofre de credenciais

### 7.8 Ciclo de vida e execução do aplicativo

### 7.9 Decisões arquiteturais e justificativas

## 8. Dados e interfaces

### 8.1 Entidades e identificadores

### 8.2 Configuração e preferências

### 8.3 Estado persistente da sincronização

### 8.4 Interfaces com APIs externas

### 8.5 Interfaces com o sistema operacional

### 8.6 Interfaces entre módulos

### 8.7 Formatos de dados e compatibilidade

## 9. Experiência e interface do usuário

### 9.1 Fluxo de primeiro acesso

### 9.2 Conexão e gestão de contas

### 9.3 Configuração de sincronização

### 9.4 Painel principal e indicadores

### 9.5 Estados de erro e recuperação

A interface deverá representar os resultados de RF-CON e RF-REC em linguagem comum, distinguindo conflito, incompatibilidade, problema operacional e verificação inconclusiva. Para cada pendência, mostrar conta, item, localização, motivo, ação possível e estado real da preservação.

Antes de uma resolução, apresentar as versões disponíveis, suas origens, consequências da escolha e destino da cópia preservada. Informação ausente deve aparecer como desconhecida; ações não suportadas ou sem autorização não podem ser executadas. Se o item mudar durante a comparação, solicitar nova análise, sem reaplicar a escolha anterior.

Adiar e reabrir devem recuperar a mesma pendência. Uma raiz suspensa deve aparecer como indisponível, sem sugerir que seus arquivos foram excluídos. O resumo não pode declarar sincronização completa enquanto houver itens bloqueados ou inconclusivos. O fluxo deve funcionar por teclado e leitor de tela, sem depender apenas de cor.

A disposição visual de uma central de atenção e comparadores por formato continua em estudo na [pesquisa de UX](../engine/src/conflicts/ux-research.md); esta seção define comportamento observável, não um desenho de tela aprovado.

### 9.6 Integração com o ambiente GNOME

## 10. Verificação e validação

### Catálogo de cenários e fluxogramas de sincronização

A [matriz numerada de 54 cenários](sync/scenarios.md) define pré-condições e ações esperadas para mudanças B/L/R, criações, exclusões, renomeações, conectividade, interrupções e falhas técnicas. Os [fluxogramas Mermaid individuais](sync/flowcharts.md) explicitam decisões e bloqueios de segurança; a [matriz de verificação](sync/verification.md) será preenchida após confronto com o código e resultados de testes. Estas são **especificações propostas**, não declarações de funcionalidades implementadas ou aprovadas. O [catálogo técnico anterior C/E/P/N](../engine/src/conflicts/validation-checklist.md) não é substituído por esta numeração.


### 10.1 Estratégia de testes

A verificação deve partir dos requisitos, das invariantes e dos efeitos observáveis esperados. Deve combinar avaliação de regras isoladas, integração entre responsabilidades e cenários de ponta a ponta nos ambientes declarados como suportados.

Cenários de falha, interrupção, concorrência e isolamento de contas devem acompanhar os fluxos de sucesso. Testes simulados e testes com serviços reais fornecem evidências diferentes e devem ser identificados separadamente.

### 10.2 Critérios de aceite dos requisitos

Cada requisito deve ter critérios que descrevam condições iniciais, ação ou evento, resultado esperado e efeitos proibidos. Critérios de preservação devem verificar também o conteúdo que não pode ser alterado.

A existência de código ou a compilação bem-sucedida não bastam para declarar aceite. A validação deve identificar cenário, ambiente, resultado observado e limitações da evidência.

### 10.3 Testes unitários e de integração

Para RF-SYNC-01–05, os testes puros do planner verificam decisões sem I/O;
[baseline_tests.rs](../engine/src/download/baseline_tests.rs) combina planner,
journal e executor com diretórios temporários e SQLite reais, reabrindo o estado
entre ciclos e reutilizando o provedor remoto falso. Os testes de download e
atualização complementam integridade, colisões e replay. A
[rastreabilidade por regra](../engine/src/state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
é o índice dessa evidência. Resultados/comandos pontuais ficam nos commits, não
constituem histórico cumulativo nesta especificação.

### 10.4 Testes com a API Google

A bateria A01–A12 da SYNC-001 permanece pendente com Google Drive real e está
registrada na [issue de acompanhamento #28](https://github.com/rafatosta/drivesync/issues/28). Exige
conta/perfil e raízes isolados, arquivos dispensáveis, atualização do mesmo ID
remoto e evidência de conteúdo, identidade, revisão, base/fase persistida e
repetição após reinício. Deve também demonstrar a classificação usada, pela
interface ou diagnóstico. Reabertura de conexões SQLite em testes locais não
substitui reinício do aplicativo, nem simula queda de energia. O smoke test
ignorado de navegação Google não satisfaz esses critérios.

O [roteiro de baterias reais](../engine/src/download/tests/test-cases.md#baterias-com-google-drive-real)
detalha preparação, procedimentos e oráculos de A01–A12, grupos complementares
de integração/empacotamento e limitações das ferramentas disponíveis. Todos
permanecem pendentes; a existência do roteiro não registra execução.

### 10.5 Testes de sincronização, conflitos e falhas

Os critérios abaixo são oráculos de aceite propostos, não resultados de execução. Cada fixture deve fixar conta/raiz, B/L/R, identidades, revisões, conteúdos e autorização. Conferir tanto o resultado quanto os efeitos proibidos e os bytes preservados. Os casos C/E/P/N são definidos no [catálogo](../engine/src/conflicts/validation-checklist.md).

| Critério / requisitos | Condição e evento | Resultado exigido e efeitos proibidos |
| --- | --- | --- |
| AC-CON-01 / RF-CON-01 | B confirmado; variar L/R entre inalterado, modificado e ausente confirmado nas nove combinações da matriz do catálogo. Repetir com observação desconhecida. | Classificação determinística conforme a matriz; nenhuma escrita na classificação. Ausência confirmada não se confunde com desconhecido; exclusão candidata permanece bloqueada. |
| AC-CON-02 / RF-CON-01 | Sem B, apresentar arquivos preexistentes; depois apresentar conteúdo equivalente com timestamps distintos e evidência suficiente de identidade. | Sem baseline inventado ou sobrescrita inicial. Equivalência comprovada não produz conflito de conteúdo; evidência insuficiente não é equivalência. |
| AC-CON-03 / RF-CON-02 | Itens diferentes disputam um caminho, ou um ancestral muda de identidade; incluir renomeação e modificação independentes. | Colisão bloqueia item e dependentes sem eleger vencedor por nome. Mudanças independentes só são conciliáveis com identidade e destino demonstrados. |
| AC-CON-04 / RF-CON-03 | Duas versões divergentes; interromper preservação, esgotar espaço e ocupar o nome reservado. | Conteúdo original intacto; nenhuma versão declarada preservada sem verificação. Repetição não sobrescreve cópia alheia nem cria cópias indefinidamente. |
| AC-CON-05 / RF-CON-04 | Adiar conflito de um item, sincronizar outro independente e reiniciar. | Mesma pendência reaparece com identidade e revisões; trabalho independente progride. Descendentes dependentes permanecem bloqueados. |
| AC-CON-06 / RF-CON-05 | Escolher cada resolução com versões estáveis; repetir após mudança de conteúdo, identidade, autorização ou escopo. | Ação suportada segue os efeitos de 6.4 com preservação; ação não suportada é recusada. Decisão obsoleta não altera conteúdo nem é aplicada a outra conta/raiz. |
| AC-CON-07 / RF-CON-06 | Simular remoção confirmada, perda de acesso, inventário incompleto e grande conjunto de ausências; solicitar preferência por data ou mesclagem automática. | Causas distintas na saída; nenhuma exclusão propagada, escolha por timestamp, mesclagem automática ou ação destrutiva em lote. |
| AC-REC-01 / RF-REC-01 | Interromper antes e depois do efeito externo e antes/depois da confirmação durável; tornar a resposta remota incerta. | Reinício confirma efeito demonstrado ou mantém pendência; repetir não duplica item nem perde versão. Sem evidência suficiente, nenhuma repetição com efeito incerto. |
| AC-REC-02 / RF-REC-02 | Remover, corromper ou substituir por formato incompatível o estado de uma raiz antes gerenciada. | Arquivos e evidências restantes preservados; erro explícito e bloqueio de decisões dependentes. Nenhum reset silencioso ou inferência de exclusão. |
| AC-REC-03 / RF-REC-03 | Desmontar raiz, revogar acesso, trocar identidade/configuração ou iniciar ator concorrente na mesma raiz durante trabalho. | Escopo afetado suspenso; nenhum acesso fora da raiz ou entre contas. Efeitos anteriores reconciliados antes de novo plano; ator concorrente não recebe autorização conflitante. |
| AC-REC-04 / RF-REC-04 | Injetar erro transitório, permanente e ambíguo; pausar durante espera e restaurar conectividade. | Categoria correta, tentativas dentro do limite definido pelo contrato, pausa mantida e nenhum retry cego de resultado ambíguo. |
| AC-REC-05 / RF-REC-05 | Alterar arquivo durante transferência, produzir checksum divergente, cancelar ou falhar na gravação do estado final. | Nenhuma confirmação falsa ou baseline avançado; arquivo parcial não publicado como definitivo e conteúdo previamente confirmado preservado. |
| AC-UX-01 / RF-CON-03–05, RF-REC-03–04 | Abrir, adiar e reabrir pendências por teclado e leitor de tela; incluir cópia inacessível e decisão invalidada. | Origem, motivo, consequência e preservação real compreensíveis; ações impossíveis indisponíveis; nenhuma indicação de sucesso global com pendências. |

Executar primeiro classificação com entradas simuladas; depois efeitos com filesystem/estado isolados e provedor controlado, incluindo injeção de falhas e reinício. Testes com API real e desktop são evidências adicionais, exigindo conta e pasta exclusivas com dados dispensáveis. Só declarar um requisito validado com referência ao teste, ambiente e resultado; os 56 itens permanecem pendentes até evidência específica.

### 10.6 Testes de segurança e privacidade

### 10.7 Testes GNOME e Flatpak

### 10.8 Matriz de rastreabilidade

Esta matriz vincula requisitos ao catálogo e aos critérios novos; não atribui cobertura executada. Os agrupamentos são relacionados, não equivalências entre um teste e todos os cenários do grupo.

| Requisitos | Casos relacionados | Critérios de aceite |
| --- | --- | --- |
| RF-CON-01 | C01, C03–C06, C21, C25, N01, N12, P01 | AC-CON-01–02 |
| RF-CON-02 | C02, C05, C07–C20, C22–C24, N05 | AC-CON-03 |
| RF-CON-03 | P02, P08, E02, E05, E10 | AC-CON-04 |
| RF-CON-04 | P07, C16–C17 | AC-CON-05 |
| RF-CON-05 | P03, P09, N06–N07 | AC-CON-06, AC-UX-01 |
| RF-CON-06 | P04–P06, C03–C04, C08, C11, C16, C25, N08 | AC-CON-07 |
| RF-REC-01 | N11, E04–E05, P09 | AC-REC-01 |
| RF-REC-02 | N01–N02, N12 | AC-REC-02 |
| RF-REC-03 | N03–N05, N07, N09, C19, C25 | AC-REC-03 |
| RF-REC-04 | E01–E04, E07–E09 | AC-REC-04, AC-UX-01 |
| RF-REC-05 | E05–E06, E10, N10 | AC-REC-05 |

Cada caso relacionado ainda requer fixture e evidência próprias. P04 (preferência por data), P05 (mesclagem) e P06 (propagação de exclusões) são cobertos aqui por critérios de bloqueio/exclusão de escopo, não por implementação dessas capacidades. Particularidades de nomes, links, atributos, tipos e restaurações continuam exigindo detalhamento nos contratos antes de sua execução.

## 11. Distribuição e operação

### 11.1 Ambientes Development e Production

### 11.2 Compilação, empacotamento e versionamento

### 11.3 Automação e entrega via GitHub Actions

### 11.4 Instalação, atualização e remoção

### 11.5 Configuração e verificação OAuth para distribuição

### 11.6 Canais de distribuição e suporte

## 12. Evolução e governança

### 12.1 Situação atual dos requisitos

A inspeção de código permite localizar comportamento existente, mas não demonstra aceite dos requisitos completos. A evidência de execução deve acompanhar os commits ou testes correspondentes.

| Área | Comportamento existente e referência | Lacuna frente à especificação |
| --- | --- | --- |
| Classificação | [planner](../engine/src/planner/mod.rs) usa evidência local atual e uma base por ID remoto, extraída do último registro durável `Installed`/`Updated`; a base vincula revisão remota, SHA-256 e identidade local (dispositivo/inode). Distingue item sem base, mudança local isolada e mudanças concorrentes; a classificação não executa operações. | [SYNC-003](../engine/src/changes/reconciliation.rs) integra inventário local completo, diagnóstico de indisponibilidade remota e equivalência forte para mudanças concorrentes do mesmo objeto com B. Exclusões, atualização remota e cobertura integral de RF-CON-01–02 continuam sem aceite completo. |
| Preservação e retomada local | [atualizações de download](../engine/src/download/execution/updates.rs) preservam a versão anterior e reconciliam fases de instalação interrompida. | Não equivalem ao registro geral de conflitos nem à resolução assistida RF-CON-03–05; aceite completo de RF-REC-01/05 pendente. |
| Estado | [state](../engine/src/state/mod.rs) separa observações de base instalada e rejeita banco existente inválido/incompatível; o journal por raiz em [downloads](../engine/src/state/downloads.rs) fornece B apenas após confirmação durável da instalação e invalida candidatos quando a operação mais recente está incompleta. | Uma raiz sem journal não pode reconstruir B; reconstrução local explícita disponível via STATE-003; recuperação de intents remotos perdidos e aceite real continuam limitados conforme o contrato. |
| Upload | [upload](../engine/src/upload/mod.rs) e [execução](../engine/src/upload/execution.rs) tratam criação de itens com autorização específica, estabilização, temporários, snapshots privados e revalidação da origem antes dos blocos e da publicação final. | Não autorizam atualização remota de existentes nem propagação de exclusões. |
| Conflitos gerais | [conflicts](../engine/src/conflicts/mod.rs) centraliza a classificação pura e os adaptadores do planner, incluindo conciliação candidata de localização/conteúdo e distinção entre equivalência, conflito e falta de evidência. | Aplicação bidirecional de rename/move, preservação geral com cópias e resolução assistida permanecem planejadas; a classificação não confirma sua execução. |
| Coordenação | [coordinator](../engine/src/coordinator/mod.rs) verifica identidade/disponibilidade da raiz e classifica suspensões; a fachada retém bloqueio por ciclo e `state::Roots` persiste vínculo e motivo fora do volume. | A coordenação geral continua na fachada; validação real de desmontagem, permissões Google, portal e reinício do aplicativo permanece pendente. |

RF-CON-01–06 e RF-REC-01–05 estão especificados neste documento, com implementação parcial em fluxos restritos ou ainda ausente conforme a tabela. Nenhum recebe estado de validação completa por esta revisão documental.

### 12.2 Propostas de funcionalidades futuras

### 12.3 Riscos, pendências e dívida técnica

Para conflitos e recuperação, as decisões seguintes condicionam futuras implementações. Enquanto abertas, manter o bloqueio conservador correspondente:

- **Preservação:** definir armazenamento, reserva de nomes, quota, retenção e recuperação das cópias; não habilitar limpeza automática de pendências.
- **Concorrência:** detalhar contratos de revalidação local/remota, identidade e exclusão mútua por raiz; não presumir que uma consulta anterior elimina a corrida antes da escrita.
- **Estado perdido:** definir como reconhecer raiz anteriormente gerenciada e reconstruir evidências sem inventar baseline; nunca tratar ausência de banco como autorização para sobrescrever.
- **Resultado remoto ambíguo:** uploads de criação seguem SYNC-013 abaixo; definir evidências para futuras operações de atualização/exclusão e manter resultado desconhecido sem repetição cega.
- **Tentativas e lotes:** uploads de criação têm orçamento persistente SYNC-013; definir política para os demais executores. Propagação de exclusões, limiares de impacto e resolução em lote exigem especificação e revisão próprias.
- **Tipos e identidade:** detalhar renomeações, links, atributos, documentos nativos e restauração histórica; nenhuma inferência universal por nome ou timestamp.
- **Interface:** validar compreensão, acessibilidade e consequências das escolhas antes de habilitar resolução assistida; comparadores de conteúdo continuam fora da primeira implementação.

A classificação B/L/R do fluxo download-only usa inventário local com estabilização e fingerprint sem seguir symlinks. SYNC-003 acrescenta equivalência forte revalidada, observações persistidas separadamente de B e apresentação de pendências na interface existente. O journal de downloads fornece B somente após confirmação durável. Preservação e execução bidirecionais, reconstrução de B perdida, resolução assistida e aceite completo de AC-CON-01–03 continuam dependentes de contratos e validação específicos.

### 12.4 Política de mudanças da especificação

Mudanças devem explicitar o problema tratado, o comportamento esperado e o impacto sobre requisitos, políticas e critérios de aceite. Alterações que afetem preservação de dados, autorização ou operações destrutivas exigem revisão dessas garantias.

Decisões vigentes, propostas e hipóteses devem permanecer distinguíveis. Contratos não devem ser eliminados apenas por ainda não terem implementação, e propostas não devem ser apresentadas como funcionalidades disponíveis.

### 12.5 Rastreabilidade com código, testes e decisões

Requisitos devem receber identificadores estáveis quando detalhados e manter vínculos com as decisões que os justificam e os critérios que permitem verificá-los. Referências à implementação e aos testes devem ser acrescentadas na consolidação das seções técnicas.

O estado de implementação e o estado de validação devem ser registrados separadamente. A rastreabilidade deve permitir identificar tanto requisitos sem evidência quanto comportamentos que necessitem de especificação.

### 12.6 Referências documentais e acervo histórico

O acervo em [`old/`](old/README.md) deve permanecer identificável como documentação anterior. Seu aproveitamento exige revisão de contexto, coerência e vigência, preservando decisões relevantes sem transportar automaticamente descrições de versões passadas.

Esta especificação deve concentrar a definição vigente do produto. Documentos complementares podem detalhar assuntos específicos, evitando duplicação de regras e mantendo referências claras entre as fontes.

### Proteção de disponibilidade de raízes — SYNC-004

O escopo de aceite desta entrega é offline: decisões e efeitos com provedor
remoto simulado, filesystem e SQLite reais isolados, falhas determinísticas,
reabertura de estado e idempotência. Segue a separação de evidências das entregas
locais SYNC-001/SYNC-003. Testes Google, desmontagem física, reinício real do
aplicativo, portal instalado e leitor de tela são validações posteriores; não são
pré-condição do aceite offline nem recebem aceite por simulação.

Uma associação existente exige marcador local previamente vinculado ao cliente,
conta e ID remoto. O caminho existir ou estar vazio não prova identidade. A
verificação de uma associação existente não cria marcador, banco ou arquivo de
sondagem na raiz. Metadados incompatíveis não são migrados nem apagados.

Antes de inventariar ou executar efeitos, o motor verifica disponibilidade e
identidade da raiz. Mantém exclusão mútua local durante preparação e transferências;
revalida a associação e os acessos antes dos efeitos. Perda de acesso suspende o
escopo afetado, preservando base, journals e arquivos; não autoriza exclusão.

A identidade esperada e o motivo de suspensão ficam no armazenamento privado do
aplicativo, fora do volume selecionado. Sua perda não autoriza adotar outra raiz.
Uma retomada verifica novamente as raízes e produz plano novo, reconciliando fases
interrompidas antes de repetir efeitos. Pausa do usuário não é anulada por eventos
de rede. Apenas falhas transitórias de conexão permitem tentativas automáticas;
identidade divergente ou acesso negado exigem correção explícita e nova tentativa.

Uma resposta 404 não comprova exclusão: pode representar perda de acesso. Uma
pasta remota movida que conserva ID e acesso mantém sua associação. Falhas de item
não suspendem automaticamente itens independentes, salvo evidência de falha da raiz.

Aceite: N03 (volume ausente, substituído ou inacessível), N04 (raiz remota movida,
removida ou inacessível), C25 (perda de compartilhamento) e E09 (item indisponível)
exigem zero exclusões, preservação dos bytes e da base confirmada, diagnóstico
acionável, reinício e repetição sem adoção ou publicação indevida. Testes locais
com provedor simulado e validação real com Drive/Flatpak são evidências distintas.

A implementação de SYNC-004 e os oráculos locais são rastreáveis na
[matriz de disponibilidade](../engine/src/coordinator/tests/test-cases.md).
O registro externo ausente não é reconstruído automaticamente: selecionar
explicitamente a associação original permite verificar seu marcador; marcador
perdido ou diferente continua bloqueado. Resultados locais e capturas de fixtures
não conferem aceite da bateria real descrita nessa matriz.

### SYNC-012 — revalidação antes de aplicar resolução (N06/E06)

Requisito: uma decisão baseada em comparação antiga nunca age sobre versões
modificadas. Antes da ação, identidade remota, revisão e fingerprint local são
revalidados; divergência ou evidência ausente aborta a proposta, preserva a
pendência, audita o motivo e exige nova comparação. Contrato em
[conflitos](../engine/src/conflicts/contract.md#sync-012--revalidação-antes-de-aplicar-resolução-n06e06).
Validado com SQLite real e entradas simuladas; sem Google Drive real.

### SYNC-010 — cópias de conteúdo divergente (P02/P08)

Requisito: nenhuma escolha pode eliminar silenciosamente uma versão divergente.
Preservar a versão remota binária comprovada em cópia local sem sobrescrita,
manter o original local e registrar vínculo com origem/ID/revisão e integridade.
Falhas ou evidência insuficiente mantêm pendência explícita, sem avançar a base.
Reserva de nome, journal separado, recuperação e oráculos estão no
[contrato de preservação](../engine/src/conflicts/preservation.md).
Resolução assistida e políticas de retenção não fazem parte desta operação.
Implementado no executor de download com journal de preservação separado no
SQLite principal v3, nome determinístico reservado e publicação NOREPLACE.
A matriz de testes automatizados está em `download/preservation_tests.rs`;
os resultados executados ficam na PR. Simulação não comprova Google Drive real
nem queda de energia. O formato v2 é rejeitado sem apagar dados; a recuperação
explícita de estado existente permite reconstrução conservadora.


### SYNC-013 — resultado remoto incerto e tentativas de criação (N11/E04/E08)

**Requisito:** perda de resposta não prova ausência remota. Persistir intenção e
ID antes do efeito; consultar esse mesmo ID e verificar pai, nome, tipo e hashes
antes de confirmar ou retomar. Nunca gerar ID substituto nem repetir cegamente
um bloco. Evidência insuficiente ou divergente preserva arquivo e snapshot,
sem avançar a base instalada.

**Política implementada:** [contrato de criação](../engine/src/upload/create-only.md#sync-013--uncertain-outcomes-and-bounded-replay).
Cada intenção tem cinco passes de execução no máximo, com contador e prazo
persistidos antes das consultas/efeitos. A espera exponencial de 60–960 segundos
mais jitter de 0–30 segundos sobrevive à reabertura e não bloqueia o worker com
sleep. Um passe transfere vários blocos; falha/crash consome o passe reservado.
Após esgotamento, somente consultas espaçadas podem confirmar efeito remoto já
concluído. Não há reset automático, mesmo após revogação/retomada do consentimento.
Formato interno de uploads 2 rejeita e preserva formatos anteriores sem migração.

**Aceite verificável:** timeout após criação de pasta e publicação de arquivo
não duplica IDs; bloco interrompido consulta offset antes de retomar; rede
indisponível não confirma nem descarta dados; rate limit não dispara chamadas
do adaptador de upload durante espera; o sexto passe não cria sessão nem envia
bytes; reserva sobrevive à reabertura; worker concorrente não altera orçamento;
resposta tardia da quinta tentativa pode ser confirmada sem sexto efeito.
Os oráculos ficam em [upload/tests.rs](../engine/src/upload/tests.rs), com relógio
controlado, provedor simulado, filesystem e SQLite reais isolados. Resultados de
execução pertencem ao PR, sem equivalência com teste Google real ou queda de energia.

**Limites:** não é um escalonador global de chamadas da API; não interpreta
Retry-After. Inventário, OAuth e verificação geral de raiz mantêm os próprios
contratos. Não autoriza atualização/exclusão remota nem garante unicidade de
nome frente a clientes independentes. Validação Google/Flatpak empacotado é
separada e permanece no acompanhamento de upload [#43](https://github.com/rafatosta/drivesync/issues/43).

### Proteção de lotes — SYNC-014

**Intenção e política do produto.** Detectar impacto por raiz e por pasta antes de
qualquer transferência. Um lote é suspeito quando afeta pelo menos 100 itens
confirmados, ou pelo menos 10 itens e 20% da população confirmada do escopo.
Esses valores são política conservadora inicial, não limites recomendados pelo
Google; ficam em um tipo próprio validado, para configuração futura sem alterar
o algoritmo. Configuração persistida e editor de limiares não fazem parte desta
entrega. Não existe preferência para desativar a proteção nem consentimento global.

**Contrato de implementação.** A população é formada pelas identidades com base
confirmada, contadas uma vez por escopo ancestral da localização conhecida;
localização desconhecida conta conservadoramente na raiz. Ausência
local, indisponibilidade/exclusão remota, edição, substituição e mudança de localização
contam como impacto; itens novos e equivalentes não contam. Evidência inconclusiva
em item confirmado também conta conservadoramente, sem se transformar em exclusão.
O cálculo usa componentes de caminho, inclui raiz e todas as pastas ancestrais,
e não soma duplicatas de diagnóstico de uma identidade.

O coordinator salva a reconciliação completa antes de verificar o lote e interrompe
o ciclo inteiro antes de downloads, cópias de conflitos e uploads se houver impacto
suspeito. Falha de coleta/persistência também impede transferências. A base confirmada
não avança. A preparação apenas observacional continua permitida. A revisão informa
escopo, quantidade e proporção e pede uma decisão explícita com cancelamento padrão.
Confirmar permite somente uma tentativa com evidência idêntica, sob os locks normais,
sem autorizar exclusões, sobrescritas remotas ou resoluções ainda não implementadas.

A proposta contém um resumo e um digest determinístico das evidências B/L/R,
plano e escopo cliente/conta/raiz/caminho. A nova tentativa recolhe observações;
qualquer diferença invalida a proposta e exige revisão nova. Consentimento não é
persistido nem reutilizado por timers, conectividade, reinício ou nova tentativa.
As observações duráveis permitem reconstruir a revisão após reinício; cada efeito
continua sujeito às verificações individuais existentes. Não há transação atômica
entre observação e Drive: mudança durante execução pode interromper o restante do
ciclo após efeitos já confirmados, que permanecem recuperáveis pelos journals.

**Algoritmo BULK-001.** Validar limiares; para cada base com localização, associar
os diagnósticos pelo ID e verificar presença local/remota e localização atual.
Acumular população e identidades afetadas em cada ancestral. Selecionar escopos que
atinjam contagem absoluta OU mínimo e proporção; sem escopo suspeito, prosseguir.
Ordenar evidências por identidade/caminho, calcular digest e comparar com proposta
explicitamente fornecida a esta tentativa. Sem igualdade integral, retornar revisão
pendente antes do executor. Nunca converter ausência, erro ou confirmação em delete.

**Critérios de aceite e validação.** N08/P06: 4.800 ausências em 5.000 bases devem
suspender sem transferências/exclusões; pequenas edições normais devem continuar;
uma subpasta afetada deve bloquear mesmo se diluída na raiz; propostas antigas,
contas/raízes diferentes e evidências offline devem falhar fechadas. Testar SQLite
e filesystem reais isolados com provedor simulado: reabertura, cancelamento,
concorrência, repetição e preservação de bytes/base. Testes automatizados não
atestam Google Drive real nem interação manual com aplicativo empacotado.

**Fundamentação.** A persistência reaproveita transações/CAS existentes, com a
limitação de atomicidade por banco descrita em [SQLite Atomic Commit](https://www.sqlite.org/atomiccommit.html).
Falhas de acesso continuam inconclusivas conforme [Drive: resolve errors](https://developers.google.com/workspace/drive/api/guides/handle-errors).
Referências verificadas em 2026-10-30. Contagem, proporção e confirmação por lote
são política DriveSync; não são capacidades de exclusão transacional da API.

A população protetiva mantém a última base confirmada observada por identidade,
separada da base que o journal autoriza usar na execução. Operações incompletas
não reduzem essa população. A revisão SQLite usa formato interno 4, sem migração;
formato anterior é rejeitado preservando o banco. A recuperação explícita existente
pode reconstruir observações; arquivos e credenciais permanecem preservados.
Bases sem localização conhecida contam conservadoramente no escopo da raiz.
