# DriveSync requirements

Scope: non-official Google Drive synchronization client for GNOME. First release: account login, configured local folder, incremental observations, safe uploads/downloads and configurable sync selection. Offline content is real local files, not FUSE.

Out of scope v1: multiple cloud providers, block-delta protocol, virtual drive, unrestricted Google Docs bidirectional editing. Docs/Sheets/Slides are special remote resources.

Milestones: 0.1 login/list; 0.2 one-way download without deletes; 0.3 two-way sync with conflict handling; 1.0 validated recovery and robustness.


## Primeiro MVP funcional (planejamento, não número de release)

O [MVP](MVP.md) é uma entrega transversal que consolida os marcos iniciais de autenticação/listagem e download unilateral: **pelo menos duas contas Google independentes no mesmo processo, uma pasta remota/local por conta, transferência de arquivos comuns, incremental e recuperação isolada sem perda silenciosa**. A denominação “MVP 0.1” em seu documento designa a revisão do plano, não antecipa o lançamento 0.1/0.2 da aplicação.

Uploads, deleções automáticas e conflitos com resolução avançada permanecem no produto completo. Os contratos devem permitir evolução sem retrabalho estrutural.
