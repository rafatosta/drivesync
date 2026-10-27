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

### 1.2 Versão e estado da especificação

### 1.3 Histórico de revisões

### 1.4 Fontes e referências

## 2. Visão geral do produto

### 2.1 Problema e motivação

O DriveSync é um cliente independente do Google Drive para Linux. A interface usa GTK4 e Libadwaita e segue as diretrizes de design do GNOME, sem restringir o aplicativo a esse ambiente de desktop. Flatpak é uma tecnologia de distribuição, não parte da definição do produto. A compatibilidade das integrações com o desktop deve ser validada nos ambientes de destino.

### 2.2 Objetivos do produto

### 2.3 Público-alvo e partes interessadas

### 2.4 Escopo do produto

### 2.5 Funcionalidades incluídas

### 2.6 Funcionalidades excluídas e limitações

### 2.7 Glossário e terminologia

## 3. Contexto e restrições

### 3.1 Ambiente Linux e diretrizes de design GNOME

### 3.2 Sistemas e serviços externos

### 3.3 Google Drive API e OAuth 2.0

### 3.4 Sistema de arquivos e armazenamento local

### 3.5 Premissas e dependências

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

### 5.2 Privacidade e tratamento de dados

### 5.3 Integridade e prevenção de perda de dados

### 5.4 Confiabilidade, resiliência e disponibilidade

### 5.5 Desempenho e consumo de recursos

### 5.6 Usabilidade e acessibilidade

### 5.7 Compatibilidade e portabilidade

### 5.8 Observabilidade e diagnósticos

### 5.9 Manutenibilidade e testabilidade

## 6. Regras de negócio e políticas operacionais

### 6.1 Identidade e isolamento de contas

### 6.2 Vinculação de pastas e escopo de sincronização

### 6.3 Política de upload e download

### 6.4 Política de conflitos e alterações concorrentes

**Referências:** [viabilidade Rust/Drive API](../engine/src/conflicts/technical-feasibility.md) e [pesquisa de experiência e ferramentas existentes](../engine/src/conflicts/ux-research.md). Preferências automáticas somente por opção explícita e sem ignorar salvaguardas.

### 6.5 Política de exclusões e operações destrutivas

### 6.6 Política de falhas, retries e retomadas

### 6.7 Política de consistência e persistência de estado

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

### 10.2 Critérios de aceite dos requisitos

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

### 12.5 Rastreabilidade com código, testes e decisões

### 12.6 Referências documentais e acervo histórico
