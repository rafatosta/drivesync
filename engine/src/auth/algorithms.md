# Algoritmos de autenticação

O login usa Authorization Code com PKCE S256, state aleatório e callback
loopback. Após trocar o código e verificar identidade e permissões, salva a
credencial no Secret Service antes de declarar sucesso.

A renovação é serializada por conta e preserva a credencial existente se
falhar. O logoff é explícito, serializado com o uso da credencial e remove a
credencial local sem revogar a autorização no Google. Erros de rede ou do
cofre não comprovam que uma credencial foi removida.

Detalhes e limites: [contrato](contract.md) e
[oráculos automatizados](tests/test-cases.md). Testes reais de Google,
navegador e cofre exigem validação independente.
