# Google

## Objetivo

Disponibilizar a API do Google Drive, incluindo as operações de autenticação Google necessárias ao seu acesso, por meio de funções que abstraem a comunicação com os serviços externos.

## Execução e acompanhamento técnico

As funções que acessam o Google podem realizar I/O de rede e devem ser implementadas sem bloquear o loop GTK. O mapeamento de funções rápidas e puras pode continuar síncrono; a escolha definitiva de assinaturas `async` e dos tipos de stream será validada na implementação Rust.

**Não há estado global de execução do Google Provider.** O status de cada requisição é sua resposta ou `GoogleApiFailure`. Paginação de listas e mudanças, `GoogleUploadProgress`, `GoogleDownloadStream` e `GoogleOperation` refletem apenas estados técnicos retornados pelo protocolo. O progresso completo de upload/download e o gerenciamento de operações são responsabilidades do Operador; este provedor não registra jobs, não faz reconciliação nem mantém fila.

Timeout, quota, `Retry-After`, autenticação expirada e efeitos remotos incertos devem ser traduzidos de forma fiel, sem supor que nova tentativa é sempre segura.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)
