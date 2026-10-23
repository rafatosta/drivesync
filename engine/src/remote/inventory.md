# Inventário inicial somente de leitura

Unidade REMOTE-001/002: inventariar a raiz configurada antes de planejar downloads.
Não constitui sincronização, snapshot consistente do Drive nem autorização para
substituir arquivos locais. Não avança cursores nem grava estado de sincronização.

Entrada: conta identificada, raiz remota configurada e token de cancelamento.
A fachada obtém a credencial da própria conta e resolve a identidade real da raiz.
Saída: observações tipadas com conta, raiz, IDs, pai, nome, versão, tamanho,
checksum disponível e classificação. Nomes iguais preservam IDs distintos.

Algoritmo: fila de pastas iniciada na raiz; para cada pasta percorrer todas as
páginas, inclusive páginas vazias com continuação; validar referências, IDs,
versões e tokens; registrar itens e enfileirar apenas pastas suportadas. Rejeitar
IDs repetidos, ciclos de paginação, resposta incompleta e limites excedidos.
Verificar cancelamento antes/depois de consultas e durante a classificação.
Erros interrompem o inventário sem publicar um resultado parcial como completo.

Políticas: 100.000 itens e 10.000 páginas por inventário; limites explícitos
retornam erro, nunca truncamento silencioso. Pastas de nomes inseguros ou Drives
compartilhados não são percorridas. Documentos Workspace, atalhos e tipos Google
especiais são classificados como ignorados; arquivos sem permissão de download
ou metadados mínimos de integridade não são candidatos a download. Nenhuma
consulta de conteúdo, criação local, upload ou exclusão ocorre nesta unidade.
A transferência futura deve consultar novamente metadados e validar conflitos;
a árvore pode mudar enquanto o inventário é produzido.

Testes: árvore aninhada, múltiplas páginas/página vazia, duplicatas de nomes,
IDs/tokens repetidos, pais inconsistentes, tipos ignorados, metadados inválidos,
erros propagados, cancelamento e limites. Usar provedores falsos, sem credenciais.

Referências oficiais consultadas em 2026-10-23:
- https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list
- https://developers.google.com/workspace/drive/api/reference/rest/v3/files
- https://developers.google.com/workspace/drive/api/guides/manage-downloads
