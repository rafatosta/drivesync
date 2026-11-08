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

## Critérios de aceite

1. Todos os métodos públicos têm testes positivos, negativos e de validação de resultados.
2. Operações locais são exercitadas sobre arquivos e diretórios temporários reais.
3. Operações remotas são exercitadas pela interface do provedor simulado, com chamadas e parâmetros verificáveis.
4. Retomada, concorrência e efeitos indeterminados são exercitados onde aplicáveis.
5. Testes comprovam comportamento por evidências independentes da própria implementação.
6. Resultados executados são relatados como PASS/FAIL/BLOCKED, identificando cenários não validados.

**Estado:** plano de testes documental; nenhum teste foi executado pela criação deste arquivo.
