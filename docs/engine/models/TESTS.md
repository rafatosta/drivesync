# Models — TESTS (plano conceitual)

## Estado

**Papéis aprovados; atributos ainda não definidos.** Esta etapa documenta **o que precisará ser comprovado**, sem inventar fixtures, campos ou comportamentos adicionais para implementação.

Nenhum teste é executado ou exigível agora. A matriz final com dados de entrada/saída, casos de falha e nomes de métodos será redigida **depois da definição dos atributos** em [INTERFACES.md](INTERFACES.md).

## Matriz preliminar de responsabilidades

| Caso | Objeto | O que deve ser comprovado após definição de atributos |
| --- | --- | --- |
| MOD-CON-01 | `LocalFile` | Representa dados locais sem executar operações de filesystem |
| MOD-CON-02 | `RemoteFile` | Representa dados remotos sem depender da implementação de API específica |
| MOD-CON-03 | `FileData` | Contém **uma** variante `Local(LocalFile)` **ou** `Remote(RemoteFile)` |
| MOD-CON-04 | `FileData` | A identificação da variante cabe ao consumidor especializado, não ao Orquestrador |
| MOD-CON-05 | `File` | Transporta uma observação autocontida com contexto suficiente, conforme campos aprovados |
| MOD-CON-06 | `File` | Não equivale automaticamente a `SyncItem` ou estado atual confirmado |
| MOD-CON-07 | `FileBatch` | Agrupa objetos `File`; o contrato executável do lote pertence ao Detector |
| MOD-CON-08 | fronteira | `models/` não cria trait de serviço nem depende de Detector, Storage, Inventário ou Google API |
| MOD-CON-09 | notificações | Um evento sem metadados suficientes não é convertido em falsa confirmação de exclusão |
| MOD-CON-10 | persistência de entrega | A integração posterior Detector/Storage preserva `File` integralmente até confirmação segura, sem testar SQLite dentro de Models |

## Isolamento

Testes de Models devem ser **puros e independentes**: validar tipos e invariantes aprovados, sem rede, filesystem, banco, Orquestrador ou fakes de serviços. Testes de lotes/acknowledgement, replay e Storage **pertencem a `detectors/`**, não a Models.

Os casos acima são **requisitos conceituais**, não uma lista de testes executados. Na etapa 2 serão desdobrados em testes determinísticos específicos por atributo/variante, com casos positivos, negativos, opcionais e limites.

## Critério para permitir implementação

1. Aprovar os atributos de `LocalFile`, `RemoteFile` e `File`.
2. Fixar as regras de construção, validação, ausência, identidade e serialização escolhidas.
3. Ajustar casos de teste conforme **apenas** os contratos efetivamente aprovados.
4. Implementar e validar no Flatpak, sem inventar atributos ou APIs.

**Nenhum código Rust ou teste foi produzido nesta revisão.**
