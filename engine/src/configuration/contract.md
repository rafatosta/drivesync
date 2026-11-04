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
