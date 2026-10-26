# Invariantes globais

1. Nenhum arquivo alterado pode ser substituído ou excluído silenciosamente.
2. Falha de rede não deve ser confundida com exclusão remota.
3. O estado persistente nunca pode avançar além de alterações recuperáveis.
4. Operações interrompidas devem ser reconciliadas antes de nova operação destrutiva.
5. Uma inconsistência no banco de estado bloqueia operações destrutivas até reconciliação.
6. Credenciais e tokens nunca aparecem em logs.
7. Toda mudança remota ou local exige identidade e versão verificadas.
