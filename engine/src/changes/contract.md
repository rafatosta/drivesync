# Contrato — changes

Contratos vigentes: [feed incremental](incremental.md) e
[reconciliação SYNC-003](reconciliation.md). Consumidores: fachada, planner e
executores existentes. O módulo combina observações locais/remotas com B e
histórico; pode solicitar leitura remota para comprovar equivalência.

Resultados são planos e diagnósticos, nunca autorização independente para
instalar, publicar ou excluir conteúdo. O executor revalida os efeitos permitidos.
Estado desconhecido, falha de rede e desaparecimento no feed não autorizam
exclusão. Cancelamento, erros e recuperação seguem os contratos vinculados.
