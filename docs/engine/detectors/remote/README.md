# Detector Remoto — observação independente do Google Drive

## Objetivo

O **Detector Remoto** é um **produtor individual de sinais de alteração remota**. Ele observa Google Drive com estratégias próprias (consultas periódicas, leitura inicial e cursor de mudanças), sem conhecer o Detector Local, o Storage do Detector ou o Orquestrador.

Seu único contrato de saída é um **sink interno de eventos individuais**, injetado pelo módulo pai Detector. **Cada chamada de emissão contém um evento**; agregação, fila, persistência de notificações, reserva e confirmação pertencem ao pai e ao Storage.

## Responsabilidades

- Validar escopo remoto/conta e sessão recebidos.
- Observar árvore por páginas na inicialização, respeitando limites e mudanças concorrentes.
- Consultar mudanças incrementais pela Google API, em intervalos próprios configuráveis, sem impor periodicidade ao Local.
- Emitir sinais de alteração, diagnóstico e reavaliação, nunca comandos de upload/download.
- Manter ou recuperar checkpoint de leitura **próprio do mecanismo remoto**, por um contrato de checkpoint injetado e independente da fila do Detector. A localização/tecnologia dessa persistência não faz parte do contrato do Remoto.
- Confirmar avanço do checkpoint somente após aceitação durável de todos os sinais da página no sink. Resposta de envio aceita sem durabilidade comprovada **não** autoriza avançar cursor.

## Limites

- Sem SQL, armazenamento de notificações, `inventory.sqlite3`, `detector.sqlite3`, Orquestrador ou gerenciamento de contas globais.
- Sem chamadas HTTP diretamente: usar [Google API](../../google_api/INTERFACES.md), inclusive `files.list` e `changes.list` quando aplicáveis.
- `removed=true` é sinal de reavaliação (pode significar perda de acesso), não prova inequívoca de exclusão.
- Sem inferir ausência por falha 403/404, timeout, paginação incompleta, cursor inválido, escopo limitado ou campos parciais.
- O arquivo remoto é identificado por ID estável; nome, versão e pais são metadados mutáveis.

## Estratégia de funcionamento

1. Receber sessão, raiz, parâmetros de intervalo e sink individuais.
2. Durante `start`, agendar leitura inicial ou retomada incremental conforme checkpoint.
3. Em cada ciclo, consumir páginas da Google API; converter mudanças em eventos individuais; publicar ao sink.
4. Só depois que o sink confirmar persistência durável, confirmar checkpoint correspondente por seu backend injetado.
5. Consultar novamente segundo agendamento próprio. Push pode ser usado como otimização, nunca como garantia de entrega.
6. Se parar, encerrar polling; sinais já aceitos permanecem sob responsabilidade do Detector. Na próxima inicialização, recuperação por cursor ou nova leitura segura é responsabilidade deste produtor — não reconstituir necessariamente cada operação intermediária.

**O Remoto pode operar em testes isolados com servidor HTTP fake da Google API, sink fake e checkpoint fake/real isolado.** Não precisa do Local, do banco Storage nem do Detector real.

## Invariantes

1. Evento emitido **individualmente**, sem lotes para o Orquestrador.
2. Não avançar cursor antes do sink confirmar recepção durável.
3. Erro remoto ou observação parcial nunca transforma ausência em exclusão confirmada.
4. Cursor expirado exige recuperação segura sem zerar checkpoint silenciosamente.
5. Se sink estiver indisponível, parar o avanço e repetir/reconciliar; não perder página.
6. Intervalo de polling é política própria; não afeta ciclo de vida local.
7. Nenhuma dependência concreta da implementação de `storage/` ou `local/`.

## Documentos

[INTERFACES.md](INTERFACES.md) · [TESTS.md](TESTS.md) · [AGENTS.md](AGENTS.md) · [Detector pai](../README.md).

**Estado:** especificação proposta, sem testes executados nem implementação.
