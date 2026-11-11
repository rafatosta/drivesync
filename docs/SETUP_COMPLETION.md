# Conclusão do setup e edição posterior

O setup inicial mantém as escolhas somente em memória. O botão **Concluir
configuração** na revisão executa a validação final e salva um registro completo
antes de trocar o conteúdo da janela pelo dashboard. Não inicia sincronização.

## Validação e salvamento

`services::configuration::save_checked` confirma novamente:

1. A conta existe na engine e está autorizada.
2. A pasta local é um diretório local acessível com permissão anunciada de escrita.
3. O ID remoto escolhido ainda corresponde a uma pasta existente, não excluída,
   acessível pela sessão daquela conta. A consulta usa a API pública da engine,
   que chama o `files_get` existente de Providers.
4. O registro foi substituído atomicamente com permissão privada.

As escolhas concluídas ficam em
`$XDG_CONFIG_HOME/drivesync/connections/<account-id-em-hexadecimal>.ini`, com versão,
identidade da conta, URI local, ID remoto e descrição da pasta. Não contêm tokens
ou senha. O caminho físico é interno; a interface não depende de uma flag de
conclusão nem reaproveita rascunhos legados como configuração válida.

Falhas de validação ou escrita mantêm o usuário no setup. Uma solicitação para
fechar durante a conclusão aguarda a operação; o logout provisório não concorre
com a gravação. Antes da conclusão, cancelar ou fechar continua descartando as
escolhas e desconectando contas provisórias.

## Dashboard e inicialização

Após o salvamento, o dashboard substitui o setup na mesma janela e sem uma entrada
no histórico para voltar ao assistente. Seu conteúdo é vazio, com o menu do
aplicativo no cabeçalho. Não contém editor de conta, configuração de pastas ou
atalho de adicionar conta.

Na inicialização, registros completos mantêm o destino no dashboard mesmo quando
a conta exige reautorização ou uma pasta está temporariamente indisponível. Esses
problemas são corrigidos em Preferências; nunca causam exclusão automática de
uma configuração concluída. Um arquivo ilegível ou malformado produz um erro e
preserva a conta. Escolher novamente uma conta já configurada na adição de contas preserva seu
registro e volta ao dashboard, sem torná-la provisória. Contas sem registro
concluído seguem a limpeza de setup
abandonado já existente.

## Preferências

- **Configuração** mostra as pastas por conta. Escolher novas pastas altera um
  rascunho local da página; **Salvar** executa a mesma validação final antes de
  substituir o registro. **Cancelar** restaura os valores anteriores.
- **Contas** concentra nome, autorização, desconexão e adição de contas.
- Aparência e execução em segundo plano continuam independentes.

Desconectar uma conta remove suas credenciais/metadados pela engine e tenta
remover também as escolhas concluídas e rascunhos legados. Uma falha nessa
limpeza é informada separadamente; ela não restaura a conta removida.

## Verificação

Os testes usam respostas Google sintéticas, bancos e perfis XDG isolados. Cobrem
validação do ID remoto, rejeição de arquivos/inacessibilidade, isolamento entre
contas, leitura estrita de registros, substituição privada e conclusão/reabertura
na mesma janela. O fixture visual usa um adaptador de pastas e dados de conta
fictícios; ele não comprova consentimento Google nem portal real.

A validação local de permissões não garante acesso futuro ao portal ou que os
arquivos já foram sincronizados. A engine atual não possui um processo integrado
de sincronização; concluir setup não representa sucesso de transferência.
