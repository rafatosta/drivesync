# Módulo coordinator

Implementa os gates de identidade e disponibilidade usados pela fachada antes e
durante ciclos de sincronização. Não autentica, transfere arquivos ou classifica
conflitos de conteúdo. A fachada mantém a orquestração geral e retém o bloqueio
local durante preparação, download e upload.

- [Contrato](contract.md)
- [Algoritmos gerais propostos](algorithms.md)
- [Cenários e limites de validação](tests/test-cases.md)

A implementação de disponibilidade é parcial em relação ao Coordinator/Scheduler
geral proposto. Integração real com Google Drive, desmontagem e portal requer
validação específica; compilar no SDK não demonstra esses comportamentos.
