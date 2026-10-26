# ADR 0001 — Spec-first, language-independent modules

Status: proposed. Modules own role, contract, algorithm, and tests. Documentation stays independent from Rust, SQLite, HTTP and GTK where possible. Global safety invariants override module-local decisions. Implementation language can change without changing behavior contract. Source code and packaging come later, except initial executable window.
