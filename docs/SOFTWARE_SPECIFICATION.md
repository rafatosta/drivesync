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
  - [4.7 Detecção de mudanças e sincronização](#47-deteccao-de-mudancas-e-sincronizacao)
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

### 4.8 Pausa, retomada e execução em segundo plano

### 4.9 Tratamento de conflitos e exclusões

**Referência de requisitos em revisão:** [catálogo de 56 casos e políticas](../engine/src/conflicts/validation-checklist.md). Distinguir conflito, equivalência, incompatibilidade e falha; proteger versões e operações em massa. Os códigos não correspondem a 56 funcionalidades independentes.

### 4.10 Recuperação de falhas e reconciliação

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

**Referências:** [viabilidade Rust/Drive API](../engine/src/conflicts/technical-feasibility.md) e [pesquisa de experiência e ferramentas existentes](../engine/src/conflicts/ux-research.md). Preferências automáticas somente por opção explícita e sem ignorar salvaguardas.

### 6.5 Política de exclusões e operações destrutivas

Falhas de acesso, comunicação ou autenticação não constituem evidência suficiente de exclusão. Uma operação destrutiva exige identidade, estado e autorização compatíveis com a política definida para o recurso.

Diante de inconsistência ou resultado incerto de uma operação anterior, novas operações destrutivas devem permanecer bloqueadas até reconciliação. A conveniência de automatizar uma ação não justifica ignorar a preservação de versões ou o controle do usuário.

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

**Diretriz UX em estudo:** central de atenção com comparação e consequências das decisões, separando conflitos, incompatibilidades e erros. Ver [pesquisa de UX](../engine/src/conflicts/ux-research.md).

### 9.6 Integração com o ambiente GNOME

## 10. Verificação e validação

### 10.1 Estratégia de testes

A verificação deve partir dos requisitos, das invariantes e dos efeitos observáveis esperados. Deve combinar avaliação de regras isoladas, integração entre responsabilidades e cenários de ponta a ponta nos ambientes declarados como suportados.

Cenários de falha, interrupção, concorrência e isolamento de contas devem acompanhar os fluxos de sucesso. Testes simulados e testes com serviços reais fornecem evidências diferentes e devem ser identificados separadamente.

### 10.2 Critérios de aceite dos requisitos

Cada requisito deve ter critérios que descrevam condições iniciais, ação ou evento, resultado esperado e efeitos proibidos. Critérios de preservação devem verificar também o conteúdo que não pode ser alterado.

A existência de código ou a compilação bem-sucedida não bastam para declarar aceite. A validação deve identificar cenário, ambiente, resultado observado e limitações da evidência.

### 10.3 Testes unitários e de integração

### 10.4 Testes com a API Google

### 10.5 Testes de sincronização, conflitos e falhas

**Fonte de rastreabilidade:** [checklist C01–C25, E01–E10, P01–P09, N01–N12](../engine/src/conflicts/validation-checklist.md). Todos pendentes de evidência por cenário.

### 10.6 Testes de segurança e privacidade

### 10.7 Testes GNOME e Flatpak

### 10.8 Matriz de rastreabilidade

## 11. Distribuição e operação

### 11.1 Ambientes Development e Production

### 11.2 Compilação, empacotamento e versionamento

### 11.3 Automação e entrega via GitHub Actions

### 11.4 Instalação, atualização e remoção

### 11.5 Configuração e verificação OAuth para distribuição

### 11.6 Canais de distribuição e suporte

## 12. Evolução e governança

### 12.1 Situação atual dos requisitos

### 12.2 Propostas de funcionalidades futuras

### 12.3 Riscos, pendências e dívida técnica

### 12.4 Política de mudanças da especificação

Mudanças devem explicitar o problema tratado, o comportamento esperado e o impacto sobre requisitos, políticas e critérios de aceite. Alterações que afetem preservação de dados, autorização ou operações destrutivas exigem revisão dessas garantias.

Decisões vigentes, propostas e hipóteses devem permanecer distinguíveis. Contratos não devem ser eliminados apenas por ainda não terem implementação, e propostas não devem ser apresentadas como funcionalidades disponíveis.

### 12.5 Rastreabilidade com código, testes e decisões

Requisitos devem receber identificadores estáveis quando detalhados e manter vínculos com as decisões que os justificam e os critérios que permitem verificá-los. Referências à implementação e aos testes devem ser acrescentadas na consolidação das seções técnicas.

O estado de implementação e o estado de validação devem ser registrados separadamente. A rastreabilidade deve permitir identificar tanto requisitos sem evidência quanto comportamentos que necessitem de especificação.

### 12.6 Referências documentais e acervo histórico

O acervo em [`old/`](old/README.md) deve permanecer identificável como documentação anterior. Seu aproveitamento exige revisão de contexto, coerência e vigência, preservando decisões relevantes sem transportar automaticamente descrições de versões passadas.

Esta especificação deve concentrar a definição vigente do produto. Documentos complementares podem detalhar assuntos específicos, evitando duplicação de regras e mantendo referências claras entre as fontes.
