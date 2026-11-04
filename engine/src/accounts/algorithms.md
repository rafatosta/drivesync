# Persistência de contas

O índice `$XDG_DATA_HOME/drivesync/accounts.json` contém perfis e seleções de
pastas por cliente OAuth, sem credenciais. O caminho XDG relativo não é aceito.
O formato atual valida versão, identidades, duplicatas e limites antes de usar
os dados. Um erro de leitura ou versão incompatível é explícito e preserva o
arquivo original.

Alterações relêem e gravam o índice sob lock entre processos; a substituição
usa arquivo temporário privado no mesmo diretório, sincronização e rename. A
checagem de sobreposição de pastas ocorre sob o mesmo lock da gravação.
Reautorização pode atualizar o perfil sem alterar outra conta.

Credenciais são armazenadas no cofre, não no índice. Uma falha após a gravação
no cofre e antes da gravação do perfil é reportada; não há migração ou
recuperação de formatos anteriores. Os testes isolados estão em
[`tests.rs`](tests.rs); os limites de verificação estão na
[especificação](../../../docs/SOFTWARE_SPECIFICATION.md).
