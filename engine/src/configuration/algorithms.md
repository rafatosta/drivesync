# Seleção de pastas

1. Listar e selecionar uma pasta remota por ID e revalidar seus metadados antes
   de salvar; não aceitar arquivo, atalho ou item na lixeira.
2. Selecionar um diretório local existente. Canonicalizar o caminho e resolver
   aliases do Documents portal quando necessário, mantendo o caminho exportado
   para acesso e o caminho de origem para exibição e comparação.
3. Confirmar que o diretório é gravável com um arquivo temporário exclusivo.
4. Sob o lock do índice, rejeitar sobreposição e persistir a seleção
   atomicamente. Não criar metadados em pastas do usuário.

Falhas do portal, da API ou do filesystem são reportadas, sem concluir a
configuração. Veja [contrato](contract.md).
