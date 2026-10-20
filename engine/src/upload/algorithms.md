# Algoritmos — upload

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
UPLOAD-001,UPLOAD-002.

## Objetivo
Enviar arquivos com sessão retomável e confirmar revisão.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Observar versão local estável.
2. criar sessão retomável.
3. enviar blocos.
4. retomar offset após interrupção.
5. consultar resultado remoto antes de repetir operação ambígua.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Se arquivo mudar durante envio, reagendar sua versão atual.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### UPLOAD-001

- **Técnica conhecida:** Upload retomável com sessão e offsets.
- **Referências primárias:** [Google Drive upload guide](https://developers.google.com/workspace/drive/api/guides/manage-uploads).
- **Política do DriveSync:** Criar sessão, persistir dados necessários sem expor URI sensível, consultar status/offset ao retomar; não reenviar arquivo se sessão já o concluiu.
- **Testes/oráculos a detalhar:** Queda após bloco, expiração de sessão, arquivo grande, duplicação após restart.
- **Limitações / questões abertas:** A sessão expira; revisar retenção segura da URL e política de renovação.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### UPLOAD-002

- **Técnica conhecida:** Reconciliação após confirmação perdida / controle de versão remota.
- **Referências primárias:** [Google Drive upload guide](https://developers.google.com/workspace/drive/api/guides/manage-uploads), [Drive files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update).
- **Política do DriveSync:** Se o servidor aplicou mudança mas resposta se perdeu, consultar ID/revisão antes de repetir; revalidar origem local. Não presumir CAS atômico remoto.
- **Testes/oráculos a detalhar:** Timeout pós-commit, alteração local simultânea, atualização remota concorrente.
- **Limitações / questões abertas:** Prova de concorrência e precondições da Drive API é requisito bloqueante para overwrite automático.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
