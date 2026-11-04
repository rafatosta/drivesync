# Contrato de configuração de pastas

Cada conta pode guardar uma pasta remota existente e uma pasta local. A escolha
remota é revalidada pela API usando a credencial da conta. A pasta local deve
ser absoluta, existente, gravável e distinta da raiz do filesystem. O caminho é
normalizado e aliases do Documents portal são resolvidos para comparação.

Não aceitar caminhos iguais ou aninhados entre contas ou clientes OAuth. A
checagem e a gravação do índice são serializadas; cancelamento ou erro não
confirma a nova seleção. Verificar a pasta local cria e remove apenas um
arquivo temporário exclusivo.

Configurar, trocar ou restaurar pastas não transfere nem apaga arquivos.
Nenhum estado de sincronização anterior é lido ou migrado.

Uma concessão direta somente de leitura pode ser usada pelo observador, mas
não comprova acesso de escrita para configurar a pasta. Se a origem direta não
passar na prova de escrita, preservar o caminho gravável concedido pelo seletor
quando ele passar nessa prova. Não ampliar permissões nem substituir silenciosamente
a capacidade de escrita por um caminho somente de leitura.
