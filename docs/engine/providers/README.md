# Providers — APIs externas

## Objetivo

O módulo `providers/` disponibiliza **exclusivamente o mapeamento direto de APIs de serviços externos**. Cada submódulo representa um provedor e expõe suas operações oficiais por funções próprias, sem definir como os consumidores as utilizam.

A responsabilidade se limita a endpoints, métodos HTTP, parâmetros, cabeçalhos, formatos de entrada e saída, autenticação necessária às chamadas, comunicação e representação de respostas e erros técnicos.

## Limites

- Não implementar lógica funcional do DriveSync, sincronização, gerenciamento de downloads ou uploads, filas, conflitos ou persistência de estado da aplicação.
- Disponibilizar operações de conteúdo ou envio de bytes que façam parte **da própria API** não equivale a implementar um gerenciador de transferência.
- Não armazenar credenciais, tokens, contas ou sessões persistentes; apenas aceitar e processar dados exigidos pelas chamadas.
- Não impor API universal comum a todos os serviços, nem inventar funcionalidades inexistentes.
- Não conhecer nem documentar módulos consumidores.
- Não duplicar camadas de API: `providers/google/` é diretamente o mapeamento Google.

## Submódulos

- [Google](google/README.md): mapeamento específico da API Google Drive e dos protocolos Google necessários.

Outros submódulos serão documentados apenas quando houver implementação correspondente. A organização dos arquivos de código internos será decidida na implementação, sem estrutura antecipada.

## Documentação

Os quatro arquivos documentais deste módulo e de cada submódulo têm papéis distintos:

- [README.md](README.md): objetivo, escopo e limites.
- [INTERFACES.md](INTERFACES.md): interface pública e operações expostas.
- [TESTS.md](TESTS.md): testes de conformidade do mapeamento.
- [AGENTS.md](AGENTS.md): orientações específicas aos agentes.

**Estado:** especificação documental; não constitui prova de implementação Rust ou testes executados.
