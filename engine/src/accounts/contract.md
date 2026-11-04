# Contrato — contas

O perfil usa `google:<sub>` como identidade estável e pertence a um cliente
OAuth. O e-mail é metadado opcional, não chave. Reautorizar a mesma identidade
atualiza o perfil sem duplicá-lo; clientes e contas distintos permanecem
isolados. Perfis e configurações não contêm tokens.

Listar perfis não comprova autenticação. Restaurar verifica as credenciais de
cada conta e retorna erros individuais; falha de rede ou do cofre não apaga
perfis. Ausência confirmada de credencial permite remover o perfil.

Sair é uma ação explícita: remove a credencial local e o perfil da conta,
incluindo a configuração de pastas. Falhas são reportadas; não se excluem
arquivos locais ou remotos nem se revoga o acesso no Google.

O índice tem formato versionado atual; formatos inválidos ou incompatíveis
são rejeitados sem apagar dados. As gravações são serializadas entre processos
e usam substituição atômica. Veja [persistência](algorithms.md).
