# Operador — TESTS

## Objetivo

Verificar que cada método público executa o comando solicitado no recurso e destino corretos, respeita suas precondições e informa o resultado observável com segurança.

Os testes seguem [INTERFACES.md](INTERFACES.md) e [README.md](README.md). Cada método deve ser testado em isolamento, com diretórios temporários reais para operações locais e implementações simuladas de interfaces de provedores para operações remotas.

## Fixtures

- `local_tree`: árvore temporária com arquivos pequenos, grandes, diretórios e nomes válidos/limítrofes.
- `remote_fake`: interface simulada de provedor com IDs estáveis, versões, metadados e erros configuráveis.
- `content_versions`: duas versões distintas do mesmo recurso, com conteúdo e hashes conhecidos.
- `conflicting_target`: nome de destino ocupado ou recurso com revisão divergente.
- `partial_transfer`: fluxo interrompido após alguns bytes, com progresso e estado de retomada controlados.
- `unknown_outcome`: aplicação remota bem-sucedida seguida de perda da resposta.

## Casos por método público

| Caso | Método | Sucesso esperado | Falhas e verificações |
| --- | --- | --- | --- |
| OPR-01 | `new` | Prepara interface com dependências válidas | Configuração inválida e provedor indisponível para seleção |
| OPR-02 | `create_file` | Cria recurso com identidade e metadados retornados | Nome inválido, pai inexistente, destino ocupado, conteúdo não suportado |
| OPR-03 | `create_directory` | Cria pasta no destino correto | Pai inacessível, colisão, permissão negada |
| OPR-04 | `upload` | Cria recurso remoto e envia todo o conteúdo | Origem não local, falha parcial, versão/identidade retornada, integridade |
| OPR-05 | `download` | Cria arquivo local com conteúdo correto | Origem não remota, falha parcial, arquivo final íntegro e publicação segura |
| OPR-06 | `update_content` | Atualiza conteúdo preservando identidade do destino | Precondição divergente, falha de escrita/rede, destino inexistente |
| OPR-07 | `rename` | Altera somente o nome do recurso | Nome inválido, conflito, revisão divergente, identidade preservada |
| OPR-08 | `move_item` | Altera localização no mesmo ambiente | Ciclo de diretórios, destino incompatível, conflito, identidade preservada |
| OPR-09 | `delete` | Remove exclusivamente recurso identificado | Identidade errada, permissão negada, precondição divergente |
| OPR-10 | `trash` | Envia recurso à lixeira e retorna resultado disponível | Lixeira indisponível, acesso negado, conflito de versão |
| OPR-11 | `restore` | Restaura o recurso com identidade e localização observadas | Recurso não restaurável, destino ocupado, falta de permissão |

## Casos transversais

### Seleção de executor

- OPR-S-01: referência local seleciona execução local.
- OPR-S-02: referência remota seleciona o especialista correspondente ao provedor da sessão.
- OPR-S-03: contas e provedores diferentes utilizam seus próprios contextos, sem misturar identidades.
- OPR-S-04: capacidades indisponíveis retornam erro tipado.

### Segurança e integridade

- OPR-I-01: precondição de versão divergente interrompe alteração antes do efeito quando a plataforma oferece verificação atômica.
- OPR-I-02: download interrompido preserva um arquivo final anterior e mantém dados temporários recuperáveis quando apropriado.
- OPR-I-03: upload interrompido distingue recursos incompletos, falha confirmada e efeito indeterminado.
- OPR-I-04: tamanho/hash disponíveis são verificados com expectativas independentes da implementação.
- OPR-I-05: renomear/mover recurso remoto usa sua identidade e não exige retransferência de conteúdo quando a API suporta essa operação.
- OPR-I-06: exclusão e movimentação respeitam exatamente o alvo informado, inclusive perante IDs/caminhos semelhantes.
- OPR-I-07: caminhos com `..`, symlinks, nomes reservados e alterações concorrentes recebem tratamento seguro.
- OPR-I-08: uma resposta remota perdida após efetivar a ação gera resultado indeterminado; nova tentativa requer verificação prévia.
- OPR-I-09: cancelamento e progresso não alteram indevidamente o estado final ou a integridade de recursos.

### Comportamento das respostas

- OPR-R-01: `ItemResult` retorna a identidade realmente obtida após a operação.
- OPR-R-02: campos de metadados indisponíveis permanecem opcionais.
- OPR-R-03: `VerificationStatus::Verified` exige evidência concreta.
- OPR-R-04: `bytes_transferred` contabiliza os bytes efetivamente transferidos na execução.
- OPR-R-05: falhas devolvem `OperatorError` apropriado, sem vazar credenciais ou dados sensíveis.

## Testes unitários da biblioteca privada

A biblioteca privada possui uma matriz própria, vinculada à seção [8.4 de INTERFACES.md](INTERFACES.md#84-comportamento-verificável-de-cada-função). Exercitar as funções com arquivos e diretórios temporários **reais** em um escopo isolado, incluindo casos de erro. Não utilizar arquivos pessoais. As asserções devem inspecionar o filesystem após cada operação, independentemente dos valores retornados.

### Casos por função

| Caso | Função | Preparação e ação | Evidência de sucesso | Falhas e invariantes |
| --- | --- | --- | --- | --- |
| OPR-L-01 | `open_local_resource` | Abrir arquivo existente sob raiz temporária | Referência aponta ao recurso correto e conserva o escopo | Arquivo ausente, `..`, symlink externo, caminho absoluto fora do escopo e substituição concorrente são rejeitados |
| OPR-L-02 | `local_metadata` | Consultar arquivo conhecido e diretório | Tipo/tamanho e demais atributos disponíveis correspondem a consultas independentes | Permissão negada, referência obsoleta, metadados opcionais indisponíveis; nenhuma informação inventada |
| OPR-L-03 | `read_local` | Ler conteúdo binário em múltiplos blocos | Bytes recebidos são idênticos ao conteúdo original; arquivo não é modificado | Diretório como fonte, arquivo inacessível e erro de leitura são propagados |
| OPR-L-04 | `create_local_directory` | Criar subdiretório com nome válido | Exatamente uma nova entrada diretório aparece sob o pai esperado | Nome vazio, separador, `..`, colisão e falta de permissão não alteram outras entradas |
| OPR-L-05 | `create_local_file` | Criar arquivo regular vazio em diretório temporário | Arquivo criado tem tamanho zero, referência correta e permissões previstas | Colisão com arquivo/diretório existente preserva os bytes anteriores; nome inválido falha |
| OPR-L-06 | `create_temp_file` | Criar dois temporários no mesmo diretório | Nomes/identidades são exclusivos; nenhum arquivo final é criado | Falha de criação, diretório sem acesso e descarte/limpeza não alteram destino final |
| OPR-L-07 | `write_local` | Copiar dados binários em blocos, inclusive leitura/escrita parcial simulada | Conteúdo temporário e contador de bytes conferem; progresso é monotônico | Falha de leitura/escrita, cancelamento e espaço insuficiente não publicam destino parcial |
| OPR-L-08 | `atomic_replace` | Publicar temporário completo sobre arquivo conhecido | Novo conteúdo aparece integralmente, referência final correta; versão anterior não é visível parcialmente | Revisão/identidade divergente e falha pré-publicação preservam destino anterior; verificar efeitos de interrupção |
| OPR-L-09 | `rename_local` | Renomear arquivo e diretório no mesmo pai | Nome antigo desaparece, novo nome aparece, conteúdo e identidade são preservados quando suportado | Colisão, nome inválido e troca concorrente não sobrescrevem terceiros |
| OPR-L-10 | `move_local` | Mover arquivo e diretório entre pastas temporárias | Entrada sai da pasta anterior e surge no destino com conteúdo correto | Destino ocupado, movimento de diretório para descendente, escopo externo e filesystem incompatível falham sem perdas |
| OPR-L-11 | `delete_local` | Excluir recurso identificado em árvore com vizinhos | Apenas a entrada identificada desaparece; resultado corresponde ao efeito observado | Referência obsoleta, precondição divergente, recurso ausente e diretório não vazio preservam recursos não autorizados |
| OPR-L-12 | `verify_local_integrity` | Comparar conteúdo com tamanho/hash conhecidos | `Verified` quando evidência corresponde; bytes originais permanecem intactos | Divergência gera erro de integridade; evidência insuficiente produz `NotAvailable`, sem sucesso fictício |

### Cenários transversais de segurança

| Caso | Cenário | Verificação obrigatória |
| --- | --- | --- |
| OPR-L-S01 | Raiz temporária com arquivo externo acessível por symlink | Operações de leitura/escrita/movimentação não escapam do escopo autorizado |
| OPR-L-S02 | Alteração concorrente entre consulta e operação | A operação valida o recurso efetivo e rejeita referência obsoleta quando necessário |
| OPR-L-S03 | Nomes Linux não UTF-8 válidos | Criação, leitura, renomeação e movimentação preservam bytes do nome |
| OPR-L-S04 | Dois comandos simultâneos para o mesmo destino | Exclusividade, precondições e atomicidade evitam sobrescrita silenciosa |
| OPR-L-S05 | Injeção de falhas antes e durante a publicação | Arquivo final permanece integralmente na versão antiga ou nova; erro identifica situação observada |
| OPR-L-S06 | Operação interrompida e limpeza de temporários | Temporários são rastreáveis e recuperados ou descartados de acordo com estado comprovado |
| OPR-L-S07 | Arquivos binários, vazios e maiores que o buffer de I/O | Conteúdo preservado e processamento em fluxo, sem truncamento por tamanho de bloco |
| OPR-L-S08 | Escrita concluída seguida de queda simulada antes da sincronização de dados/diretório | Relato de durabilidade é compatível com as garantias do sistema de arquivos |
| OPR-L-S09 | Retornos e mensagens de erro | Erros tipados, ausência de dados privados e nenhuma declaração de sucesso sem evidência |

### Critérios para validar a implementação privada

1. Cada caso `OPR-L-01` a `OPR-L-12` possui ao menos um teste positivo e um teste de falha com asserções sobre o estado real do filesystem.
2. Verificações de integridade e identidade usam valores esperados independentes da função testada.
3. Testes que necessitam simular falha, concorrência ou crash explicam o ponto de injeção e distinguem o que foi realmente executado do que foi apenas modelado.
4. Todo comportamento não definido suficientemente pela interface é marcado `BLOCKED`, com a definição técnica pendente registrada; não atribuir `PASS` com base em mocks que apenas repetem resultados esperados.
5. A implementação privada é validada diretamente, separadamente dos testes de fachada `OPR-01` a `OPR-11`.

## Acompanhamento e cancelamento — casos adicionais

| ID | Cenário | Resultado esperado |
| --- | --- | --- |
| OPR-P01 | Cada uma das dez requisições com `ExecutionOptions` | API aceita cancelamento e progress reporter opcionais sem requerer ambos |
| OPR-P02 | Transferência lenta com reporter | Fases verdadeiras, bytes observados monotônicos e total somente se conhecido |
| OPR-P03 | Renomear, mover, excluir, lixeira e restaurar com reporter | Fases adequadas sem fingir bytes transferidos |
| OPR-P04 | Nenhum reporter instalado | Operação termina normalmente; progresso não é pré-condição |
| OPR-P05 | Reporter lento, falha ou indisponível | Trabalho não fica bloqueado nem falha por causa do consumidor de telemetria |
| OPR-P06 | Cancelamento antes do primeiro efeito | Nenhuma mudança material; erro tipado de cancelamento quando apropriado |
| OPR-P07 | Cancelamento durante download antes de publicar | Arquivo final anterior preservado; temporário tratado com segurança |
| OPR-P08 | Cancelamento após efeito remoto, resposta perdida | Resultado indeterminado informado; sem retry automático ou reversão presumida |
| OPR-P09 | Cancelamento solicitado após ponto de commit | Não declarar cancelamento de efeito já confirmado; retorno representa realidade observada |
| OPR-P10 | Progresso alcança total de bytes antes de verificação | Não afirmar `Verified` ou conclusão só por bytes iguais ao total |
| OPR-P11 | Operações independentes em paralelo | Reporters isolados por chamada, sem misturar contagens |
| OPR-P12 | Comandos concorrentes no mesmo destino | Precondições/identidade impedem alteração indevida; não depender de fila interna do Operador |

Testes documentais devem usar execução controlada e fakes de IO para observar progresso durante a chamada, e filesystem temporário real onde aplicável. Sem evidência de teste Rust executado, não declarar aprovação.

## Critérios de aceite

1. Todos os métodos públicos têm testes positivos, negativos e de validação de resultados.
2. Operações locais são exercitadas sobre arquivos e diretórios temporários reais.
3. Operações remotas são exercitadas pela interface do provedor simulado, com chamadas e parâmetros verificáveis.
4. Retomada, concorrência e efeitos indeterminados são exercitados onde aplicáveis.
5. Testes comprovam comportamento por evidências independentes da própria implementação.
6. Resultados executados são relatados como PASS/FAIL/BLOCKED, identificando cenários não validados.

**Estado:** plano de testes documental; nenhum teste foi executado pela criação deste arquivo.

## Conformidade dos executores especializados

Executar esta suíte com cada implementação real do contrato `OperationExecutor`, usando um provedor simulado e filesystem temporário. Validar a saída e os efeitos reais capturados pelo ambiente de teste.

| Caso | Validação |
| --- | --- |
| OPR-E-01 | Implementa os dez métodos da interface; compilação confirma assinatura completa |
| OPR-E-02 | Seleção do executor usa serviço e conta da sessão, com isolamento entre contas |
| OPR-E-03 | `create_file` e `create_directory` retornam identidades reais dos recursos criados |
| OPR-E-04 | `upload` cria o recurso remoto e transfere o conteúdo exato |
| OPR-E-05 | `download` publica arquivo local íntegro |
| OPR-E-06 | `update_content` preserva a identidade do recurso existente |
| OPR-E-07 | `rename` e `move_item` alteram metadados preservando a identidade |
| OPR-E-08 | `delete`, `trash` e `restore` aplicam somente o efeito requisitado |
| OPR-E-09 | Capacidade ausente retorna erro tipado, sem falso sucesso |
| OPR-E-10 | Conflitos, permissão negada, falha parcial e resultado indeterminado são tratados |
| OPR-E-11 | Operações sobre arquivos locais respeitam as garantias da biblioteca privada |
| OPR-E-12 | Interrupções de transferência preservam os dados e informam o progresso verificado |

O executor concreto deve ser exercitado; um mock que só reproduz a resposta esperada não comprova conformidade.
