# DriveSync requirements

Scope: non-official Google Drive synchronization client for GNOME. First release: account login, configured local folder, incremental observations, safe uploads/downloads and configurable sync selection. Offline content is real local files, not FUSE.

Out of scope v1: multiple cloud providers, block-delta protocol, virtual drive, unrestricted Google Docs bidirectional editing. Docs/Sheets/Slides are special remote resources.

Milestones: 0.1 login/list; 0.2 one-way download without deletes; 0.3 two-way sync with conflict handling; 1.0 validated recovery and robustness.
