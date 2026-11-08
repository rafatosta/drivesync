# Agentes — Detector Remoto Google

## Escopo

Implementar **somente** o especialista `detectors/remote/google/`, obedecendo o contrato do [Detector Remoto](../INTERFACES.md), seus [testes comuns](../TESTS.md) e os testes específicos em [TESTS.md](TESTS.md). Consultar [Google API](../../../providers/google/api/INTERFACES.md), [AGENTS raiz](../../../../../AGENTS.md) e [padrão](../../../MODULE_STANDARD.md).

## Fronteiras

- Não implementar nem refatorar Local, Storage, Detector pai, Upload/Download ou Inventário para facilitar o especialista Google.
- Fazer chamadas à Google API **por sua interface**, não montar URLs/HTTP/OAuth por conta própria.
- Usar sink injetado, emitindo um evento por chamada; não guardar a fila e não declarar sucesso sem aceite durável.
- Gerir polling/cursor específicos do Google, sem impor estratégia aos outros provedores.
- Sem tipos `GoogleFile` obrigatórios na API pública do Detector; tradução é interna.
- Checkpoint deve respeitar a durabilidade dos eventos já aceitos e suportar replay seguro.

## TDD

Testar especialista real com Google API fake e dependências simuladas, sem Google real. RED → GREEN → REFACTOR, regressão incremental e commits locais por unidade. Executar Cargo/Rust somente no Flatpak do manifesto; comentários e commits em inglês; sem push ou PR sem autorização explícita.

“Implemente Detector Remoto Google” não significa implementar qualquer outro módulo nem um provedor futuro. Reportar testes executados, cobertura dos cinco contratos, bloqueios e integrações pendentes.

**Este documento é instrução de implementação, não execução de código.**
