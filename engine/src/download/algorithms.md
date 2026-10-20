# Algoritmos — download

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
DOWNLOAD-001,DOWNLOAD-002.

## Objetivo
Baixar em staging e validar antes de instalar.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Consultar ID/revisão remotos.
2. verificar espaço.
3. baixar por streaming para arquivo temporário.
4. validar hash e tamanho quando possível.
5. sincronizar e renomear atomicamente.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Arquivo incompleto não substitui arquivo válido.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### DOWNLOAD-001

- **Técnica conhecida:** Gravação em staging e commit por renomeação no mesmo filesystem.
- **Referências primárias:** [Drive download/export](https://developers.google.com/workspace/drive/api/guides/manage-downloads), [Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html).
- **Política do DriveSync:** Criar temporário não visível como arquivo final; verificar destino e versões; rename controlado no mesmo volume; preservar arquivo anterior em conflitos.
- **Testes/oráculos a detalhar:** Rede cortada no meio, falta de espaço, falha ao renomear, processo encerrado, destino alterado.
- **Limitações / questões abertas:** Atomicidade de rename não cobre durabilidade em queda de energia; especificar sync_all do arquivo e diretório conforme plataforma.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### DOWNLOAD-002

- **Técnica conhecida:** Integridade de bytes e semântica de recursos Workspace.
- **Referências primárias:** [Drive download/export](https://developers.google.com/workspace/drive/api/guides/manage-downloads), [Drive files resource](https://developers.google.com/workspace/drive/api/reference/rest/v3/files).
- **Política do DriveSync:** Conferir tamanho/checksum quando disponíveis e aplicáveis; não assumir md5Checksum em Docs/Sheets/Slides; se garantia insuficiente, resultado permanece não validado.
- **Testes/oráculos a detalhar:** Checksum ausente, hash diferente, export parcial, mudança da revisão remota durante download.
- **Limitações / questões abertas:** Definir tratamento explícito de documentos nativos e suas limitações antes de permitir bidirecional.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
