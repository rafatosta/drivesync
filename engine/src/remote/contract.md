# Contrato — pastas remotas

Consultar metadados e listar pastas por conta, com paginação e IDs validados.
Aceitar somente pastas acessíveis e não descartadas; nomes não identificam
recursos. Erros de autenticação, rede, resposta e acesso são reportados sem
expor tokens ou corpos privados da resposta.

Este adaptador não transfere, modifica ou remove conteúdo. Os testes usam
respostas sintéticas; acesso real ao Drive e ao portal requer validação
separada.
