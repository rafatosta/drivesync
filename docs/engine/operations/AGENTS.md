# Orientações para agentes — Operador

## Objetivo

Implementar a API pública de execução segura de comandos de manipulação e transferência de arquivos e diretórios locais e remotos.

## Fontes do contrato

- [README.md](README.md): objetivo e comportamento do Operador.
- [INTERFACES.md](INTERFACES.md): operações públicas, requisições, resultados e garantias.
- [TESTS.md](TESTS.md): verificações funcionais e critérios de aceite.
- [AGENTS.md da raiz](../../../AGENTS.md): padrões gerais do repositório.

A documentação descreve uma arquitetura proposta e não comprova a existência das interfaces em código Rust.

## Diretrizes de implementação

- Disponibilizar a fachada `Operator` com métodos explícitos para criação, upload, download, atualização de conteúdo, renomeação, movimentação, exclusão, lixeira e restauração.
- Receber uma requisição tipada completa para cada comando, validar identidade, destino, precondições e capacidades aplicáveis, selecionar a implementação adequada e devolver o resultado.
- Para serviços remotos, selecionar a implementação compatível com a conta/sessão e utilizar exclusivamente as funções de [Providers](../providers/INTERFACES.md).
- Preservar identidade dos recursos nas operações de atualização, renomeação e movimentação, conforme as capacidades disponíveis.
- Tratar transferências de criação separadamente de atualização de conteúdo existente.
- Utilizar operações seguras de filesystem, escrita temporária, publicação atômica e validação de integridade quando aplicável.
- Expor erros tipados, incluindo efeitos indeterminados. Cancelamento e progresso devem refletir apenas evidências observadas.
- Manter assinaturas, resultados e tratamento de erros coerentes com `INTERFACES.md`. Detalhes de implementação podem ser organizados conforme as responsabilidades reais, sem impor antecipadamente arquivos ou camadas artificiais.
- Preservar o escopo de cada alteração, a segurança de dados e as regras de documentação Rust.

## Estratégia de testes

Testar a fachada real com implementações de execução simuladas, verificando despacho por operação e provedor, validação dos parâmetros e propagação de resultados. Exercitar comportamento de arquivos em diretórios temporários reais e serviços remotos com interfaces de provedores simuladas. Cobrir segurança, concorrência, falhas, retomada e cancelamento conforme [TESTS.md](TESTS.md).

Executar comandos Rust apenas no ambiente Flatpak configurado pelo repositório. Informar os testes efetivamente executados, resultados e limitações. Registrar decisões ainda pendentes antes de atribuir comportamentos não definidos pelo contrato.

## Definições para a implementação

Revisar os tipos de referência e conteúdo, ownership e assincronicidade, a passagem segura de sessão remota e o contrato de progresso/cancelamento antes de converter as assinaturas conceituais em API compilada.
